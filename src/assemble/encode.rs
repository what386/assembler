use crate::assemble::resolution::{
    ResolvedAddress, ResolvedInstruction, ResolvedOperand, Resolver,
};
use crate::{
    assemble::encodings::encode_condition,
    diagnostics::{Diagnostic, DiagnosticCode, DiagnosticEmitter, DiagnosticLabel, Partial, Span},
    directives::{
        data::encode_data_directive,
        incbin::{IncbinContext, incbin_bytes},
    },
    frontend::{
        analysis::{isa::{Bitfield, OpFormatKind}, symbol_table::SymbolTable},
        syntax::statements::{DirectiveArg, DirectiveStatement, Program, Statement},
    },
};

const PAGE_SIZE_BYTES: i64 = 128;
const INSTRUCTION_SIZE_BYTES: i64 = 2;

#[derive(Debug, Clone, Default)]
pub struct Encoder {
    incbin: IncbinContext,
}

impl Encoder {
    pub fn new() -> Self {
        Self::with_context(IncbinContext::default())
    }

    pub fn with_context(incbin: IncbinContext) -> Self {
        Self { incbin }
    }

    pub fn assemble(&self, program: &Program) -> Partial<Vec<u8>> {
        let mut emitter = DiagnosticEmitter::new();
        let symbols = SymbolTable::build_with_context(program, &self.incbin);
        let symbol_errors = !symbols.diagnostics.is_empty();
        emitter.extend(symbols.diagnostics);
        let Some(symbols) = symbols.value else {
            return emitter.fail();
        };
        if symbol_errors {
            return emitter.fail();
        }

        let resolver = Resolver::new();
        let mut image = Vec::new();
        let mut cursor = 0usize;

        for statement in &program.statements {
            match statement {
                Statement::Label(_) => {}
                Statement::Instruction(instruction) => {
                    let resolved = match resolver.resolve_instruction(instruction, &symbols) {
                        Ok(resolved) => resolved,
                        Err(diagnostic) => {
                            emitter.push(diagnostic);
                            continue;
                        }
                    };

                    match self.encode_instruction(&resolved, cursor as i64) {
                        Ok(word) => write_word(&mut image, cursor, word),
                        Err(diagnostic) => {
                            emitter.push(diagnostic);
                            continue;
                        }
                    }
                    cursor += 2;
                }
                Statement::Directive(directive) => {
                    self.encode_directive(directive, &mut image, &mut cursor, &mut emitter);
                }
            }
        }

        emitter.finish(image)
    }

    fn encode_instruction(
        &self,
        instruction: &ResolvedInstruction,
        instruction_address: i64,
    ) -> Result<u16, Diagnostic> {
        let mut word = 0u16;
        let mut written_bits = 0u8;

        push_bits(
            &mut word,
            &mut written_bits,
            parse_opcode_bits(instruction.bits),
            instruction.bits.len() as u8,
            instruction.span,
        )?;

        for field in instruction.bitfields {
            match field {
                Bitfield::Operand(operand) => {
                    let value = instruction
                        .operands
                        .get(usize::from(operand.operand_order))
                        .ok_or_else(|| {
                            encoding_error(
                                instruction.span,
                                format!(
                                    "instruction `{}` is missing operand {}",
                                    instruction.mnemonic, operand.operand_order
                                ),
                            )
                        })?;
                    let (bits, length) = encode_operand(
                        instruction.mnemonic.as_str(),
                        operand.operand_order,
                        value,
                        operand.kind,
                        instruction.span,
                        instruction_address,
                    )?;
                    push_bits(&mut word, &mut written_bits, bits, length, instruction.span)?;
                }
                Bitfield::Kind(length) => {
                    let value = u32::from(instruction.kind.unwrap_or(0));
                    ensure_unsigned_fit(
                        i64::from(value),
                        *length,
                        instruction.span,
                        "instruction kind does not fit field width",
                    )?;
                    push_bits(
                        &mut word,
                        &mut written_bits,
                        value,
                        *length,
                        instruction.span,
                    )?;
                }
                Bitfield::Pad { data, length } => {
                    let bits = encode_signed_value(
                        i64::from(*data),
                        *length,
                        instruction.span,
                        "fixed pad value does not fit field width",
                    )?;
                    push_bits(
                        &mut word,
                        &mut written_bits,
                        bits,
                        *length,
                        instruction.span,
                    )?;
                }
            }
        }

        if written_bits != 16 {
            return Err(encoding_error(
                instruction.span,
                format!(
                    "instruction `{}` encoded to {written_bits} bits instead of 16",
                    instruction.mnemonic
                ),
            ));
        }

        Ok(word)
    }

