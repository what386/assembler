use crate::{
    diagnostics::{Diagnostic, DiagnosticCode, DiagnosticLabel, Span},
    frontend::syntax::{
        statements::{
            Address, AltCondition, Condition, DirectiveArg, DirectiveStatement,
            InstructionStatement, LabelStatement, Operand, Program, Register, Statement,
            StdCondition,
        },
        tokens::{Token, TokenKind},
    },
};

pub struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<Program, Diagnostic> {
        let mut statements = Vec::new();

        self.skip_newlines();
        while !self.at_end() {
            statements.push(self.parse_statement()?);
            self.skip_newlines();
        }

        Ok(Program { statements })
    }

    fn parse_statement(&mut self) -> Result<Statement, Diagnostic> {
        let start = self.current_span();

        match self.current_kind() {
            TokenKind::Dot => self.parse_directive(start),
            TokenKind::Identifier(_) => {
                let (name, end_span) = self.parse_qualified_name()?;
                if self.matches(|kind| matches!(kind, TokenKind::Colon)) {
                    let span = self.merge_spans(start, self.previous_span());
                    Ok(Statement::Label(LabelStatement { name, span }))
                } else {
                    self.parse_instruction_from_name(name, start, end_span)
                }
            }
            TokenKind::Newline | TokenKind::Eof => Err(self.error_here("expected statement")),
            _ => Err(self.error_here("expected label, instruction, or directive")),
        }
    }

    fn parse_directive(&mut self, start: Span) -> Result<Statement, Diagnostic> {
        self.bump();
        let (name, mut end_span) = self.parse_qualified_name()?;
        let args = if name == "bytes" {
            let (args, end) = self.parse_directive_args_with_commas()?;
            if let Some(end) = end {
                end_span = end;
            }
            args
        } else {
            let (args, end) = self.parse_directive_args_positional()?;
            if let Some(end) = end {
                end_span = end;
            }
            args
        };

        let span = self.merge_spans(start, end_span);
        Ok(Statement::Directive(DirectiveStatement {
            name,
            args,
            span,
        }))
    }

    fn parse_instruction_from_name(
        &mut self,
        mnemonic: String,
        start: Span,
        mut end_span: Span,
    ) -> Result<Statement, Diagnostic> {
        let mut operands = Vec::new();

        if !self.at_statement_end() {
            loop {
                let (operand, operand_span) = self.parse_operand()?;
                operands.push(operand);
                end_span = operand_span;

                if self.matches(|kind| matches!(kind, TokenKind::Comma)) {
                    if self.at_statement_end() {
                        return Err(self.error_here("expected operand after `,`"));
                    }
                    continue;
                }

                if self.at_statement_end() {
                    break;
                }

                return Err(self.error_here("expected `,` or end of line"));
            }
        }

        let span = self.merge_spans(start, end_span);
        Ok(Statement::Instruction(InstructionStatement {
            mnemonic,
            operands,
            span,
        }))
    }

    fn parse_operand(&mut self) -> Result<(Operand, Span), Diagnostic> {
        match self.current_kind() {
            TokenKind::Integer { .. }
            | TokenKind::Char { .. }
            | TokenKind::Plus
            | TokenKind::Minus => self.parse_immediate_operand(),
            TokenKind::Question => self.parse_standard_condition_operand(),
            TokenKind::At => self.parse_alternate_condition_operand(),
            TokenKind::LBracket => self.parse_address_operand(),
            TokenKind::Identifier(_) => {
                let (name, span) = self.parse_qualified_name()?;
                if let Some(register) = self.parse_register_name(&name) {
                    Ok((Operand::Register(register), span))
                } else {
                    Ok((Operand::Label(name), span))
                }
            }
            _ => Err(self.error_here("expected operand")),
        }
    }

    fn parse_immediate_operand(&mut self) -> Result<(Operand, Span), Diagnostic> {
        let start = self.current_span();
        let (value, end_span) = match self.current_kind() {
            TokenKind::Char { value, .. } => {
                let span = self.current_span();
                let value = *value;
                self.bump();
                (value, span)
            }
            _ => self.parse_signed_integer()?,
        };

        Ok((Operand::Immediate(value), self.merge_spans(start, end_span)))
    }

    fn parse_standard_condition_operand(&mut self) -> Result<(Operand, Span), Diagnostic> {
        let start = self.current_span();
        self.bump();
        let (name, end_span) = self.parse_qualified_name()?;
        let condition = self.parse_standard_condition_name(&name).ok_or_else(|| {
            self.error_with_code(
                DiagnosticCode::UnknownCondition(format!(
                    "unknown standard condition `{name}`"
                )),
                end_span,
            )
        })?;

        Ok((
            Operand::Condition(Condition::Standard(condition)),
            self.merge_spans(start, end_span),
        ))
    }

    fn parse_alternate_condition_operand(&mut self) -> Result<(Operand, Span), Diagnostic> {
        let start = self.current_span();
        self.bump();
        let (name, end_span) = self.parse_qualified_name()?;
        let condition = self.parse_alternate_condition_name(&name).ok_or_else(|| {
            self.error_with_code(
                DiagnosticCode::UnknownCondition(format!(
                    "unknown alternate condition `{name}`"
                )),
                end_span,
            )
        })?;

        Ok((
            Operand::Condition(Condition::Alternate(condition)),
            self.merge_spans(start, end_span),
        ))
    }

    fn parse_address_operand(&mut self) -> Result<(Operand, Span), Diagnostic> {
        let start = self.current_span();
        self.bump();

        match self.current_kind() {
            TokenKind::Identifier(_) => {
                let (name, end_span) = self.parse_qualified_name()?;
                let Some(base) = self.parse_register_name(&name) else {
                    return Err(self.error_with_code(
                        DiagnosticCode::UnknownRegister(
                            "expected register or absolute address".to_owned(),
                        ),
                        end_span,
                    ));
                };

                if self.matches(|kind| matches!(kind, TokenKind::RBracket)) {
                    return Ok((
                        Operand::Address(Address::Indexed { base, offset: None }),
                        self.merge_spans(start, self.previous_span()),
                    ));
                }

                let negative = if self.matches(|kind| matches!(kind, TokenKind::Plus)) {
                    false
                } else if self.matches(|kind| matches!(kind, TokenKind::Minus)) {
                    true
                } else {
                    return Err(self.error_here("expected `]`, `+`, or `-` in address"));
                };

                let (magnitude, offset_span) = self.parse_integer_token()?;
                let signed = if negative { -magnitude } else { magnitude };
                let offset = i8::try_from(signed).map_err(|_| {
                    self.error_with_code(
                        DiagnosticCode::InvalidOperand(
                            "indexed address offset must fit in i8".to_owned(),
                        ),
                        offset_span,
                    )
                })?;
                self.expect(TokenKind::RBracket, "expected `]` after address")?;

                Ok((
                    Operand::Address(Address::Indexed {
                        base,
                        offset: Some(offset),
                    }),
                    self.merge_spans(start, self.previous_span()),
                ))
            }
            TokenKind::Integer { .. } | TokenKind::Minus | TokenKind::Plus => {
                let ((_, value), span) = self.parse_signed_integer_with_raw()?;
                if value < 0 {
                    return Err(self.error_with_code(
                        DiagnosticCode::InvalidOperand(
                            "absolute address must be non-negative".to_owned(),
                        ),
                        span,
                    ));
                }

                self.expect(TokenKind::RBracket, "expected `]` after address")?;

                Ok((
                    Operand::Address(Address::Absolute(value as u64)),
                    self.merge_spans(start, self.previous_span()),
                ))
            }
            _ => Err(self.error_here("expected register or absolute address")),
        }
    }

    fn parse_directive_args_positional(
        &mut self,
    ) -> Result<(Vec<DirectiveArg>, Option<Span>), Diagnostic> {
        let mut args = Vec::new();
        let mut end_span = None;

        while !self.at_statement_end() {
            if matches!(self.current_kind(), TokenKind::Comma) {
                return Err(self.error_with_code(
                    DiagnosticCode::InvalidDirective("unexpected `,` in directive".to_owned()),
                    self.current_span(),
                ));
            }

            let (arg, span) = self.parse_directive_arg()?;
            end_span = Some(span);
            args.push(arg);
        }

        Ok((args, end_span))
    }

    fn parse_directive_args_with_commas(
        &mut self,
    ) -> Result<(Vec<DirectiveArg>, Option<Span>), Diagnostic> {
        let mut args = Vec::new();
        let mut end_span = None;

        if self.at_statement_end() {
            return Ok((args, end_span));
        }

        loop {
            let (arg, span) = self.parse_directive_arg()?;
            end_span = Some(span);
            args.push(arg);

            if self.matches(|kind| matches!(kind, TokenKind::Comma)) {
                if self.at_statement_end() {
                    return Err(self.error_with_code(
                        DiagnosticCode::InvalidDirective("expected value after `,`".to_owned()),
                        self.current_span(),
                    ));
                }
                continue;
            }

            if self.at_statement_end() {
                break;
            }

            return Err(self.error_with_code(
                DiagnosticCode::InvalidDirective("expected `,` or end of line".to_owned()),
                self.current_span(),
            ));
        }

        Ok((args, end_span))
    }

    fn parse_directive_arg(&mut self) -> Result<(DirectiveArg, Span), Diagnostic> {
        match self.current_kind() {
            TokenKind::Identifier(_) => {
                let (name, span) = self.parse_qualified_name()?;
                Ok((DirectiveArg::Identifier(name), span))
            }
            TokenKind::Integer { .. } | TokenKind::Minus | TokenKind::Plus => {
                let ((raw, value), span) = self.parse_signed_integer_with_raw()?;
                Ok((DirectiveArg::Integer { raw, value }, span))
            }
            TokenKind::String(value) => {
                let span = self.current_span();
                let value = value.clone();
                self.bump();
                Ok((DirectiveArg::String(value), span))
            }
            TokenKind::Char { raw, value } => {
                let span = self.current_span();
                let raw = *raw;
                let value = *value;
                self.bump();
                Ok((DirectiveArg::Char { raw, value }, span))
            }
            _ => Err(self.error_here("expected directive argument")),
        }
    }

    fn parse_qualified_name(&mut self) -> Result<(String, Span), Diagnostic> {
        let TokenKind::Identifier(name) = self.current_kind() else {
            return Err(self.error_here("expected identifier"));
        };

        let mut full = self.normalize_name(name);
        let mut end_span = self.current_span();
        self.bump();

        while self.matches(|kind| matches!(kind, TokenKind::Dot)) {
            let TokenKind::Identifier(part) = self.current_kind() else {
                return Err(self.error_here("expected identifier after `.`"));
            };
            full.push('.');
            full.push_str(&self.normalize_name(part));
            end_span = self.current_span();
            self.bump();
        }

        Ok((full, end_span))
    }

    fn parse_signed_integer(&mut self) -> Result<(i64, Span), Diagnostic> {
        let ((_, value), span) = self.parse_signed_integer_with_raw()?;
        Ok((value, span))
    }

    fn parse_signed_integer_with_raw(&mut self) -> Result<((String, i64), Span), Diagnostic> {
        let start = self.current_span();
        let negative = if self.matches(|kind| matches!(kind, TokenKind::Plus)) {
            false
        } else if self.matches(|kind| matches!(kind, TokenKind::Minus)) {
            true
        } else {
            false
        };

        let (value, end_span, raw) = match self.current_kind() {
            TokenKind::Integer { raw, value } => (*value, self.current_span(), raw.clone()),
            _ => return Err(self.error_here("expected integer literal")),
        };
        self.bump();

        let signed = if negative {
            value
                .checked_neg()
                .ok_or_else(|| self.error_span(end_span, "integer literal is out of range"))?
        } else {
            value
        };

        let raw = if negative { format!("-{raw}") } else { raw };

        Ok(((raw, signed), self.merge_spans(start, end_span)))
    }

    fn parse_integer_token(&mut self) -> Result<(i64, Span), Diagnostic> {
        match self.current_kind() {
            TokenKind::Integer { value, .. } => {
                let span = self.current_span();
                let value = *value;
                self.bump();
                Ok((value, span))
            }
            _ => Err(self.error_here("expected integer literal")),
        }
    }

    fn parse_register_name(&self, name: &str) -> Option<Register> {
        match name {
            "r0" => Some(Register::R0),
            "r1" => Some(Register::R1),
            "r2" => Some(Register::R2),
            "r3" => Some(Register::R3),
            "r4" => Some(Register::R4),
            "r5" => Some(Register::R5),
            "r6" => Some(Register::R6),
            "r7" => Some(Register::R7),
            _ => None,
        }
    }

    fn parse_standard_condition_name(&self, name: &str) -> Option<StdCondition> {
        match name {
            "equal" | "zero" => Some(StdCondition::Equal),
            "not_equal" | "not_zero" => Some(StdCondition::NotEqual),
            "lower" => Some(StdCondition::Lower),
            "higher" => Some(StdCondition::Higher),
            "lower_same" => Some(StdCondition::LowerSame),
            "higher_same" | "carry" => Some(StdCondition::HigherSame),
            "even" => Some(StdCondition::Even),
            "always" => Some(StdCondition::Always),
            _ => None,
        }
    }

    fn parse_alternate_condition_name(&self, name: &str) -> Option<AltCondition> {
        match name {
            "overflow" => Some(AltCondition::Overflow),
            "no_overflow" => Some(AltCondition::NoOverflow),
            "less" => Some(AltCondition::Less),
            "greater" => Some(AltCondition::Greater),
            "less_equal" => Some(AltCondition::LessEqual),
            "greater_equal" => Some(AltCondition::GreaterEqual),
            "odd" => Some(AltCondition::Odd),
            "always" => Some(AltCondition::Always),
            _ => None,
        }
    }

    fn normalize_name(&self, name: &str) -> String {
        name.to_ascii_lowercase()
    }

    fn skip_newlines(&mut self) {
        while matches!(self.current_kind(), TokenKind::Newline) {
            self.bump();
        }
    }

    fn expect(&mut self, expected: TokenKind, message: &str) -> Result<(), Diagnostic> {
        if self.current_kind() == &expected {
            self.bump();
            Ok(())
        } else {
            Err(self.error_here(message))
        }
    }

    fn matches(&mut self, predicate: impl FnOnce(&TokenKind) -> bool) -> bool {
        if predicate(self.current_kind()) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn at_statement_end(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Newline | TokenKind::Eof)
    }

    fn at_end(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Eof)
    }

    fn current_kind(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn current_span(&self) -> Span {
        self.tokens[self.pos].span
    }

    fn previous_span(&self) -> Span {
        self.tokens[self.pos.saturating_sub(1)].span
    }

    fn bump(&mut self) {
        if !self.at_end() {
            self.pos += 1;
        }
    }

    fn merge_spans(&self, start: Span, end: Span) -> Span {
        start.merge(end)
    }

    fn error_here(&self, message: impl Into<String>) -> Diagnostic {
        self.error_with_code(
            DiagnosticCode::UnexpectedToken(message.into()),
            self.current_span(),
        )
    }

    fn error_span(&self, span: Span, message: impl Into<String>) -> Diagnostic {
        let message = message.into();
        Diagnostic::error(message.clone()).with_label(DiagnosticLabel::new(span, message))
    }

    fn error_with_code(&self, code: DiagnosticCode, span: Span) -> Diagnostic {
        let message = code.message();
        Diagnostic::error_code(code).with_label(DiagnosticLabel::new(span, message))
    }
}

