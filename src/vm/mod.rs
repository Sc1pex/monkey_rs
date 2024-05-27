#![allow(dead_code)]

use crate::{
    compiler::{Bytecode, Bytes, OpCode},
    eval::Object,
};

const STACK_SIZE: usize = 2048;

pub struct Vm {
    instructions: Bytes,
    constants: Vec<Object>,

    stack: Box<[Object; STACK_SIZE]>,
    /// Points to next value. Top of stack is at sp - 1
    sp: usize,
}

impl Vm {
    pub fn new(b: Bytecode) -> Self {
        Vm {
            instructions: b.instructions,
            constants: b.constants,

            stack: vec![Object::Null; STACK_SIZE].try_into().unwrap(),
            sp: 0,
        }
    }

    pub fn run(&mut self) -> RunResult {
        let mut ip = 0;

        while ip < self.instructions.len() {
            let op: OpCode = self.instructions.read(ip);
            ip += 1;

            match op {
                OpCode::Constant => {
                    let const_idx: u16 = self.instructions.read(ip);
                    self.push(self.constants[const_idx as usize].clone())?;
                    ip += 2;
                }
                OpCode::Add
                | OpCode::Sub
                | OpCode::Mul
                | OpCode::Div
                | OpCode::Greater
                | OpCode::Eq
                | OpCode::NotEq => self.execute_bin_op(op)?,
                OpCode::Pop => {
                    self.pop();
                }
                OpCode::True => self.push(Object::Bool(true))?,
                OpCode::False => self.push(Object::Bool(false))?,
                _ => todo!(),
            }
        }

        Ok(())
    }

    pub fn stack_top(&self) -> Option<&Object> {
        if self.sp == 0 {
            None
        } else {
            Some(&self.stack[self.sp - 1])
        }
    }

    pub fn last_popped(&self) -> &Object {
        &self.stack[self.sp]
    }
}

impl Vm {
    fn push(&mut self, obj: Object) -> RunResult {
        if self.sp >= STACK_SIZE {
            Err(format!("Stack overflow"))
        } else {
            self.stack[self.sp] = obj;
            self.sp += 1;
            Ok(())
        }
    }

    fn pop(&mut self) -> Object {
        let obj = self.stack[self.sp - 1].clone();
        self.sp -= 1;
        obj
    }

    fn execute_bin_op(&mut self, op: OpCode) -> RunResult {
        let right = self.pop();
        let left = self.pop();

        match (&left, &right) {
            (Object::Integer(left), Object::Integer(right)) => match op {
                OpCode::Add => self.push(Object::Integer(left + right)),
                OpCode::Sub => self.push(Object::Integer(left - right)),
                OpCode::Mul => self.push(Object::Integer(left * right)),
                OpCode::Div => self.push(Object::Integer(left / right)),
                OpCode::Eq => self.push(Object::Bool(left == right)),
                OpCode::NotEq => self.push(Object::Bool(left != right)),
                OpCode::Greater => self.push(Object::Bool(left > right)),
                _ => unreachable!(),
            },
            _ if left.kind() == right.kind() => match op {
                OpCode::Eq => self.push(Object::Bool(left == right)),
                OpCode::NotEq => self.push(Object::Bool(left != right)),
                _ => Err(format!("unknown operation: {} {} {}", left, op, right)),
            },
            _ => Err(format!("unknown operation: {} {} {}", left, op, right)),
        }
    }
}

pub type RunResult = Result<(), String>;

#[cfg(test)]
mod test;