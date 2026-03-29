#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperandKind {
    Register,
    Condition,
    Address,
    Pointer,
    PointerOffset,
    Immediate { bits: u8, signed: bool},
    Location,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstFmt {
    pub mnemonic: &'static str,
    pub operands: &'static [OperandKind],
}

macro_rules! reg {
    ($bits:expr) => {
        OperandKind::Register
    };
}

macro_rules! cond {
    ($bits:expr) => {
        OperandKind::Condition
    };
}

macro_rules! addr {
    ($bits:expr) => {
        OperandKind::Address
    };
}

macro_rules! ptr {
    ($bits:expr) => {
        OperandKind::Pointer
    };
}

macro_rules! ptoff {
    ($bits:expr) => {
        OperandKind::PointerOffset
    };
}

macro_rules! imm {
    ($bits:expr) => {
        OperandKind::Immediate {
            bits: $bits,
            signed: false,
        }
    };
}

macro_rules! simm {
    ($bits:expr) => {
        OperandKind::Immediate {
            bits: $bits,
            signed: true,
        }
    };
}

macro_rules! ximm {
    ($bits:expr) => {
        OperandKind::Immediate {
            bits: $bits,
            signed: Signedness::SourceDirected,
        }
    };
}

macro_rules! loc {
    ($bits:expr) => {
        OperandKind::Location
    };
}

macro_rules! inst {
    ($name:literal, [$($operand:expr),* $(,)?]) => {
        InstFmt {
            mnemonic: $name,
            operands: &[$($operand),*],
        }
    };
}

pub const INSTRUCTION_SET: &[InstFmt] = &[
    inst!("", [])
];

pub fn get_format(mnemonic: &str) -> Option<&'static InstFmt> {
    INSTRUCTION_SET
        .iter()
        .find(|spec| spec.mnemonic.eq_ignore_ascii_case(mnemonic))
}
