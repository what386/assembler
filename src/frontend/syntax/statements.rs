use crate::diagnostics::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Register {
    R0, R1, R2, R3, R4, R5, R6, R7
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Address {
    Absolute(u64),
    Indexed {
        base: Register,
        offset: Option<i8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StdCondition {
    Equal, NotEqual, Lower, Higher, LowerSame, HigherSame, Even, Always
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AltCondition {
    Overflow, NoOverflow, Less, Greater, LessEqual, GreaterEqual, Odd, Always
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    Standard(StdCondition),
    Alternate(AltCondition)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    Register(Register),
    Immediate(i64),
    Address(Address),
    Condition(Condition),
    Symbol(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementKind {
    Label,
    Instruction(Vec<Operand>),
    Directive(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub name: String,
    pub kind: StatementKind,
    pub span: Span,
}