#[cfg(test)]
mod tests {
    use crate::frontend::syntax::{
        lexer::Tokenizer,
        parser::Parser,
        statements::{
            Address, AltCondition, Condition, DirectiveArg, Operand, Program, Register, Statement,
            StdCondition,
        },
    };

    fn parse(source: &str) -> Program {
        let tokens = Tokenizer::new(0, source).tokenize().unwrap();
        Parser::new(&tokens).parse().unwrap()
    }

    #[test]
    fn parses_labels_and_qualified_mnemonics() {
        let program = parse("start:\nfunc.halt\n");

        assert_eq!(program.statements.len(), 2);
        assert!(matches!(
            &program.statements[0],
            Statement::Label(label) if label.name == "start"
        ));
        assert!(matches!(
            &program.statements[1],
            Statement::Instruction(instruction)
                if instruction.mnemonic == "func.halt" && instruction.operands.is_empty()
        ));
    }

    #[test]
    fn parses_instruction_operands() {
        let program = parse("cmov r7, r6, ?not_equal\naddi r3, -20\nmlx r2, [r3+4]\n");

        assert!(matches!(
            &program.statements[0],
            Statement::Instruction(instruction)
                if instruction.mnemonic == "cmov"
                    && instruction.operands == vec![
                        Operand::Register(Register::R7),
                        Operand::Register(Register::R6),
                        Operand::Condition(Condition::Standard(StdCondition::NotEqual)),
                    ]
        ));

        assert!(matches!(
            &program.statements[1],
            Statement::Instruction(instruction)
                if instruction.mnemonic == "addi"
                    && instruction.operands == vec![
                        Operand::Register(Register::R3),
                        Operand::Immediate(-20),
                    ]
        ));

        assert!(matches!(
            &program.statements[2],
            Statement::Instruction(instruction)
                if instruction.mnemonic == "mlx"
                    && instruction.operands == vec![
                        Operand::Register(Register::R2),
                        Operand::Address(Address::Indexed {
                            base: Register::R3,
                            offset: Some(4),
                        }),
                    ]
        ));
    }