    fn encode_directive(
        &self,
        directive: &DirectiveStatement,
        image: &mut Vec<u8>,
        cursor: &mut usize,
        emitter: &mut DiagnosticEmitter,
    ) {
        if encode_data_directive(directive, image, cursor, emitter) {
            return;
        }

        match directive.name.as_str() {
            "page" => {
                align_pages(image, cursor, directive, emitter);
            }
            "org" => {
                if let Some(target) = directive_address(directive, 0, 1, "origin", false, emitter) {
                    seek(image, cursor, target, directive, emitter);
                }
            }
            "incbin" => match incbin_bytes(directive, &self.incbin) {
                Ok(bytes) => {
                    for byte in bytes {
                        write_byte(image, *cursor, byte);
                        *cursor += 1;
                    }
                }
                Err(diagnostic) => emitter.push(diagnostic),
            },
            other => emitter.push(encoding_error(
                directive.span,
                format!("directive `.{other}` cannot be encoded"),
            )),
        }
    }
}

fn encode_operand(
    mnemonic: &str,
    operand_order: u8,
    operand: &ResolvedOperand,
    kind: OpFormatKind,
    span: Span,
    instruction_address: i64,
) -> Result<(u32, u8), Diagnostic> {
    match kind {
        OpFormatKind::Register => match operand {
            ResolvedOperand::Register(register) => Ok((
                encode_unsigned_value(*register, 3, span, "register does not fit in 3 bits")?,
                3,
            )),
            _ => Err(encoding_error(
                span,
                "expected register operand during encoding",
            )),
        },
        OpFormatKind::Condition => match operand {
            ResolvedOperand::Condition(condition) => {
                Ok((u32::from(encode_condition(condition)), 3))
            }
            _ => Err(encoding_error(
                span,
                "expected condition operand during encoding",
            )),
        },
        OpFormatKind::Address => match operand {
            ResolvedOperand::Address(ResolvedAddress::Direct(address)) => Ok((
                encode_unsigned_value(*address, 8, span, "address does not fit in 8 bits")?,
                8,
            )),
            ResolvedOperand::Label(label) => Ok((
                encode_unsigned_value(label.value, 8, span, "address does not fit in 8 bits")?,
                8,
            )),
            _ => Err(encoding_error(
                span,
                "expected absolute address during encoding",
            )),
        },
        OpFormatKind::Pointer => match operand {
            ResolvedOperand::Address(ResolvedAddress::Pointer { register, offset }) => {
                if *offset != 0 {
                    return Err(encoding_error(
                        span,
                        "pointer operand must not include an offset",
                    ));
                }
                Ok((
                    encode_unsigned_value(*register, 3, span, "pointer register does not fit")?,
                    3,
                ))
            }
            _ => Err(encoding_error(
                span,
                "expected pointer operand during encoding",
            )),
        },
        OpFormatKind::OffsetPointer => match operand {
            ResolvedOperand::Address(ResolvedAddress::Pointer { register, offset }) => {
                let register_bits =
                    encode_unsigned_value(*register, 3, span, "pointer register does not fit")?;
                let offset_bits =
                    encode_signed_value(*offset, 5, span, "pointer offset does not fit in 5 bits")?;
                Ok((((register_bits << 5) | offset_bits), 8))
            }
            _ => Err(encoding_error(
                span,
                "expected indexed pointer operand during encoding",
            )),
        },
        OpFormatKind::Offset { bit_length } => {
            if is_control_flow_location(mnemonic, operand_order) {
                return encode_control_flow_target(
                    mnemonic,
                    operand,
                    span,
                    bit_length,
                    instruction_address,
                );
            }
            let value = encode_location_like_operand(operand, span)?;
            Ok((
                encode_signed_value(
                    value,
                    bit_length,
                    span,
                    "offset does not fit target field width",
                )?,
                bit_length,
            ))
        }
        OpFormatKind::Immediate { bit_length } => {
            if is_control_flow_location(mnemonic, operand_order) {
                return encode_control_flow_target(
                    mnemonic,
                    operand,
                    span,
                    bit_length,
                    instruction_address,
                );
            }
            let value = encode_location_like_operand(operand, span)?;
            Ok((
                encode_signed_value(
                    value,
                    bit_length,
                    span,
                    "immediate does not fit target field width",
                )?,
                bit_length,
            ))
        }
    }
}

