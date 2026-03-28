use crate::models::diagnostics::{Diagnostic, DiagnosticLabel, FileId, Severity};
use crate::models::tokens::{Token, TokenKind};
use crate::models::Span;

pub fn tokenize(file_id: FileId, source: &str) -> Result<Vec<Token>, Diagnostic> {
    Tokenizer::new(file_id, source).tokenize()
}

struct Tokenizer<'a> {
    file_id: FileId,
    source: &'a str,
    pos: usize,
    tokens: Vec<Token>,
}

impl<'a> Tokenizer<'a> {
    fn new(file_id: FileId, source: &'a str) -> Self {
        Self {
            file_id,
            source,
            pos: 0,
            tokens: Vec::new(),
        }
    }

    fn tokenize(mut self) -> Result<Vec<Token>, Diagnostic> {
        while let Some(ch) = self.peek() {
            match ch {
                ' ' | '\t' => {
                    self.bump();
                }
                '\n' => {
                    let start = self.pos;
                    self.bump();
                    self.push(TokenKind::Newline, start, self.pos);
                }
                '\r' => {
                    let start = self.pos;
                    self.bump();
                    if self.peek() == Some('\n') {
                        self.bump();
                    }
                    self.push(TokenKind::Newline, start, self.pos);
                }
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
                    let token = self.lex_char()?;
                    self.tokens.push(token);
                }
                '"' => {
                    let token = self.lex_string()?;
                    self.tokens.push(token);
                }
                '0'..='9' => {
                    let token = self.lex_integer()?;
                    self.tokens.push(token);
                }
                _ if is_identifier_start(ch) => {
                    let token = self.lex_identifier();
                    self.tokens.push(token);
                }
                _ => {
                    return Err(self.error_at(
                        self.pos,
                        self.pos + ch.len_utf8(),
                        format!("unexpected character `{ch}`"),
                    ));
                }
            }
        }

        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span: self.span(self.pos, self.pos),
        });

        Ok(self.tokens)
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
        let mut base = 10;

        if self.peek() == Some('0') {
            match self.peek_next() {
                Some('x') | Some('X') => {
                    base = 16;
                    self.bump();
                    self.bump();
                }
                Some('b') | Some('B') => {
                    base = 2;
                    self.bump();
                    self.bump();
                }
                _ => {}
            }
        }

        let digits_start = self.pos;
        let mut digits = String::new();

        while let Some(ch) = self.peek() {
            if ch == '_' {
                self.bump();
                continue;
            }

            if is_digit_for_base(ch, base) {
                digits.push(ch);
                self.bump();
                continue;
            }

            break;
        }

        if digits.is_empty() {
            let raw_end = self.pos.max(start + 1);
            return Err(self.error_at(
                start,
                raw_end,
                match base {
                    16 => "expected at least one hexadecimal digit after `0x`".to_owned(),
                    2 => "expected at least one binary digit after `0b`".to_owned(),
                    _ => "expected digits".to_owned(),
                },
            ));
        }

        let raw = self.source[start..self.pos].to_owned();
        let value = i64::from_str_radix(&digits, base).map_err(|_| {
            self.error_at(
                start,
                self.pos,
                format!("integer literal `{raw}` is out of range for i64"),
            )
        })?;

        if matches!(self.peek(), Some(ch) if is_identifier_start(ch)) {
            return Err(self.error_at(
                digits_start,
                self.pos + self.peek().map(char::len_utf8).unwrap_or(0),
                format!("invalid integer literal `{}`", &self.source[start..self.pos + self.peek().map(char::len_utf8).unwrap_or(0)]),
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

        let raw = match self.peek() {
            Some('\\') => {
                self.bump();
                self.lex_escape(start)?
            }
            Some('\n') | Some('\r') | None => {
                return Err(self.error_at(start, self.pos, "unterminated character literal".to_owned()));
            }
            Some(ch) => {
                self.bump();
                ch
            }
        };

        match self.peek() {
            Some('\'') => {
                self.bump();
            }
            Some(_) => {
                return Err(self.error_at(
                    start,
                    self.pos,
                    "character literal must contain exactly one character".to_owned(),
                ));
            }
            None => {
                return Err(self.error_at(start, self.pos, "unterminated character literal".to_owned()));
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
                Some('\\') => {
                    self.bump();
                    value.push(self.lex_escape(start)?);
                }
                Some('\n') | Some('\r') | None => {
                    return Err(self.error_at(start, self.pos, "unterminated string literal".to_owned()));
                }
                Some(ch) => {
                    self.bump();
                    value.push(ch);
                }
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
                self.pos,
                self.pos + ch.len_utf8(),
                format!("unsupported escape sequence `\\{ch}`"),
            )),
            None => Err(self.error_at(
                literal_start,
                self.pos,
                "unterminated escape sequence".to_owned(),
            )),
        }
    }

    fn single(&mut self, kind: TokenKind) {
        let start = self.pos;
        self.bump();
        self.push(kind, start, self.pos);
    }

    fn push(&mut self, kind: TokenKind, start: usize, end: usize) {
        self.tokens.push(Token {
            kind,
            span: self.span(start, end),
        });
    }

    fn span(&self, start: usize, end: usize) -> Span {
        Span {
            file_id: self.file_id,
            start,
            end,
        }
    }

    fn error_at(&self, start: usize, end: usize, message: String) -> Diagnostic {
        Diagnostic {
            severity: Severity::Error,
            message: message.clone(),
            labels: vec![DiagnosticLabel {
                span: self.span(start, end),
                message,
            }],
        }
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
    use super::tokenize;
    use crate::models::tokens::TokenKind;

    #[test]
    fn tokenizes_instruction_line() {
        let tokens = tokenize(0, "mld r1, [0x10]\n").unwrap();
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
        let tokens = tokenize(0, "'A' \"hi\\n\"").unwrap();
        let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Char { raw: 'A', value: 65 },
                TokenKind::String("hi\n".to_owned()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn rejects_invalid_char_literals() {
        let error = tokenize(0, "'ab'").unwrap_err();
        assert_eq!(error.message, "character literal must contain exactly one character");
    }

    #[test]
    fn rejects_comment_sigil_in_tokenizer_input() {
        let error = tokenize(0, "jmp label ; branch\n").unwrap_err();
        assert_eq!(error.message, "unexpected character `;`");
    }
}
