use crate::diagnostics::Span;

pub type Register = u8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Address {
    Absolute(u64),
    Indexed {
        base: Register,
        offset: Option<i8>,
    },
}

pub enum StdCondition {
    Equal,
    NotEqual,
    Lower,
    Higher,
    LowerSame,
    HigherSame,
    Even,
    Always
}

pub enum AltCondition {
    Overflow,
    NoOverflow,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    Odd,
    Always
}

pub enum Condition {
    Standard(StdCondition),
    Alternate(AltCondition)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    Register(Register),
    Immediate(i64),
    Address(Address),
    Symbol(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementKind {
    Label { name: String },
    Instruction {mnemonic: String, operands: Vec<Operand>},
    Directive { name: String, args: Vec<String>},
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}
