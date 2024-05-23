#![allow(dead_code)]

use crate::{
    ast::{ArrayExpr, Expression, HashExpr, Ident, Program, Statement},
    lexer::TokenType,
};
use builtin::Builtin;
pub use env::Environment;
use object::*;
use std::{cell::RefCell, rc::Rc};

mod builtin;
mod env;
mod object;

pub fn eval_program(prog: Program, env: &Rc<RefCell<Environment>>) -> EvalResult {
    let mut res = Rc::new(Obj::Null);
    for stmt in prog.statements {
        res = eval_stmt(stmt, env)?;

        if let Obj::Return(val) = &*res {
            return Ok(val.clone());
        }
    }
    Ok(res)
}

fn eval_stmt(stmt: Statement, env: &Rc<RefCell<Environment>>) -> EvalResult {
    match stmt {
        Statement::Let(l) => {
            let val = eval_expr(l.expr, env)?;
            env.borrow_mut().set(l.ident, val);
            Ok(Rc::new(Obj::Null))
        }
        Statement::Return(r) => {
            let val = eval_expr(r.expr, env)?;
            Ok(Rc::new(Obj::Return(val)))
        }
        Statement::Expression(e) => eval_expr(e, env),
    }
}

fn eval_expr(e: Expression, env: &Rc<RefCell<Environment>>) -> EvalResult {
    match e {
        Expression::Ident(i) => eval_ident(i, env),
        Expression::Number(x) => Ok(Rc::new(Obj::Integer(x))),
        Expression::String(s) => Ok(Rc::new(Obj::String(s))),
        Expression::Prefix(p) => {
            let right = eval_expr(*p.right, env)?;
            eval_prefix(p.operator, right)
        }
        Expression::Infix(i) => {
            let left = eval_expr(*i.left, env)?;
            let right = eval_expr(*i.right, env)?;
            eval_infix(left, i.operator, right)
        }
        Expression::Bool(b) => Ok(Rc::new(Obj::Bool(b))),
        Expression::If(i) => {
            let cond = eval_expr(*i.condition, env)?;

            if cond.is_truthy() {
                eval_block(i.if_branch, env)
            } else {
                match i.else_branch {
                    Some(b) => eval_block(b, env),
                    None => Ok(Rc::new(Obj::Null)),
                }
            }
        }
        Expression::Func(f) => Ok(Rc::new(Obj::Func(FuncObj {
            expr: f,
            env: env.clone(),
        }))),
        Expression::Call(c) => {
            let func = eval_expr(*c.func, env)?;
            let args = eval_exprs(c.arguments, env)?;

            apply_func(func, args)
        }
        Expression::Array(a) => eval_arr(a, env),
        Expression::Index(i) => {
            let left = eval_expr(*i.left, env)?;
            let index = eval_expr(*i.index, env)?;

            eval_index(left, index)
        }
        Expression::Hash(h) => eval_hash(h, env),
    }
}

fn eval_ident(ident: Ident, env: &Rc<RefCell<Environment>>) -> EvalResult {
    if let Some(r) = env.borrow().get(&ident) {
        Ok(r)
    } else if let Some(b) = Builtin::from_ident(&ident) {
        Ok(b)
    } else {
        Err(format!("identifier not found: {}", ident))
    }
}

fn eval_arr(a: ArrayExpr, env: &Rc<RefCell<Environment>>) -> EvalResult {
    let elements = a
        .elements
        .into_iter()
        .map(|e| eval_expr(e, env))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Rc::new(Obj::Array(ArrayObj { elements })))
}

fn eval_hash(h: HashExpr, env: &Rc<RefCell<Environment>>) -> EvalResult {
    let keys = h
        .pairs
        .clone()
        .into_iter()
        .map(|(k, _)| eval_expr(k, env))
        .collect::<Result<Vec<_>, _>>()?;
    let values = h
        .pairs
        .into_iter()
        .map(|(_, v)| eval_expr(v, env))
        .collect::<Result<Vec<_>, _>>()?;
    let map = keys.into_iter().zip(values.into_iter()).collect();

    Ok(Rc::new(Obj::Hash(HashObj { map })))
}

fn eval_index(left: Rc<Obj>, index: Rc<Obj>) -> EvalResult {
    match (&*left, &*index) {
        (Obj::Array(left), Obj::Integer(index)) => Ok(left
            .elements
            .get(*index as usize)
            .cloned()
            .unwrap_or(Rc::new(Obj::Null))),
        (Obj::Hash(left), _) => {
            if matches!(*index, Obj::Integer(_) | Obj::String(_) | Obj::Bool(_)) {
                Ok(left.map.get(&index).cloned().unwrap_or(Rc::new(Obj::Null)))
            } else {
                Err(format!("unusable as hash key: {}", index.kind()))
            }
        }
        _ => Err(format!("index operator not supported: {}", left.kind())),
    }
}

