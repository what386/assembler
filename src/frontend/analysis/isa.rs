use phf::phf_map;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpFormatKind {
    Register,      // 3 bits
    Condition,     // 3 bits
    Address,       // 8 bits
    Pointer,       // 3 bits (just a register)
    OffsetPointer, // 8 bits (3 bit pointer + 5 bit offset)
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
    Pad { data: i32, length: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstFmt {
    pub bits: &'static str,
    pub mnemonic: &'static str,
    pub bitfields: &'static [Bitfield],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstructionSpec {
    pub bits: &'static str,
    pub bitfields: &'static [Bitfield],
    pub resolved_mnemonic: &'static str,
    pub kind: Option<u8>,
}

impl InstructionSpec {
    pub fn operand_formats(self) -> Vec<OperandFormat> {
        let mut operands = operand_formats_for_bitfields(self.bitfields);
        operands.sort_by_key(|operand| operand.operand_order);
        operands
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstOverload {
    pub resolved_mnemonic: &'static str,
    pub bitfields: &'static [Bitfield],
}

macro_rules! reg {
    ($order:expr) => {
        Bitfield::Operand(OperandFormat {
            operand_order: $order,
            kind: OpFormatKind::Register,
        })
    };
}
macro_rules! cond {
    ($order:expr) => {
        Bitfield::Operand(OperandFormat {
            operand_order: $order,
            kind: OpFormatKind::Condition,
        })
    };
}
macro_rules! addr {
    ($order:expr) => {
        Bitfield::Operand(OperandFormat {
            operand_order: $order,
            kind: OpFormatKind::Address,
        })
    };
}
macro_rules! ptr {
    ($order:expr) => {
        Bitfield::Operand(OperandFormat {
            operand_order: $order,
            kind: OpFormatKind::Pointer,
        })
    };
}
macro_rules! off {
    ($order:expr, $len:expr) => {
        Bitfield::Operand(OperandFormat {
            operand_order: $order,
            kind: OpFormatKind::Offset { bit_length: $len },
        })
    };
}
macro_rules! ptroff {
    ($order:expr) => {
        Bitfield::Operand(OperandFormat {
            operand_order: $order,
            kind: OpFormatKind::OffsetPointer,
        })
    };
}
macro_rules! imm {
    ($order:expr, $len:expr) => {
        Bitfield::Operand(OperandFormat {
            operand_order: $order,
            kind: OpFormatKind::Immediate { bit_length: $len },
        })
    };
}

macro_rules! kind {
    ($len:expr) => {
        Bitfield::Kind($len)
    };
}
macro_rules! pad {
    ($data:expr, $len:expr) => {
        Bitfield::Pad {
            data: $data,
            length: $len,
        }
    };
}

macro_rules! inst {
    ($bits:literal, $name:literal, [$($field:expr),* $(,)?]) => {
        InstFmt { bits: $bits, mnemonic: $name, bitfields: &[$($field),*] }
    };
}

macro_rules! overload {
    ($resolved:literal, [$($field:expr),* $(,)?]) => {
        InstOverload {
            resolved_mnemonic: $resolved,
            bitfields: &[$($field),*],
        }
    };
}

#[rustfmt::skip]
pub const INSTRUCTION_SET: &[InstFmt] = &[
    inst!("00000", "func", [kind!(3), imm!(0,8)]),                      // misc functions
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
pub const INSTRUCTION_OVERLOADS: &[InstOverload] = &[
    overload!("pop",  [reg!(0),  kind!(2), pad!(0,6)]),            // pop stack without offset
    overload!("psh",  [reg!(0),  kind!(2), pad!(0,6)]),            // push stack without offset
    overload!("cmov", [reg!(0),  reg!(1), kind!(2), pad!(0b111,3)]), // move/exchange without condition
    overload!("crets", [pad!(0b111,3), kind!(2), pad!(0,6)]), // return shorthand
    overload!("crets", [cond!(0), kind!(2), pad!(0,6)]),      // conditional return shorthand
];

#[rustfmt::skip]
pub const PSEUDO_INSTRUCTIONS: &[InstFmt] = &[
    inst!("00101", "brx",   [cond!(0), kind!(2), pad!(0,3), ptr!(1)]), // branch indexed -> branch
    inst!("11001", "cmp",  [pad!(0,3),  reg!(0), pad!(0,2), reg!(1)]), // compare -> subtraction
    inst!("11011", "not", [reg!(0), reg!(1), pad!(1,2), pad!(0, 3)]),  // NOT -> inverse bitwise
    inst!("10010", "inc", [reg!(0), pad!(1,8)]),                       // increment -> add immediate
    inst!("10010", "dec", [reg!(0), pad!(-1,8)]),                      // decrement -> add immediate
    inst!("00000", "halt", [pad!(0b001,3), pad!(0,8)]),                // halt processor
    inst!("00000", "nop",  [pad!(0b010,3), pad!(0,8)]),                // no-op
];

pub static INSTRUCTION_ALIASES: phf::Map<&'static str, (&'static str, u8)> = phf_map! {
    "ret"   => ("crets", 0b00),
    "brk"   => ("crets", 0b01),
    //""   => ("crets", 0b10),
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
    //""  => ("cmov", 0b10),
    //""  => ("cmov", 0b11),


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

    //NOTE: 0b000 is illegal
    //"" => ("func", 0b011),
    //"" => ("func", 0b100),
    //"" => ("func", 0b101),
    "mpge" => ("func", 0b110),
    "int" => ("func", 0b111),

    "timer.init" => ("ctrl", 0b000),
    "timer.val" => ("ctrl", 0b001),
    "pcw.clear" => ("ctrl", 0b010),
    "pcw.set" => ("ctrl", 0b011),
    //"" => ("", 0b100),
    //"" => ("", 0b101),
    //"" => ("", 0b110),
    //"" => ("", 0b111),
};

pub fn lookup_instruction(mnemonic: &str, operand_count: usize) -> Option<InstructionSpec> {
    let alias = resolve_instruction_alias(mnemonic).or_else(|| resolve_blit_alias(mnemonic));
    let overload_family = alias.map_or(mnemonic, |(resolved_mnemonic, _)| resolved_mnemonic);
    let kind = alias.map(|(_, kind)| kind);

    if let Some(overload) = instruction_overload(overload_family, operand_count) {
        let bits = instruction_format(INSTRUCTION_SET, overload.resolved_mnemonic)?.bits;
        return Some(InstructionSpec {
            bits,
            bitfields: overload.bitfields,
            resolved_mnemonic: overload.resolved_mnemonic,
            kind,
        });
    }

    if let Some((resolved_mnemonic, kind)) = alias {
        let fmt = instruction_format(INSTRUCTION_SET, resolved_mnemonic)?;
        if operand_count != operand_formats_for_bitfields(fmt.bitfields).len() {
            return None;
        }
        return Some(InstructionSpec {
            bits: fmt.bits,
            bitfields: fmt.bitfields,
            resolved_mnemonic,
            kind: Some(kind),
        });
    }

    if !matches!(mnemonic, "func" | "ctrl" | "blit")
        && let Some(fmt) = instruction_format(INSTRUCTION_SET, mnemonic)
        && operand_count == operand_formats_for_bitfields(fmt.bitfields).len()
    {
        return Some(InstructionSpec {
            bits: fmt.bits,
            bitfields: fmt.bitfields,
            resolved_mnemonic: fmt.mnemonic,
            kind: None,
        });
    }

    let fmt = instruction_format(PSEUDO_INSTRUCTIONS, mnemonic)?;
    if operand_count != operand_formats_for_bitfields(fmt.bitfields).len() {
        return None;
    }
    Some(InstructionSpec {
        bits: fmt.bits,
        bitfields: fmt.bitfields,
        resolved_mnemonic: fmt.mnemonic,
        kind: None,
    })
}

fn resolve_instruction_alias(mnemonic: &str) -> Option<(&'static str, u8)> {
    INSTRUCTION_ALIASES
        .get(mnemonic)
        .map(|(resolved_mnemonic, kind)| (*resolved_mnemonic, *kind))
}

fn resolve_blit_alias(mnemonic: &str) -> Option<(&'static str, u8)> {
    let mut parts = mnemonic.split('.');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("blit"), Some(op), Some(source), None) => {
            let op_bits = blit_op_bits(op)?;
            let source_bits = blit_source_bits(source)?;
            Some(("blit", (source_bits << 3) | op_bits))
        }
        _ => None,
    }
}

fn blit_op_bits(op: &str) -> Option<u8> {
    match op {
        "copy" => Some(0b000),
        "fill" => Some(0b001),
        "and" => Some(0b010),
        "or" => Some(0b011),
        "xor" => Some(0b100),
        "mask" => Some(0b101),
        _ => None,
    }
}

fn blit_source_bits(source: &str) -> Option<u8> {
    match source {
        "ram" => Some(0b00),
        "arom" => Some(0b01),
        "brom" => Some(0b10),
        _ => None,
    }
}

fn instruction_overload(
    resolved_mnemonic: &str,
    operand_count: usize,
) -> Option<&'static InstOverload> {
    INSTRUCTION_OVERLOADS
        .iter()
        .find(|overload| {
            overload.resolved_mnemonic == resolved_mnemonic
                && operand_formats_for_bitfields(overload.bitfields).len() == operand_count
        })
}

fn instruction_format<'a>(set: &'a [InstFmt], mnemonic: &str) -> Option<&'a InstFmt> {
    set.iter().find(|fmt| fmt.mnemonic == mnemonic)
}

