#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    REG,
    COND,
    ADDR,
    PTR,
    PTROFF,
    IMM(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstFmt {
    pub mnemonic: &'static str,
    pub operands: &'static [Op],
}

const NO_OPERANDS: &[Op] = &[];
const THREE_REGISTERS: &[Op] = &[Op::REG, Op::REG, Op::REG];

#[rustfmt::skip]
pub const INSTRUCTION_SET: &[InstFmt] = &[
    InstFmt{ mnemonic: "func",  operands: &[]}
    InstFmt{ mnemonic: "ctrl",  operands: &[]}
    InstFmt{ mnemonic: "in",    operands: &[]}
    InstFmt{ mnemonic: "out",   operands: &[]}
    InstFmt{ mnemonic: "jmp",   operands: &[]}
    InstFmt{ mnemonic: "bra",   operands: &[]}
    InstFmt{ mnemonic: "cal",   operands: &[]}
    InstFmt{ mnemonic: "crets", operands: &[]}
    InstFmt{ mnemonic: "blit",  operands: &[]}
    InstFmt{ mnemonic: "bit",   operands: &[]}
    InstFmt{ mnemonic: "pop",   operands: &[]}
    InstFmt{ mnemonic: "psh",   operands: &[]}
    InstFmt{ mnemonic: "mld",   operands: &[]}
    InstFmt{ mnemonic: "mst",   operands: &[]}
    InstFmt{ mnemonic: "mldx",  operands: &[]}
    InstFmt{ mnemonic: "mstx",  operands: &[]}
    InstFmt{ mnemonic: "lim",   operands: &[]}
    InstFmt{ mnemonic: "cmov",  operands: &[]}
    InstFmt{ mnemonic: "addi",  operands: &[]}
    InstFmt{ mnemonic: "andi",  operands: &[]}
    InstFmt{ mnemonic: "ori",   operands: &[]}
    InstFmt{ mnemonic: "xori",  operands: &[]}
    InstFmt{ mnemonic: "cmpi",  operands: &[]}
    InstFmt{ mnemonic: "tsti",  operands: &[]}
    InstFmt{ mnemonic: "add",   operands: &[]}
    InstFmt{ mnemonic: "sub",   operands: &[]}
    InstFmt{ mnemonic: "bitw",  operands: &[]}
    InstFmt{ mnemonic: "bntw",  operands: &[]}
    InstFmt{ mnemonic: "bsh",   operands: &[]}
    InstFmt{ mnemonic: "bshi",  operands: &[]}
    InstFmt{ mnemonic: "mdo",   operands: &[]}
    InstFmt{ mnemonic: "btc",   operands: &[]}
];

pub fn get_format(mnemonic: &str) -> Option<&'static InstFmt> {
    INSTRUCTION_SET
        .iter()
        .find(|spec| spec.mnemonic.eq_ignore_ascii_case(mnemonic))
}