fn is_control_flow_location(mnemonic: &str, operand_order: u8) -> bool {
    matches!((mnemonic, operand_order), ("jmp" | "cal" | "bra", 0))
}

fn encode_control_flow_target(
    mnemonic: &str,
    operand: &ResolvedOperand,
    span: Span,
    bit_length: u8,
    _instruction_address: i64,
) -> Result<(u32, u8), Diagnostic> {
    let target_byte_address = encode_location_like_operand(operand, span)?;
    if target_byte_address < 0 {
        return Err(encoding_error(
            span,
            "control-flow target must be a non-negative address",
        ));
    }
    if target_byte_address % INSTRUCTION_SIZE_BYTES != 0 {
        return Err(encoding_error(
            span,
            "control-flow target must be instruction-aligned",
        ));
    }

    if mnemonic == "bra" {
        let slot = target_byte_address.rem_euclid(PAGE_SIZE_BYTES) / INSTRUCTION_SIZE_BYTES;
        return Ok((
            encode_unsigned_value(
                slot,
                bit_length,
                span,
                "branch target does not fit in the 6-bit page slot",
            )?,
            bit_length,
        ));
    }

    let instruction_index = target_byte_address / INSTRUCTION_SIZE_BYTES;
    Ok((
        encode_unsigned_value(
            instruction_index,
            bit_length,
            span,
            "control-flow target does not fit target field width",
        )?,
        bit_length,
    ))
}

fn encode_location_like_operand(operand: &ResolvedOperand, span: Span) -> Result<i64, Diagnostic> {
    match operand {
        ResolvedOperand::Immediate(value) => Ok(*value),
        ResolvedOperand::Label(label) => Ok(label.value),
        ResolvedOperand::Address(ResolvedAddress::Direct(address)) => Ok(*address),
        _ => Err(encoding_error(
            span,
            "expected immediate, label, or absolute address during encoding",
        )),
    }
}

fn parse_opcode_bits(bits: &str) -> u32 {
    bits.bytes().fold(0u32, |acc, bit| match bit {
        b'0' => acc << 1,
        b'1' => (acc << 1) | 1,
        _ => acc,
    })
}

fn push_bits(
    word: &mut u16,
    written_bits: &mut u8,
    value: u32,
    length: u8,
    span: Span,
) -> Result<(), Diagnostic> {
    if *written_bits + length > 16 {
        return Err(encoding_error(span, "instruction fields exceed 16 bits"));
    }

    let shift = 16 - (*written_bits + length);
    *word |= (value as u16) << shift;
    *written_bits += length;
    Ok(())
}

fn encode_unsigned_value(
    value: i64,
    bit_length: u8,
    span: Span,
    message: &str,
) -> Result<u32, Diagnostic> {
    ensure_unsigned_fit(value, bit_length, span, message)?;
    Ok(value as u32)
}

fn ensure_unsigned_fit(
    value: i64,
    bit_length: u8,
    span: Span,
    message: &str,
) -> Result<(), Diagnostic> {
    if value < 0 {
        return Err(encoding_error(span, message));
    }

    let max = (1_i64 << bit_length) - 1;
    if value > max {
        return Err(encoding_error(span, message));
    }

    Ok(())
}