fn operand_formats_for_bitfields(bitfields: &[Bitfield]) -> Vec<OperandFormat> {
    bitfields
        .iter()
        .filter_map(|bitfield| match bitfield {
            Bitfield::Operand(operand) => Some(*operand),
            Bitfield::Kind(_) | Bitfield::Pad { .. } => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{lookup_instruction, OpFormatKind};

    #[test]
    fn overloads_apply_to_pop_family_aliases() {
        for (mnemonic, expected_kind) in [("pop", 0), ("peek", 1), ("popf", 2), ("dsp", 3)] {
            let spec = lookup_instruction(mnemonic, 1).unwrap();

            assert_eq!(spec.resolved_mnemonic, "pop");
            assert_eq!(spec.kind, Some(expected_kind));

            let operands = spec.operand_formats();
            assert_eq!(operands.len(), 1);
            assert!(matches!(operands[0].kind, OpFormatKind::Register));
        }
    }

    #[test]
    fn overloads_apply_to_psh_family_aliases() {
        for (mnemonic, expected_kind) in [("psh", 0), ("poke", 1), ("pshf", 2), ("isp", 3)] {
            let spec = lookup_instruction(mnemonic, 1).unwrap();

            assert_eq!(spec.resolved_mnemonic, "psh");
            assert_eq!(spec.kind, Some(expected_kind));

            let operands = spec.operand_formats();
            assert_eq!(operands.len(), 1);
            assert!(matches!(operands[0].kind, OpFormatKind::Register));
        }
    }

    #[test]
    fn overloads_apply_to_crets_family_aliases() {
        for (mnemonic, expected_kind) in [("ret", 0), ("brk", 1), ("iret", 3)] {
            let spec = lookup_instruction(mnemonic, 0).unwrap();
            assert_eq!(spec.resolved_mnemonic, "crets");
            assert_eq!(spec.kind, Some(expected_kind));

            let spec = lookup_instruction(mnemonic, 1).unwrap();
            assert_eq!(spec.resolved_mnemonic, "crets");
            assert_eq!(spec.kind, Some(expected_kind));

            let operands = spec.operand_formats();
            assert_eq!(operands.len(), 1);
            assert!(matches!(operands[0].kind, OpFormatKind::Condition));
        }
    }

    #[test]
    fn overloads_apply_to_cmov_family_aliases() {
        for (mnemonic, expected_kind) in [("mov", 0), ("xchg", 1)] {
            let spec = lookup_instruction(mnemonic, 2).unwrap();

            assert_eq!(spec.resolved_mnemonic, "cmov");
            assert_eq!(spec.kind, Some(expected_kind));

            let operands = spec.operand_formats();
            assert_eq!(operands.len(), 2);
            assert!(matches!(operands[0].kind, OpFormatKind::Register));
            assert!(matches!(operands[1].kind, OpFormatKind::Register));
        }
    }

    #[test]
    fn resolves_structured_blit_aliases() {
        for (mnemonic, expected_kind) in [
            ("blit.copy.ram", 0b00000),
            ("blit.fill.arom", 0b01001),
            ("blit.mask.brom", 0b10101),
        ] {
            let spec = lookup_instruction(mnemonic, 2).unwrap();

            assert_eq!(spec.resolved_mnemonic, "blit");
            assert_eq!(spec.kind, Some(expected_kind));

            let operands = spec.operand_formats();
            assert_eq!(operands.len(), 2);
            assert!(matches!(operands[0].kind, OpFormatKind::Pointer));
            assert!(matches!(operands[1].kind, OpFormatKind::Pointer));
        }
    }

    #[test]
    fn rejects_raw_and_malformed_blit_names() {
        for mnemonic in [
            "blit",
            "blit.copy",
            "blit.ram.copy",
            "blit.copy.flash",
            "blit.foo.ram",
        ] {
            assert!(lookup_instruction(mnemonic, 2).is_none());
        }
    }
}
