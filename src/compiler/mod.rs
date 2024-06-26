#![allow(dead_code)]

use std::rc::Rc;

use crate::{ast::*, eval::Object, lexer::TokenType};

pub use code::Bytes;
pub use instructions::{Instruction, OpCode};
pub use symbol_table::*;

mod code;
mod instructions;
mod symbol_table;

#[derive(Default)]
struct Scope {
    instructions: Bytes,

    last: Option<Emmited>,
    prev: Option<Emmited>,
}

pub struct Compiler {
    constants: Vec<Object>,
    symbol_table: SymbolTableRef,
    scopes: Vec<Scope>,
}

impl Default for Compiler {
    fn default() -> Self {
        let symbol_table = SymbolTable::empty();
        let builtins = ["len", "first", "last", "rest", "push", "puts"];
        for b in builtins {
            symbol_table.borrow_mut().define_builtin(b);
        }

        Self {
            constants: vec![Object::Null],
            symbol_table,
            scopes: vec![Scope::default()],
        }
    }
}

#[derive(Clone, Copy)]
struct Emmited {
    opcode: OpCode,
    pos: usize,
}

#[derive(Default)]
pub struct Bytecode {
    pub instructions: Bytes,
    pub constants: Vec<Object>,
}

impl Compiler {
    pub fn new_with_state(symbol_table: SymbolTableRef, constants: Vec<Object>) -> Self {
        Self {
            symbol_table,
            constants,
            ..Default::default()
        }
    }

    pub fn state(&self) -> (SymbolTableRef, Vec<Object>) {
        (self.symbol_table.clone(), self.constants.clone())
    }

    pub fn compile(&mut self, program: Program) -> CompileResult {
        self.compile_block(program.statements)
    }

    pub fn bytecode(self) -> Bytecode {
        Bytecode {
            instructions: self.current_scope().instructions.clone(),
            constants: self.constants,
        }
    }
}

impl Compiler {
    fn compile_stmt(&mut self, stmt: Statement) -> CompileResult {
        match stmt {
            Statement::Let(l) => {
                self.compile_expr(l.expr)?;
                let sym = self.symbol_table.borrow_mut().define(&l.ident);
                match sym.scope {
                    symbol_table::Scope::Global => {
                        self.emit(Instruction::new(OpCode::SetGlobal, &[sym.index as u32]))
                    }
                    symbol_table::Scope::Local => {
                        self.emit(Instruction::new(OpCode::SetLocal, &[sym.index as u32]))
                    }
                    _ => unreachable!(),
                };
                Ok(())
            }
            Statement::Return(r) => {
                self.compile_expr(r.expr)?;
                self.emit(Instruction::new(OpCode::ReturnValue, &[]));
                Ok(())
            }
            Statement::Expression(e) => {
                self.compile_expr(e)?;
                self.emit(Instruction::new(OpCode::Pop, &[]));
                Ok(())
            }
        }
    }

    fn compile_expr(&mut self, expr: Expression) -> CompileResult {
        match expr {
            Expression::Ident(i) => {
                let sym = self
                    .symbol_table
                    .borrow()
                    .resolve(&i)
                    .ok_or(format!("undefined symbol: {}", i))?;

                match sym.scope {
                    symbol_table::Scope::Global => {
                        self.emit(Instruction::new(OpCode::GetGlobal, &[sym.index as u32]));
                    }
                    symbol_table::Scope::Local => {
                        self.emit(Instruction::new(OpCode::GetLocal, &[sym.index as u32]));
                    }
                    symbol_table::Scope::Builtin => {
                        self.emit(Instruction::new(OpCode::GetBuiltin, &[sym.index as u32]));
                    }
                };
            }
            Expression::Number(x) => {
                let obj = Object::Integer(x);
                let idx = self.add_constant(obj) as u32;
                self.emit(Instruction::new(OpCode::Constant, &[idx]));
            }
            Expression::String(s) => {
                let obj = Object::String(s);
                let idx = self.add_constant(obj) as u32;
                self.emit(Instruction::new(OpCode::Constant, &[idx]));
            }
            Expression::Prefix(p) => self.compile_prefix(p)?,
            Expression::Infix(i) => self.compile_infix(i)?,
            Expression::Bool(b) => {
                match b {
                    true => self.emit(Instruction::new(OpCode::True, &[])),
                    false => self.emit(Instruction::new(OpCode::False, &[])),
                };
            }
            Expression::If(IfExpr {
                condition,
                if_branch,
                else_branch,
            }) => {
                self.compile_expr(*condition)?;
                let jmp_if = self.emit(Instruction::new(OpCode::JumpNotTrue, &[9999]));

                self.compile_block(if_branch)?;
                if self.last_is(OpCode::Pop) {
                    self.remove_last();
                }
                let jmp_else = self.emit(Instruction::new(OpCode::Jump, &[9999]));

                self.patch(
                    jmp_if,
                    Instruction::new(OpCode::JumpNotTrue, &[self.instructions().len() as u32]),
                );

                if let Some(else_branch) = else_branch {
                    self.compile_block(else_branch)?;
                    if self.last_is(OpCode::Pop) {
                        self.remove_last();
                    }
                } else {
                    self.emit(Instruction::null());
                }
                self.patch(
                    jmp_else,
                    Instruction::new(OpCode::Jump, &[self.instructions().len() as u32]),
                )
            }
            Expression::Func(f) => {
                let idx = self.compile_func(f)?;
                self.emit(Instruction::new(OpCode::Constant, &[idx]));
            }
            Expression::Call(c) => {
                self.compile_expr(*c.func)?;
                let args = c.arguments.len();
                for arg in c.arguments {
                    self.compile_expr(arg)?;
                }
                self.emit(Instruction::new(OpCode::Call, &[args as u32]));
            }
            Expression::Array(a) => {
                let len = a.elements.len();
                for e in a.elements {
                    self.compile_expr(e)?;
                }
                self.emit(Instruction::new(OpCode::Array, &[len as u32]));
            }
            Expression::Index(i) => {
                self.compile_expr(*i.left)?;
                self.compile_expr(*i.index)?;
                self.emit(Instruction::new(OpCode::Index, &[]));
            }
            Expression::Hash(h) => {
                let len = h.pairs.len();
                for (k, v) in h.pairs {
                    self.compile_expr(k)?;
                    self.compile_expr(v)?;
                }
                self.emit(Instruction::new(OpCode::Hash, &[len as u32]));
            }
        }

        Ok(())
    }
}