fn encode_signed_value(
    value: i64,
    bit_length: u8,
    span: Span,
    message: &str,
) -> Result<u32, Diagnostic> {
    let mask = (1_i64 << bit_length) - 1;
    if value >= 0 {
        if value > mask {
            return Err(encoding_error(span, message));
        }
        return Ok(value as u32);
    }

    let min = -(1_i64 << (bit_length - 1));
    if value < min {
        return Err(encoding_error(span, message));
    }

    Ok((value & mask) as u32)
}

fn directive_address(
    directive: &DirectiveStatement,
    index: usize,
    shift: u32,
    label: &str,
    left_shift: bool,
    emitter: &mut DiagnosticEmitter,
) -> Option<usize> {
    let value = match directive.args.get(index) {
        Some(DirectiveArg::Integer { value, .. } | DirectiveArg::Char { value, .. }) => *value,
        _ => {
            emitter.push(encoding_error(
                directive.span,
                format!(
                    "directive `.{}` expects an integer argument",
                    directive.name
                ),
            ));
            return None;
        }
    };
    if value < 0 {
        emitter.push(encoding_error(
            directive.span,
            format!("{label} must be non-negative"),
        ));
        return None;
    }

    let value = value as usize;
    Some(if left_shift { value << shift } else { value })
}

fn align_pages(
    image: &mut Vec<u8>,
    cursor: &mut usize,
    directive: &DirectiveStatement,
    emitter: &mut DiagnosticEmitter,
) {
    if let Some(target) = directive_address(directive, 0, 7, "page", true, emitter) {
        seek(image, cursor, target, directive, emitter);
    }
}

fn seek(
    image: &mut Vec<u8>,
    cursor: &mut usize,
    target: usize,
    directive: &DirectiveStatement,
    emitter: &mut DiagnosticEmitter,
) {
    if target < *cursor {
        emitter.push(encoding_error(
            directive.span,
            format!(
                "directive `.{}` may not move encoding backward",
                directive.name
            ),
        ));
        return;
    }

    ensure_size(image, target);
    *cursor = target;
}

fn write_word(image: &mut Vec<u8>, at: usize, word: u16) {
    ensure_size(image, at + 2);
    image[at] = (word >> 8) as u8;
    image[at + 1] = word as u8;
}

fn write_byte(image: &mut Vec<u8>, at: usize, byte: u8) {
    ensure_size(image, at + 1);
    image[at] = byte;
}

fn ensure_size(image: &mut Vec<u8>, size: usize) {
    if image.len() < size {
        image.resize(size, 0);
    }
}

