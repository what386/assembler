#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DiagnosticCode {
    UnexpectedCharacter(char),
    ExpectedHexDigitsAfterPrefix,
    ExpectedBinaryDigitsAfterPrefix,
    ExpectedDigits,
    InvalidIntegerLiteral(String),
    IntegerOutOfRange(String),
    InvalidCharacterLiteralLength,
    UnterminatedCharacterLiteral,
    UnterminatedStringLiteral,
    UnsupportedEscapeSequence(char),
    UnterminatedEscapeSequence,
    UnexpectedToken(String),
    InvalidOperand(String),
    InvalidDirective(String),
    UnknownRegister(String),
    UnknownCondition(String),
    EncodingError(String),
}

impl DiagnosticCode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::UnexpectedCharacter(_) => "E0001",
            Self::ExpectedHexDigitsAfterPrefix
            | Self::ExpectedBinaryDigitsAfterPrefix
            | Self::ExpectedDigits
            | Self::InvalidIntegerLiteral(_)
            | Self::IntegerOutOfRange(_) => "E0002",
            Self::InvalidCharacterLiteralLength | Self::UnterminatedCharacterLiteral => "E0003",
            Self::UnterminatedStringLiteral => "E0004",
            Self::UnsupportedEscapeSequence(_) | Self::UnterminatedEscapeSequence => "E0005",
            Self::UnexpectedToken(_) => "E0006",
            Self::InvalidOperand(_) => "E0007",
            Self::InvalidDirective(_) => "E0008",
            Self::UnknownRegister(_) => "E0009",
            Self::UnknownCondition(_) => "E0010",
            Self::EncodingError(_) => "E0011",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::UnexpectedCharacter(ch) => format!("unexpected character `{ch}`"),
            Self::ExpectedHexDigitsAfterPrefix => {
                "expected at least one hexadecimal digit after `0x`".to_owned()
            }
            Self::ExpectedBinaryDigitsAfterPrefix => {
                "expected at least one binary digit after `0b`".to_owned()
            }
            Self::ExpectedDigits => "expected digits".to_owned(),
            Self::InvalidIntegerLiteral(raw) => format!("invalid integer literal `{raw}`"),
            Self::IntegerOutOfRange(raw) => {
                format!("integer literal `{raw}` is out of range for i64")
            }
            Self::InvalidCharacterLiteralLength => {
                "character literal must contain exactly one character".to_owned()
            }
            Self::UnterminatedCharacterLiteral => "unterminated character literal".to_owned(),
            Self::UnterminatedStringLiteral => "unterminated string literal".to_owned(),
            Self::UnsupportedEscapeSequence(ch) => {
                format!("unsupported escape sequence `\\{ch}`")
            }
            Self::UnterminatedEscapeSequence => "unterminated escape sequence".to_owned(),
            Self::UnexpectedToken(message)
            | Self::InvalidOperand(message)
            | Self::InvalidDirective(message)
            | Self::UnknownRegister(message)
            | Self::UnknownCondition(message)
            | Self::EncodingError(message) => message.clone(),
        }
    }
}
