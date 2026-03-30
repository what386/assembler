use crate::{
    diagnostics::{Diagnostic, DiagnosticCode, DiagnosticEmitter, Partial, Span},
    frontend::syntax::tokens::{Token, TokenKind},
};

type FileId = u64;

pub struct Tokenizer<'a> {
    file_id: FileId,
    source: &'a str,
    pos: usize,
    tokens: Vec<Token>,
}

impl<'a> Tokenizer<'a> {
    pub fn new(file_id: FileId, source: &'a str) -> Self {
        Self {
            file_id,
            source,
            pos: 0,
            tokens: Vec::new(),
        }
    }

    pub fn tokenize(mut self) -> Partial<Vec<Token>> {
        let mut emitter = DiagnosticEmitter::new();

        while let Some(ch) = self.peek() {
            match ch {
                ' ' | '\t' => {
                    self.bump();
                }
                '\n' | '\r' => self.lex_newline(),
                '.' => self.single(TokenKind::Dot),
                ',' => self.single(TokenKind::Comma),
                ':' => self.single(TokenKind::Colon),
                '@' => self.single(TokenKind::At),
                '?' => self.single(TokenKind::Question),
                '!' => self.single(TokenKind::Excl),
                '[' => self.single(TokenKind::LBracket),
                ']' => self.single(TokenKind::RBracket),
                '+' => self.single(TokenKind::Plus),
                '-' => self.single(TokenKind::Minus),
                '\'' => {
                    let token = self.lex_char();
                    self.push_or_recover(token, &mut emitter);
                }
                '"' => {
                    let token = self.lex_string();
                    self.push_or_recover(token, &mut emitter);
                }
                '0'..='9' => {
                    let token = self.lex_integer();
                    self.push_or_recover(token, &mut emitter);
                }
                _ if is_identifier_start(ch) => {
                    let token = self.lex_identifier();
                    self.push_token(token);
                }
                _ => {
                    emitter.push(self.error_at(
                        DiagnosticCode::UnexpectedCharacter(ch),
                        self.pos,
                        self.pos + ch.len_utf8(),
                    ));
                    self.recover_line();
                }
            }
        }

        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span: self.span(self.pos, self.pos),
        });

        emitter.finish(self.tokens)
    }

    fn lex_newline(&mut self) {
        let start = self.pos;
        let ch = self.bump();

        if matches!(ch, Some('\r')) && matches!(self.source.as_bytes().get(self.pos), Some(b'\n')) {
            self.bump();
        }

        self.push(TokenKind::Newline, start, self.pos);
    }

    fn lex_identifier(&mut self) -> Token {
        let start = self.pos;
        self.bump();

        while let Some(ch) = self.peek() {
            if is_identifier_continue(ch) {
                self.bump();
            } else {
                break;
            }
        }

        Token {
            kind: TokenKind::Identifier(self.source[start..self.pos].to_owned()),
            span: self.span(start, self.pos),
        }
    }

    fn lex_integer(&mut self) -> Result<Token, Diagnostic> {
        let start = self.pos;
        let base = self.consume_integer_prefix();

        let digits_start = self.pos;
        let digits = self.collect_integer_digits(base);

        if digits.is_empty() {
            let raw_end = self.pos.max(start + 1);
            return Err(self.error_at(
                match base {
                    16 => DiagnosticCode::ExpectedHexDigitsAfterPrefix,
                    2 => DiagnosticCode::ExpectedBinaryDigitsAfterPrefix,
                    _ => DiagnosticCode::ExpectedDigits,
                },
                start,
                raw_end,
            ));
        }

        let raw = self.source[start..self.pos].to_owned();
        let value = self.parse_integer_value(start, &raw, &digits, base)?;

        if let Some(suffix_end) = self.invalid_integer_suffix_end() {
            return Err(self.error_at(
                DiagnosticCode::InvalidIntegerLiteral(self.source[start..suffix_end].to_owned()),
                digits_start,
                suffix_end,
            ));
        }

        Ok(Token {
            kind: TokenKind::Integer { raw, value },
            span: self.span(start, self.pos),
        })
    }

    fn lex_char(&mut self) -> Result<Token, Diagnostic> {
        let start = self.pos;
        self.bump();

        let raw = self.lex_literal_char(start, "character literal")?;

        match self.peek() {
            Some('\'') => {
                self.bump();
            }
            Some(_) => {
                return Err(self.error_at(
                    DiagnosticCode::InvalidCharacterLiteralLength,
                    start,
                    self.pos,
                ));
            }
            None => {
                return Err(self.error_at(
                    DiagnosticCode::UnterminatedCharacterLiteral,
                    start,
                    self.pos,
                ));
            }
        }

        Ok(Token {
            kind: TokenKind::Char {
                raw,
                value: raw as i64,
            },
            span: self.span(start, self.pos),
        })
    }

    fn lex_string(&mut self) -> Result<Token, Diagnostic> {
        let start = self.pos;
        self.bump();

        let mut value = String::new();

        loop {
            match self.peek() {
                Some('"') => {
                    self.bump();
                    break;
                }
                Some('\n') | Some('\r') | None => {
                    return Err(self.error_at(
                        DiagnosticCode::UnterminatedStringLiteral,
                        start,
                        self.pos,
                    ));
                }
                Some(_) => value.push(self.lex_literal_char(start, "string literal")?),
            }
        }

        Ok(Token {
            kind: TokenKind::String(value),
            span: self.span(start, self.pos),
        })
    }

    fn lex_escape(&mut self, literal_start: usize) -> Result<char, Diagnostic> {
        match self.peek() {
            Some('n') => {
                self.bump();
                Ok('\n')
            }
            Some('r') => {
                self.bump();
                Ok('\r')
            }
            Some('t') => {
                self.bump();
                Ok('\t')
            }
            Some('0') => {
                self.bump();
                Ok('\0')
            }
            Some('\'') => {
                self.bump();
                Ok('\'')
            }
            Some('"') => {
                self.bump();
                Ok('"')
            }
            Some('\\') => {
                self.bump();
                Ok('\\')
            }
            Some(ch) => Err(self.error_at(
                DiagnosticCode::UnsupportedEscapeSequence(ch),
                self.pos,
                self.pos + ch.len_utf8(),
            )),
            None => Err(self.error_at(
                DiagnosticCode::UnterminatedEscapeSequence,
                literal_start,
                self.pos,
            )),
        }
    }

    fn lex_literal_char(
        &mut self,
        literal_start: usize,
        literal_kind: &str,
    ) -> Result<char, Diagnostic> {
        match self.peek() {
            Some('\\') => {
                self.bump();
                self.lex_escape(literal_start)
            }
            Some('\n') | Some('\r') | None => Err(self.error_at(
                match literal_kind {
                    "string literal" => DiagnosticCode::UnterminatedStringLiteral,
                    _ => DiagnosticCode::UnterminatedCharacterLiteral,
                },
                literal_start,
                self.pos,
            )),
            Some(ch) => {
                self.bump();
                Ok(ch)
            }
        }
    }

    fn consume_integer_prefix(&mut self) -> u32 {
        if self.peek() == Some('0') {
            match self.peek_next() {
                Some('x') | Some('X') => {
                    self.bump();
                    self.bump();
                    return 16;
                }
                Some('b') | Some('B') => {
                    self.bump();
                    self.bump();
                    return 2;
                }
                _ => {}
            }
        }

        10
    }

    fn collect_integer_digits(&mut self, base: u32) -> String {
        let mut digits = String::new();

        while let Some(ch) = self.peek() {
            if ch == '_' {
                self.bump();
            } else if is_digit_for_base(ch, base) {
                digits.push(ch);
                self.bump();
            } else {
                break;
            }
        }

        digits
    }

    fn parse_integer_value(
        &self,
        start: usize,
        raw: &str,
        digits: &str,
        base: u32,
    ) -> Result<i64, Diagnostic> {
        i64::from_str_radix(digits, base).map_err(|_| {
            self.error_at(
                DiagnosticCode::IntegerOutOfRange(raw.to_owned()),
                start,
                self.pos,
            )
        })
    }

    fn invalid_integer_suffix_end(&self) -> Option<usize> {
        let ch = self.peek()?;

        if is_identifier_start(ch) {
            Some(self.pos + ch.len_utf8())
        } else {
            None
        }
    }

    fn single(&mut self, kind: TokenKind) {
        let start = self.pos;
        self.bump();
        self.push(kind, start, self.pos);
    }

    fn push_token(&mut self, token: Token) {
        self.tokens.push(token);
    }

    fn push_or_recover(
        &mut self,
        token: Result<Token, Diagnostic>,
        emitter: &mut DiagnosticEmitter,
    ) {
        match token {
            Ok(token) => self.push_token(token),
            Err(diagnostic) => {
                emitter.push(diagnostic);
                self.recover_line();
            }
        }
    }

    fn push(&mut self, kind: TokenKind, start: usize, end: usize) {
        self.tokens.push(Token {
            kind,
            span: self.span(start, end),
        });
    }

    fn span(&self, start: usize, end: usize) -> Span {
        Span::new(self.file_id, start, end)
    }

    fn error_at(&self, code: DiagnosticCode, start: usize, end: usize) -> Diagnostic {
        let message = code.message();
        Diagnostic::error_code(code).with_span_label(self.span(start, end), message)
    }

    fn peek(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        let mut chars = self.source[self.pos..].chars();
        chars.next()?;
        chars.next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn recover_line(&mut self) {
        while let Some(ch) = self.peek() {
            if matches!(ch, '\n' | '\r') {
                self.lex_newline();
                return;
            }
            self.bump();
        }
    }
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_digit_for_base(ch: char, base: u32) -> bool {
    match base {
        2 => matches!(ch, '0' | '1'),
        10 => ch.is_ascii_digit(),
        16 => ch.is_ascii_hexdigit(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::frontend::syntax::{lexer::Tokenizer, tokens::TokenKind};

    #[test]
    fn tokenizes_instruction_line() {
        let tokens = Tokenizer::new(0, "mld r1, [0x10]\n")
            .tokenize()
            .into_result()
            .unwrap();
        let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Identifier("mld".to_owned()),
                TokenKind::Identifier("r1".to_owned()),
                TokenKind::Comma,
                TokenKind::LBracket,
                TokenKind::Integer {
                    raw: "0x10".to_owned(),
                    value: 16,
                },
                TokenKind::RBracket,
                TokenKind::Newline,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn tokenizes_char_and_string_literals() {
        let tokens = Tokenizer::new(0, "'A' \"hi\\n\"")
            .tokenize()
            .into_result()
            .unwrap();
        let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Char {
                    raw: 'A',
                    value: 65
                },
                TokenKind::String("hi\n".to_owned()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn tokenizes_binary_integer_and_escaped_char() {
        let tokens = Tokenizer::new(0, "'\\n' 0b1011")
            .tokenize()
            .into_result()
            .unwrap();
        let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Char {
                    raw: '\n',
                    value: 10,
                },
                TokenKind::Integer {
                    raw: "0b1011".to_owned(),
                    value: 11,
                },
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn rejects_invalid_char_literals() {
        let errors = Tokenizer::new(0, "'ab'")
            .tokenize()
            .into_result()
            .unwrap_err();
        assert_eq!(
            errors[0].message,
            "character literal must contain exactly one character"
        );
    }

    #[test]
    fn rejects_unterminated_character_literal() {
        let errors = Tokenizer::new(0, "'a")
            .tokenize()
            .into_result()
            .unwrap_err();
        assert_eq!(errors[0].message, "unterminated character literal");
    }

    #[test]
    fn rejects_comment_sigil_in_tokenizer_input() {
        let errors = Tokenizer::new(0, "jmp label ; branch\n")
            .tokenize()
            .into_result()
            .unwrap_err();
        assert_eq!(errors[0].message, "unexpected character `;`");
    }

    #[test]
    fn tokenizes_crlf_as_one_newline() {
        let tokens = Tokenizer::new(0, "mld\r\n")
            .tokenize()
            .into_result()
            .unwrap();
        let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Identifier("mld".to_owned()),
                TokenKind::Newline,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn rejects_integer_missing_digits_after_prefix() {
        let errors = Tokenizer::new(0, "0x")
            .tokenize()
            .into_result()
            .unwrap_err();
        assert_eq!(
            errors[0].message,
            "expected at least one hexadecimal digit after `0x`"
        );
        assert_eq!(errors[0].labels[0].span.start, 0);
        assert_eq!(errors[0].labels[0].span.end, 2);
    }

    #[test]
    fn rejects_invalid_integer_suffix() {
        let errors = Tokenizer::new(0, "123abc")
            .tokenize()
            .into_result()
            .unwrap_err();
        assert_eq!(errors[0].message, "invalid integer literal `123a`");
        assert_eq!(errors[0].labels[0].span.start, 0);
        assert_eq!(errors[0].labels[0].span.end, 4);
    }

    #[test]
    fn rejects_unsupported_escape_sequence() {
        let errors = Tokenizer::new(0, "\"\\q\"")
            .tokenize()
            .into_result()
            .unwrap_err();
        assert_eq!(errors[0].message, "unsupported escape sequence `\\q`");
    }

    #[test]
    fn rejects_unterminated_string_literal() {
        let errors = Tokenizer::new(0, "\"hi")
            .tokenize()
            .into_result()
            .unwrap_err();
        assert_eq!(errors[0].message, "unterminated string literal");
    }

    #[test]
    fn recovers_after_multiple_lex_errors() {
        let tokenized = Tokenizer::new(0, "\"\\q\"\n#\nhalt\n").tokenize();

        assert_eq!(tokenized.diagnostics.len(), 2);
        let tokens = tokenized.value.unwrap();
        assert!(tokens.iter().any(|token| {
            matches!(token.kind, TokenKind::Identifier(ref name) if name == "halt")
        }));
    }
}
