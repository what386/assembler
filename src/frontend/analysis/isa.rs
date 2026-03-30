use phf::phf_map;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpFormatKind {
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
    pub kind: OpFormatKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bitfield {
    Operand(OperandFormat),
    // type of instruction. Set automatically by name table
    Kind(u8), // bit length
    // adds raw data to the output instruction
    Pad{ data: i32, length: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstFmt {
    pub bits: &'static str,
    pub mnemonic: &'static str,
    pub bitfields: &'static [Bitfield],
}

macro_rules! reg  { ($order:expr)            => { Bitfield::Operand(OperandFormat { operand_order: $order, kind: OpFormatKind::Register }) }; }
macro_rules! cond { ($order:expr)            => { Bitfield::Operand(OperandFormat { operand_order: $order, kind: OpFormatKind::Condition }) }; }
macro_rules! addr { ($order:expr)            => { Bitfield::Operand(OperandFormat { operand_order: $order, kind: OpFormatKind::Address }) }; }
macro_rules! ptr  { ($order:expr)            => { Bitfield::Operand(OperandFormat { operand_order: $order, kind: OpFormatKind::Pointer }) }; }
macro_rules! off  { ($order:expr, $len:expr) => { Bitfield::Operand(OperandFormat { operand_order: $order, kind: OpFormatKind::Offset { bit_length: $len } }) }; }
macro_rules! ptroff { ($order:expr)          => { Bitfield::Operand(OperandFormat { operand_order: $order, kind: OpFormatKind::OffsetPointer }) }; }
macro_rules! imm  { ($order:expr, $len:expr) => { Bitfield::Operand(OperandFormat { operand_order: $order, kind: OpFormatKind::Immediate { bit_length: $len } }) }; }

macro_rules! kind { ($len:expr) => { Bitfield::Kind($len) }; }
macro_rules! pad { ($data:expr, $len:expr) => { Bitfield::Pad{data: $data, length: $len} }; }

macro_rules! inst {
    ($bits:literal, $name:literal, [$($field:expr),* $(,)?]) => {
        InstFmt { bits: $bits, mnemonic: $name, bitfields: &[$($field),*] }
    };
}

#[rustfmt::skip]
pub const INSTRUCTION_SET: &[InstFmt] = &[
    inst!("00000", "func", [kind!(3), pad!(0,8)]),                      // misc functions
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
    inst!("11010", "bitw", [reg!(0),  reg!(1), kind!(2), reg!(2)]),     // bitwise
    inst!("11011", "bntw", [reg!(0),  reg!(1), kind!(2), reg!(2)]),     // inverse bitwise
    inst!("11100", "bsh",  [reg!(0),  reg!(1), kind!(2), reg!(2)]),     // barrel shift
    inst!("11101", "bshi", [reg!(0),  reg!(1), kind!(2), imm!(2, 3)]),  // barrel shift immediate
    inst!("11110", "mdo",  [reg!(0),  reg!(1), kind!(2), reg!(2)]),     // multiply / divide
    inst!("11111", "btc",  [reg!(0),  reg!(1), kind!(2), imm!(2,3)]),   // bit count
];

#[rustfmt::skip]
pub const PSEUDO_INSTRUCTION_SET: &[InstFmt] = &[
    inst!("00101", "brx",   [cond!(0), kind!(2), pad!(0,3), ptr!(1)]), // branch indexed -> branch
    inst!("11001", "cmp",  [pad!(0,3),  reg!(0), pad!(0,2), reg!(1)]), // compare -> subtraction
    inst!("11011", "not", [reg!(0), reg!(1), pad!(1,2), pad!(0, 3)]),  // NOT -> inverse bitwise
    inst!("10010", "inc", [reg!(0), pad!(1,8)]),                       // increment -> add immediate
    inst!("10010", "dec", [reg!(0), pad!(-1,8)]),                      // decrement -> add immediate
];

pub static INSTRUCTION_ALIASES: phf::Map<&'static str, (&'static str, u8)> = phf_map! {
    "ret"   => ("crets", 0b00),
    "brk"   => ("crets", 0b01),
    "iret"  => ("crets", 0b11),

    "pop"   => ("pop",  0b00),
    "peek"  => ("pop",  0b01),
    "popf"  => ("pop",  0b10),
    "dsp"   => ("pop",  0b11),

    "psh"   => ("psh",  0b00),
    "poke"  => ("psh",  0b01),
    "pshf"  => ("psh",  0b10),
    "isp"   => ("psh",  0b11),

    "mov"   => ("cmov", 0b00),
    "xchg"  => ("cmov", 0b01),

    "add"   => ("add",  0b00),
    "adc"   => ("add",  0b01),
    "adv"   => ("add",  0b10),
    "advc"  => ("add",  0b11),

    "sub"   => ("sub",  0b00),
    "sbb"   => ("sub",  0b01),
    "sbv"   => ("sub",  0b10),
    "sbvb"  => ("sub",  0b11),

    "and"   => ("bitw", 0b00),
    "or"    => ("bitw", 0b01),
    "xor"   => ("bitw", 0b10),
    "imp"   => ("bitw", 0b11),

    "nand"  => ("bntw", 0b00),
    "nor"   => ("bntw", 0b01),
    "xnor"  => ("bntw", 0b10),
    "nimp"  => ("bntw", 0b11),

    "bsl"   => ("bsh",  0b00),
    "bsr"   => ("bsh",  0b01),
    "rol"   => ("bsh",  0b10),
    "bsxr"  => ("bsh",  0b11),

    "bsli"  => ("bshi", 0b00),
    "bsri"  => ("bshi", 0b01),
    "roli"  => ("bshi", 0b10),
    "bsxri" => ("bshi", 0b11),

    "mul"   => ("mdo",  0b00),
    "mulu"  => ("mdo",  0b01),
    "div"   => ("mdo",  0b10),
    "mod"   => ("mdo",  0b11),

    "sqrt"  => ("btc",  0b00),
    "clz"   => ("btc",  0b01),
    "ctz"   => ("btc",  0b10),
    "popcnt"=> ("btc",  0b11),
};