impl Compiler {
    fn compile_block(&mut self, block: Vec<Statement>) -> CompileResult {
        for stmt in block {
            self.compile_stmt(stmt)?;
        }
        Ok(())
    }

    fn compile_func(&mut self, FuncExpr { params, body }: FuncExpr) -> Result<u32, String> {
        self.enter_scope();

        for p in &params {
            self.symbol_table.borrow_mut().define(p);
        }

        self.compile_block(body)?;
        if self.last_is(OpCode::Pop) {
            self.remove_last();
            self.emit(Instruction::new(OpCode::ReturnValue, &[]));
        }
        if !self.last_is(OpCode::ReturnValue) {
            self.emit(Instruction::new(OpCode::Return, &[]));
        }
        let locals = self.symbol_table.borrow().symbols();
        let body = self.leave_scope().instructions;

        Ok(self.add_constant(Object::CompiledFunc(Rc::new(
            crate::eval::CompiledFuncObj {
                instructions: body,
                locals,
                params: params.len(),
            },
        ))) as u32)
    }

    fn add_constant(&mut self, obj: Object) -> usize {
        self.constants.push(obj);
        self.constants.len() - 1
    }

    fn emit(&mut self, i: Instruction) -> usize {
        let pos = self.instructions().len();

        self.current_scope_mut().prev = self.current_scope().last;
        self.current_scope_mut().last = Some(Emmited { opcode: i.op, pos });

        self.instructions_mut().push(i);
        pos
    }

    fn compile_prefix(&mut self, p: PrefixExpr) -> CompileResult {
        self.compile_expr(*p.right)?;
        match p.operator {
            TokenType::Minus => self.emit(Instruction::new(OpCode::Minus, &[])),
            TokenType::Bang => self.emit(Instruction::new(OpCode::Bang, &[])),
            _ => unreachable!(),
        };

        Ok(())
    }

    fn compile_infix(&mut self, i: InfixExpr) -> CompileResult {
        match i.operator {
            TokenType::Lt => self.compile_infix_rev(i),
            _ => self.compile_infix_normal(i),
        }
    }

    fn compile_infix_normal(&mut self, i: InfixExpr) -> CompileResult {
        self.compile_expr(*i.left)?;
        self.compile_expr(*i.right)?;

        match i.operator {
            TokenType::Plus => self.emit(Instruction::new(OpCode::Add, &[])),
            TokenType::Minus => self.emit(Instruction::new(OpCode::Sub, &[])),
            TokenType::Star => self.emit(Instruction::new(OpCode::Mul, &[])),
            TokenType::Slash => self.emit(Instruction::new(OpCode::Div, &[])),
            TokenType::Gt => self.emit(Instruction::new(OpCode::Greater, &[])),
            TokenType::Eq => self.emit(Instruction::new(OpCode::Eq, &[])),
            TokenType::NotEq => self.emit(Instruction::new(OpCode::NotEq, &[])),
            _ => unreachable!(),
        };
        Ok(())
    }

    fn compile_infix_rev(&mut self, i: InfixExpr) -> CompileResult {
        self.compile_expr(*i.right)?;
        self.compile_expr(*i.left)?;

        match i.operator {
            TokenType::Lt => self.emit(Instruction::new(OpCode::Greater, &[])),
            _ => unreachable!(),
        };
        Ok(())
    }

    fn last_is(&self, op: OpCode) -> bool {
        self.current_scope()
            .last
            .map(|l| l.opcode == op)
            .unwrap_or(false)
    }

    fn remove_last(&mut self) {
        let last = self.current_scope().last.expect("No instruction to remove");
        self.instructions_mut().remove(last.pos);

        self.current_scope_mut().last = self.current_scope().prev;
    }

    fn patch(&mut self, pos: usize, i: Instruction) {
        self.instructions_mut().patch(pos, i);
    }

    fn enter_scope(&mut self) {
        self.scopes.push(Scope::default());
        self.symbol_table = SymbolTable::new_enclosed(&self.symbol_table);
    }

    fn leave_scope(&mut self) -> Scope {
        let s = self.symbol_table.borrow_mut().outer.take();
        self.symbol_table = s.expect("Cannot leave out of global symbol table");

        assert!(self.scopes.len() > 1, "Cannot leave out of main scope");
        self.scopes.pop().unwrap()
    }

    fn instructions(&self) -> &Bytes {
        &self.current_scope().instructions
    }

    fn instructions_mut(&mut self) -> &mut Bytes {
        &mut self.current_scope_mut().instructions
    }

    fn current_scope(&self) -> &Scope {
        self.scopes
            .last()
            .expect("There should always exist at least one scope")
    }

    fn current_scope_mut(&mut self) -> &mut Scope {
        self.scopes
            .last_mut()
            .expect("There should always exist at least one scope")
    }
}

type CompileResult = Result<(), String>;

#[cfg(test)]
mod test;
