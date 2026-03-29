use crate::diagnostics::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    Label(LabelStatement),
    Instruction(InstructionStatement),
    Directive(DirectiveStatement),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelStatement {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionStatement {
    pub mnemonic: String,
    pub operands: Vec<Operand>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectiveStatement {
    pub name: String,
    pub args: Vec<DirectiveArg>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Register {
    R0,
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Address {
    Absolute(u64),
    Indexed { base: Register, offset: Option<i8> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StdCondition {
    Equal,
    NotEqual,
    Lower,
    Higher,
    LowerSame,
    HigherSame,
    Even,
    Always,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AltCondition {
    Overflow,
    NoOverflow,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    Odd,
    Always,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    Standard(StdCondition),
    Alternate(AltCondition),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    Register(Register),
    Immediate(i64),
    Address(Address),
    Condition(Condition),
    Label(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectiveArg {
    Identifier(String),
    Integer { raw: String, value: i64 },
    String(String),
    Char { raw: char, value: i64 },
}
