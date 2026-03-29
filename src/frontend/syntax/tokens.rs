use crate::diagnostics::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Identifier(String),
    Integer { raw: String, value: i64 },
    String(String),
    Char { raw: char, value: i64 },

    Dot,
    Comma,
    Colon,
    At,
    Question,
    Excl,
    LBracket,
    RBracket,
    Plus,
    Minus,

    Newline,
    Eof,
}