fn encoding_error(span: Span, message: impl Into<String>) -> Diagnostic {
    let message = message.into();
    Diagnostic::error_code(DiagnosticCode::EncodingError(message.clone()))
        .with_label(DiagnosticLabel::new(span, message))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

    use crate::{
        assemble::encode::Encoder,
        directives::incbin::IncbinContext,
        frontend::syntax::parser::Parser,
        preprocessing::Preprocessor,
    };

    fn parse(source: &str) -> crate::frontend::syntax::statements::Program {
        let preprocessed = Preprocessor::new()
            .preprocess(0, source)
            .into_result()
            .unwrap();
        Parser::new(&preprocessed.tokens)
            .parse()
            .into_result()
            .unwrap()
    }

    fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("assembler-{name}-{unique}.bin"));
        fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn encodes_addi_big_endian() {
        let program = parse("addi r1, 0x0f\n");
        let image = Encoder::new().assemble(&program).into_result().unwrap();

        assert_eq!(image, vec![0x91, 0x0f]);
    }

    #[test]
    fn encodes_jump_targets_using_instruction_indices() {
        let program = parse("start:\nhalt\nafter:\njmp after\n");
        let image = Encoder::new().assemble(&program).into_result().unwrap();

        assert_eq!(image, vec![0x01, 0x00, 0x20, 0x01]);
    }

    #[test]
    fn encodes_branch_targets_as_page_slots() {
        let program = parse(".page 0\nstart:\nhalt\nbra start, ?equal\n");
        let image = Encoder::new().assemble(&program).into_result().unwrap();

        assert_eq!(image, vec![0x01, 0x00, 0x28, 0x00]);
    }

    #[test]
    fn encodes_layout_and_data_directives_into_flat_image() {
        let program =
            parse(".page 1\nhalt\n.org 0x0084\n.bytes 0xaa, 'B'\n.fill 2, 0x00\n.string \"hi\"\n.cstring \"!\"\n");
        let image = Encoder::new().assemble(&program).into_result().unwrap();

        assert_eq!(image.len(), 0x008c);
        assert_eq!(&image[0x0080..0x0082], &[0x01, 0x00]);
        assert_eq!(&image[0x0082..0x0084], &[0x00, 0x00]);
        assert_eq!(
            &image[0x0084..0x008c],
            &[0xaa, b'B', 0x00, 0x00, b'h', b'i', b'!', 0x00]
        );
    }

    #[test]
    fn page_directive_zero_fills_until_page_start() {
        let program = parse("halt\n.page 1\nhalt\n");
        let image = Encoder::new().assemble(&program).into_result().unwrap();

        assert_eq!(image.len(), 0x0082);
        assert_eq!(&image[0x0000..0x0002], &[0x01, 0x00]);
        assert!(image[0x0002..0x0080].iter().all(|byte| *byte == 0));
        assert_eq!(&image[0x0080..0x0082], &[0x01, 0x00]);
    }

    #[test]
    fn reports_range_errors_and_continues_encoding() {
        let program = parse("addi r1, 0x1ff\nhalt\n");
        let assembled = Encoder::new().assemble(&program);

        assert_eq!(assembled.diagnostics.len(), 1);
        assert_eq!(assembled.value.unwrap(), vec![0x01, 0x00]);
    }

    #[test]
    fn rejects_zero_directive_as_unsupported() {
        let program = parse(".zero 2\n");
        let assembled = Encoder::new().assemble(&program);

        assert_eq!(assembled.diagnostics.len(), 1);
        assert_eq!(assembled.diagnostics[0].message, "directive `.zero` cannot be encoded");
    }

    #[test]
    fn encodes_conditional_ret_shorthand_like_long_form() {
        let shorthand = parse("ret ?equal\n");
        let long_form = parse("ret 0, ?equal\n");

        let shorthand_image = Encoder::new().assemble(&shorthand).into_result().unwrap();
        let long_form_image = Encoder::new().assemble(&long_form).into_result().unwrap();

        assert_eq!(shorthand_image, long_form_image);
    }

    #[test]
    fn encodes_cmov_shorthand_like_long_form() {
        let mov_shorthand = parse("mov r1, r2\n");
        let mov_long_form = parse("mov r1, r2, ?always\n");
        let xchg_shorthand = parse("xchg r3, r4\n");
        let xchg_long_form = parse("xchg r3, r4, ?always\n");

        let mov_shorthand_image = Encoder::new().assemble(&mov_shorthand).into_result().unwrap();
        let mov_long_form_image = Encoder::new().assemble(&mov_long_form).into_result().unwrap();
        let xchg_shorthand_image = Encoder::new().assemble(&xchg_shorthand).into_result().unwrap();
        let xchg_long_form_image = Encoder::new().assemble(&xchg_long_form).into_result().unwrap();

        assert_eq!(mov_shorthand_image, mov_long_form_image);
        assert_eq!(xchg_shorthand_image, xchg_long_form_image);
    }

    #[test]
    fn encodes_incbin_bytes_into_output_image() {
        let path = temp_file("encode", &[0xde, 0xad, 0xbe, 0xef]);
        let source = format!(".incbin \"{}\"\nhalt\n", path.display());
        let program = parse(&source);
        let image = Encoder::with_context(IncbinContext::default())
            .assemble(&program)
            .into_result()
            .unwrap();

        assert_eq!(image, vec![0xde, 0xad, 0xbe, 0xef, 0x01, 0x00]);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_relative_incbin_paths_without_base_dir() {
        let program = parse(".incbin \"asset.bin\"\n");
        let assembled = Encoder::new().assemble(&program);

        assert_eq!(assembled.diagnostics.len(), 1);
        assert_eq!(
            assembled.diagnostics[0].message,
            "directive `.incbin` requires an absolute path when reading source from stdin"
        );
    }
}