    #[test]
    fn parses_directives() {
        let program = parse(".section text\n.bytes 0x00, 0x1c, 0xff\n.string \"text\"\n");

        assert!(matches!(
            &program.statements[0],
            Statement::Directive(directive)
                if directive.name == "section"
                    && directive.args == vec![DirectiveArg::Identifier("text".to_owned())]
        ));

        assert!(matches!(
            &program.statements[1],
            Statement::Directive(directive)
                if directive.name == "bytes"
                    && directive.args == vec![
                        DirectiveArg::Integer {
                            raw: "0x00".to_owned(),
                            value: 0,
                        },
                        DirectiveArg::Integer {
                            raw: "0x1c".to_owned(),
                            value: 28,
                        },
                        DirectiveArg::Integer {
                            raw: "0xff".to_owned(),
                            value: 255,
                        },
                    ]
        ));

        assert!(matches!(
            &program.statements[2],
            Statement::Directive(directive)
                if directive.name == "string"
                    && directive.args == vec![DirectiveArg::String("text".to_owned())]
        ));
    }

    #[test]
    fn rejects_bang_prefixed_immediates() {
        let tokens = Tokenizer::new(0, "lim r0, !0x10\n").tokenize().unwrap();
        let error = Parser::new(&tokens).parse().unwrap_err();

        assert_eq!(error.message, "expected operand");
    }

    #[test]
    fn parses_alternate_conditions_and_crlf() {
        let program = parse("bra @overflow, target\r\ntarget:\r\n");

        assert!(matches!(
            &program.statements[0],
            Statement::Instruction(instruction)
                if instruction.operands == vec![
                    Operand::Condition(Condition::Alternate(AltCondition::Overflow)),
                    Operand::Label("target".to_owned()),
                ]
        ));
        assert!(matches!(&program.statements[1], Statement::Label(_)));
    }
}
