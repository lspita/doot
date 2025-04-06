use std::{
    error::Error,
    fmt::Display,
    num::{ParseFloatError, ParseIntError},
};

use super::TokenizationError;

#[derive(Debug, Clone)]
pub enum NumberParseError {
    InvalidDigit,
    PositiveOverflow,
    NegativeOverflow,
    InvalidFloat,
}

impl Error for NumberParseError {}
impl Display for NumberParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NumberParseError::InvalidDigit => "invalid digits found",
            NumberParseError::PositiveOverflow => "positive overflow",
            NumberParseError::NegativeOverflow => "negative overflow",
            NumberParseError::InvalidFloat => "invalid float",
        }
        .fmt(f)
    }
}

pub(super) fn replace_escape(source: &str) -> Result<char, TokenizationError> {
    // https://doc.rust-lang.org/reference/tokens.html#ascii-escapes
    match source {
        r"\n" => Ok('\n'),
        r"\r" => Ok('\r'),
        r"\t" => Ok('\t'),
        r"\\" => Ok('\\'),
        r"\0" => Ok('\0'),
        c if c == r"\u" => Err(TokenizationError::NoEscapeValue(c.to_string())),
        source => Err(TokenizationError::InvalidEscape(source.to_string())),
    }
}

pub(super) fn parse_unicode(source: &str) -> Result<char, TokenizationError> {
    u32::from_str_radix(source, 16).map_or(
        Err(TokenizationError::InvalidHex(source.to_string())),
        |code_point| {
            char::from_u32(code_point).ok_or(TokenizationError::InvalidUnicode(source.to_string()))
        },
    )
}

fn map_int_error(err: ParseIntError) -> TokenizationError {
    match err.kind() {
        std::num::IntErrorKind::Empty => panic!(),
        std::num::IntErrorKind::InvalidDigit => {
            TokenizationError::NumberParseError(NumberParseError::InvalidDigit)
        }
        std::num::IntErrorKind::PosOverflow => {
            TokenizationError::NumberParseError(NumberParseError::PositiveOverflow)
        }
        std::num::IntErrorKind::NegOverflow => {
            TokenizationError::NumberParseError(NumberParseError::NegativeOverflow)
        }
        std::num::IntErrorKind::Zero => panic!(),
        _ => todo!(),
    }
}

fn map_float_error(_: ParseFloatError) -> TokenizationError {
    TokenizationError::NumberParseError(NumberParseError::InvalidFloat)
}

pub(super) fn parse_int(source: &str) -> Result<i64, TokenizationError> {
    let mut chars = source.chars();
    match chars.next() {
        Some('0') => {
            if let Some(radix) = chars.next() {
                match radix {
                    'b' => Ok(2),
                    'o' => Ok(8),
                    'x' => Ok(16),
                    c if c.is_ascii_digit() => Ok(10),
                    _ => Err(TokenizationError::InvalidNumberRadix(radix)),
                }
                .and_then(|radix| {
                    i64::from_str_radix(&source[if radix == 10 { 0 } else { 2 }..], radix)
                        .map_err(map_int_error)
                })
            } else {
                Ok(0)
            }
        }
        Some('1'..'9') => source.parse().map_err(map_int_error),
        Some(_) | None => panic!(),
    }
}

pub(super) fn parse_float(source: &str) -> Result<f64, TokenizationError> {
    source.parse().map_err(map_float_error)
}
