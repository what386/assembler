use crate::models::Span;

struct Instruction {
    mnemonic: String,
    operands: Vec<Operand>,
    span: Span
}

enum Operand {
    Register(i64),
    Immediate(i64),
    Address(Address),
    Label(Label),
}

enum Address {
    Direct(i64),
    Pointer { register: i64, offset: i64}
}

struct Label {
    name: String,
    value: i64
}
