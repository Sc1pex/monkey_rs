use super::*;
use crate::{ast::Parser, lexer::Lexer};
use instructions::{Instruction, OpCode};

macro_rules! test {
    ($($case:expr),* $(,)?) => {
        test(&[$($case),*])
    };
}

#[test]
fn integer_math() {
    test!(
        (
            "1 + 2",
            &[Object::Integer(1), Object::Integer(2)],
            &[
                Instruction::new(OpCode::Constant, &[1]),
                Instruction::new(OpCode::Constant, &[2]),
                Instruction::new(OpCode::Add, &[]),
                Instruction::new(OpCode::Pop, &[]),
            ],
        ),
        (
            "1 - 2",
            &[Object::Integer(1), Object::Integer(2)],
            &[
                Instruction::new(OpCode::Constant, &[1]),
                Instruction::new(OpCode::Constant, &[2]),
                Instruction::new(OpCode::Sub, &[]),
                Instruction::new(OpCode::Pop, &[]),
            ],
        ),
        (
            "1 / 2",
            &[Object::Integer(1), Object::Integer(2)],
            &[
                Instruction::new(OpCode::Constant, &[1]),
                Instruction::new(OpCode::Constant, &[2]),
                Instruction::new(OpCode::Div, &[]),
                Instruction::new(OpCode::Pop, &[]),
            ],
        ),
        (
            "1 * 2",
            &[Object::Integer(1), Object::Integer(2)],
            &[
                Instruction::new(OpCode::Constant, &[1]),
                Instruction::new(OpCode::Constant, &[2]),
                Instruction::new(OpCode::Mul, &[]),
                Instruction::new(OpCode::Pop, &[]),
            ],
        ),
        (
            "-1",
            &[Object::Integer(1)],
            &[
                Instruction::new(OpCode::Constant, &[1]),
                Instruction::new(OpCode::Minus, &[]),
                Instruction::new(OpCode::Pop, &[])
            ]
        )
    )
}

#[test]
fn bool_expressions() {
    test!(
        (
            "true",
            &[],
            &[
                Instruction::new(OpCode::True, &[]),
                Instruction::new(OpCode::Pop, &[])
            ]
        ),
        (
            "false",
            &[],
            &[
                Instruction::new(OpCode::False, &[]),
                Instruction::new(OpCode::Pop, &[])
            ]
        ),
        (
            "1 > 2",
            &[Object::Integer(1), Object::Integer(2)],
            &[
                Instruction::new(OpCode::Constant, &[1]),
                Instruction::new(OpCode::Constant, &[2]),
                Instruction::new(OpCode::Greater, &[]),
                Instruction::new(OpCode::Pop, &[]),
            ],
        ),
        (
            "1 < 2",
            &[Object::Integer(2), Object::Integer(1)],
            &[
                Instruction::new(OpCode::Constant, &[1]),
                Instruction::new(OpCode::Constant, &[2]),
                Instruction::new(OpCode::Greater, &[]),
                Instruction::new(OpCode::Pop, &[]),
            ],
        ),
        (
            "1 == 2",
            &[Object::Integer(1), Object::Integer(2)],
            &[
                Instruction::new(OpCode::Constant, &[1]),
                Instruction::new(OpCode::Constant, &[2]),
                Instruction::new(OpCode::Eq, &[]),
                Instruction::new(OpCode::Pop, &[]),
            ],
        ),
        (
            "1 != 2",
            &[Object::Integer(1), Object::Integer(2)],
            &[
                Instruction::new(OpCode::Constant, &[1]),
                Instruction::new(OpCode::Constant, &[2]),
                Instruction::new(OpCode::NotEq, &[]),
                Instruction::new(OpCode::Pop, &[]),
            ],
        ),
        (
            "!false",
            &[],
            &[
                Instruction::new(OpCode::False, &[]),
                Instruction::new(OpCode::Bang, &[]),
                Instruction::new(OpCode::Pop, &[])
            ]
        ),
    )
}

#[test]
fn conditionals() {
    test!(
        (
            "if (true) { 10; }; 3333;",
            &[Object::Integer(10), Object::Integer(3333)],
            &[
                Instruction::new(OpCode::True, &[]),          // 0
                Instruction::new(OpCode::JumpNotTrue, &[10]), // 1
                Instruction::new(OpCode::Constant, &[1]),     // 4
                Instruction::new(OpCode::Jump, &[13]),        // 7
                Instruction::null(),                          // 10
                Instruction::new(OpCode::Pop, &[]),           // 13
                Instruction::new(OpCode::Constant, &[2]),     // 14
                Instruction::new(OpCode::Pop, &[]),           // 17
            ]
        ),
        (
            "if (true) { 10; } else { 20; }; 3333;",
            &[
                Object::Integer(10),
                Object::Integer(20),
                Object::Integer(3333)
            ],
            &[
                Instruction::new(OpCode::True, &[]),          // 0
                Instruction::new(OpCode::JumpNotTrue, &[10]), // 1
                Instruction::new(OpCode::Constant, &[1]),     // 4
                Instruction::new(OpCode::Jump, &[13]),        // 7
                Instruction::new(OpCode::Constant, &[2]),     // 10
                Instruction::new(OpCode::Pop, &[]),           // 13
                Instruction::new(OpCode::Constant, &[3]),     // 14
                Instruction::new(OpCode::Pop, &[]),           // 17
            ]
        ),
    )
}

fn test(cases: &[(&str, &[Object], &[Instruction])]) {
    for (input, consts, instrs) in cases {
        let lexer = Lexer::new(input.to_string());
        let mut parser = Parser::new(lexer);
        let program = parser.parse().expect("Skill issue");

        let mut compiler = Compiler::default();
        compiler.compile(program).unwrap();
        let bytecode = compiler.bytecode();

        let expected_bytes = instrs.into_iter().fold(Bytes::default(), |mut acc, x| {
            acc.push(x);
            acc
        });

        assert!(
            bytecode.instructions == expected_bytes,
            "Wrong instructions. expected:\n{}got:\n{}",
            expected_bytes,
            bytecode.instructions,
        );

        assert_eq!(&bytecode.constants[1..], *consts);
    }
}
