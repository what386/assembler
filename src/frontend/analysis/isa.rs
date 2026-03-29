#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperandKind {
    Register,
    Condition,
    Address,
    Pointer,
    OffsetPointer,
    Offset { bit_length: u8 },
    Immediate { bit_length: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperandFormat {
    pub operand_order: u8,
    pub kind: OperandKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bitfield {
    Operand(OperandFormat),
    Kind(u8), // bit length
    Pad(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstFmt {
    pub bits: &'static str,
    pub mnemonic: &'static str,
    pub bitfields: &'static [Bitfield],
}


// isnt it so awesome and cool you cant just disable
// formatting for blocks like you can do with clangd

macro_rules! reg  { ($order:expr)            => { Bitfield::Operand(OperandFormat { operand_order: $order, kind: OperandKind::Register }) }; }
macro_rules! cond { ($order:expr)            => { Bitfield::Operand(OperandFormat { operand_order: $order, kind: OperandKind::Condition }) }; }
macro_rules! addr { ($order:expr)            => { Bitfield::Operand(OperandFormat { operand_order: $order, kind: OperandKind::Address }) }; }
macro_rules! ptr  { ($order:expr)            => { Bitfield::Operand(OperandFormat { operand_order: $order, kind: OperandKind::Pointer }) }; }
macro_rules! off  { ($order:expr, $len:expr) => { Bitfield::Operand(OperandFormat { operand_order: $order, kind: OperandKind::Offset { bit_length: $len } }) }; }
macro_rules! ptroff { ($order:expr)          => { Bitfield::Operand(OperandFormat { operand_order: $order, kind: OperandKind::OffsetPointer }) }; }
macro_rules! imm  { ($order:expr, $len:expr) => { Bitfield::Operand(OperandFormat { operand_order: $order, kind: OperandKind::Immediate { bit_length: $len } }) }; }

macro_rules! kind { ($len:expr) => { Bitfield::Kind($len) }; }
macro_rules! pad { ($len:expr) => { Bitfield::Pad($len) }; }

macro_rules! inst {
    ($bits:literal, $name:literal, [$($field:expr),* $(,)?]) => {
        InstFmt { bits: $bits, mnemonic: $name, bitfields: &[$($field),*] }
    };
}

#[rustfmt::skip]
pub const INSTRUCTION_SET: &[InstFmt] = &[
    inst!("00000", "func", [kind!(3), pad!(8)]),                        // misc functions
    inst!("00001", "ctrl", [kind!(3), imm!(0,8)]),                      // control commands
    inst!("00010", "in",   [reg!(0),  addr!(1)]),                       // input
    inst!("00011", "out",  [reg!(1),  addr!(0)]),                       // output
    inst!("00100", "jmp",  [imm!(0, 11)]),                              // jump
    inst!("00101", "bra",  [cond!(1), kind!(2), imm!(0,6)]),            // branch
    inst!("00110", "cal",  [imm!(0,11)]),                               // call subroutine
    inst!("00111", "crets",[cond!(1), kind!(2), off!(0,6)]),            // conditional return skip
    inst!("01000", "blit", [ptr!(0),  ptr!(1),  kind!(5)]),             // bit blit / logical combine
    inst!("01001", "bit",  [reg!(0),  reg!(1),  kind!(2), imm!(2, 3)]), // bit manipulation
    inst!("01010", "pop",  [reg!(0),  kind!(2), off!(1,6)]),            // pop stack
    inst!("01011", "psh",  [reg!(0),  kind!(2), off!(1,6)]),            // push stack
    inst!("01100", "mld",  [reg!(0),  addr!(1)]),                       // memory load
    inst!("01101", "mst",  [reg!(1),  addr!(0)]),                       // memory store
    inst!("01110", "mlx",  [reg!(0),  ptroff!(1)]),                     // memory load indexed
    inst!("01111", "msx",  [reg!(1),  ptroff!(0)]),                     // memory store indexed
    inst!("10000", "lim",  [reg!(0),  imm!(1, 8)]),                     // load immediate
    inst!("10001", "cmov", [reg!(0),  reg!(1), kind!(2), cond!(2)]),    // conditional move
    inst!("10010", "addi", [reg!(0),  imm!(1,8)]),                      // add immediate
    inst!("10011", "andi", [reg!(0),  imm!(1,8)]),                      // and immediate
    inst!("10100", "ori",  [reg!(0),  imm!(1,8)]),                      // or immediate
    inst!("10101", "xori", [reg!(0),  imm!(1,8)]),                      // xor immediate
    inst!("10110", "cmpi", [reg!(0),  imm!(1,8)]),                      // compare immediate
    inst!("10111", "tsti", [reg!(0),  imm!(1,8)]),                      // test immediate
    inst!("11000", "add",  [reg!(0),  reg!(1), kind!(2), reg!(2)]),     // addition
    inst!("11001", "sub",  [reg!(0),  reg!(1), kind!(2), reg!(2)]),     // subtraction
    inst!("11010", "bitw", [reg!(0),  reg!(1), kind!(2), reg!(2)]),     // bitwise ops
    inst!("11011", "bntw", [reg!(0),  reg!(1), kind!(2), reg!(2)]),     // inverse bitwise ops
    inst!("11100", "bsh",  [reg!(0),  reg!(1), kind!(2), reg!(2)]),     // barrel shift
    inst!("11101", "bshi", [reg!(0),  reg!(1), kind!(2), imm!(2, 3)]),  // barrel shift immediate
    inst!("11110", "mdo",  [reg!(0),  reg!(1), kind!(2), reg!(2)]),     // multiply / divide
    inst!("11111", "btc",  [reg!(0),  reg!(1), kind!(2), imm!(2,3)]),   // bit count
];

pub const PSEUDO_INSTRUCTION_SET: &[InstFmt] = &[
    inst!("00101", "brp",   [cond!(0), kind!(2), pad!(3), ptr!(1)]),
];