fn eval_exprs(
    expr: Vec<Expression>,
    env: &Rc<RefCell<Environment>>,
) -> Result<Vec<Rc<Obj>>, String> {
    expr.into_iter().map(|e| eval_expr(e, env)).collect()
}

fn eval_block(block: Vec<Statement>, env: &Rc<RefCell<Environment>>) -> EvalResult {
    let mut res = Rc::new(Obj::Null);
    for stmt in block {
        res = eval_stmt(stmt, env)?;

        if matches!(*res, Obj::Return(_)) {
            return Ok(res);
        }
    }
    Ok(res)
}

fn eval_prefix(op: TokenType, right: Rc<Obj>) -> EvalResult {
    match op {
        TokenType::Bang => eval_bang_op(right),
        TokenType::Minus => eval_minus_op(right),
        _ => unreachable!(),
    }
}

fn eval_infix(left: Rc<Obj>, op: TokenType, right: Rc<Obj>) -> EvalResult {
    match (&*left, op, &*right) {
        (&Obj::Integer(left), _, &Obj::Integer(right)) => eval_integer_infix_op(left, op, right),
        (Obj::String(left), _, Obj::String(right)) => eval_string_infix_op(left, op, right),
        (left, TokenType::Eq, right) => Ok(Rc::new(Obj::Bool(left == right))),
        (left, TokenType::NotEq, right) => Ok(Rc::new(Obj::Bool(left != right))),
        (left, op, right) if left.kind() != right.kind() => Err(format!(
            "type mismatch: {} {} {}",
            left.kind(),
            op,
            right.kind()
        )),
        (left, op, right) => Err(format!(
            "unknown operator: {} {} {}",
            left.kind(),
            op,
            right.kind()
        )),
    }
}

fn eval_bang_op(value: Rc<Obj>) -> EvalResult {
    Ok(Rc::new(Obj::Bool(!value.is_truthy())))
}

fn eval_minus_op(value: Rc<Obj>) -> EvalResult {
    match *value {
        Obj::Integer(x) => Ok(Rc::new(Obj::Integer(-x))),
        _ => Err(format!("unknown operator: -{}", value.kind())),
    }
}

fn eval_integer_infix_op(left: i64, op: TokenType, right: i64) -> EvalResult {
    match op {
        TokenType::Plus => Ok(Rc::new(Obj::Integer(left + right))),
        TokenType::Minus => Ok(Rc::new(Obj::Integer(left - right))),
        TokenType::Star => Ok(Rc::new(Obj::Integer(left * right))),
        TokenType::Slash => Ok(Rc::new(Obj::Integer(left / right))),

        TokenType::Lt => Ok(Rc::new(Obj::Bool(left < right))),
        TokenType::Gt => Ok(Rc::new(Obj::Bool(left > right))),
        TokenType::Eq => Ok(Rc::new(Obj::Bool(left == right))),
        TokenType::NotEq => Ok(Rc::new(Obj::Bool(left != right))),
        _ => unreachable!(),
    }
}

fn eval_string_infix_op(left: &str, op: TokenType, right: &str) -> EvalResult {
    match op {
        TokenType::Plus => Ok(Rc::new(Obj::String(left.to_owned() + right))),

        TokenType::Eq => Ok(Rc::new(Obj::Bool(left == right))),
        TokenType::NotEq => Ok(Rc::new(Obj::Bool(left != right))),

        _ => Err(format!("unknown operator: STRING {} STRING", op)),
    }
}

fn apply_func(func: Rc<Obj>, args: Vec<Rc<Obj>>) -> EvalResult {
    let func = match &*func {
        Obj::Func(f) => f,
        Obj::Builtin(b) => return b.call(args),
        _ => return Err(format!("not a function: {}", func.kind())),
    };

    let env = Rc::new(RefCell::new(Environment::new_enclosed(func.env.clone())));
    if args.len() != func.expr.params.len() {
        return Err(format!(
            "function expects {} arguments but {} were given",
            func.expr.params.len(),
            args.len()
        ));
    }

    for (arg, param) in args.iter().zip(func.expr.params.iter()) {
        env.borrow_mut().set(param.into(), arg.clone())
    }
    let res = eval_block(func.expr.body.clone(), &env)?;

    match &*res {
        Obj::Return(r) => Ok(r.clone()),
        _ => Ok(res),
    }
}

type EvalResult = Result<Rc<Obj>, String>;

#[cfg(test)]
mod test;
