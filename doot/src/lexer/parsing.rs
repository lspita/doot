use std::{
    error::Error,
    fmt::Display,
    num::{IntErrorKind, ParseFloatError, ParseIntError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumberParseError {
    InvalidInt,
    InvalidRadix(String),
    PositiveOverflow,
    NegativeOverflow,
    InvalidFloat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscapeParseError {
    NoValue(String),
    InvalidEscape(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnicodeParseError {
    InvalidHex(String),
    InvalidValue(String),
}

impl Error for NumberParseError {}
impl Display for NumberParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NumberParseError::InvalidRadix(radix) => format!("invalid number radix {}", radix),
            NumberParseError::InvalidInt => "invalid digits found".to_string(),
            NumberParseError::PositiveOverflow => "positive overflow".to_string(),
            NumberParseError::NegativeOverflow => "negative overflow".to_string(),
            NumberParseError::InvalidFloat => "invalid float".to_string(),
        }
        .fmt(f)
    }
}

pub(super) fn replace_escape(source: &str) -> Result<char, EscapeParseError> {
    // https://doc.rust-lang.org/reference/tokens.html#ascii-escapes
    match source {
        r"\n" => Ok('\n'),
        r"\r" => Ok('\r'),
        r"\t" => Ok('\t'),
        r"\\" => Ok('\\'),
        r"\0" => Ok('\0'),
        c if c == r"\u" => Err(EscapeParseError::NoValue(c.to_string())),
        source => Err(EscapeParseError::InvalidEscape(source.to_string())),
    }
}

pub(super) fn parse_unicode(hex_string: &str) -> Result<char, UnicodeParseError> {
    u32::from_str_radix(hex_string, 16).map_or(
        Err(UnicodeParseError::InvalidHex(hex_string.to_string())),
        |code_point| {
            char::from_u32(code_point)
                .ok_or(UnicodeParseError::InvalidValue(hex_string.to_string()))
        },
    )
}

fn map_int_error(err: ParseIntError) -> NumberParseError {
    match err.kind() {
        IntErrorKind::Empty => panic!(),
        IntErrorKind::InvalidDigit => NumberParseError::InvalidInt,
        IntErrorKind::PosOverflow => NumberParseError::PositiveOverflow,
        IntErrorKind::NegOverflow => NumberParseError::NegativeOverflow,
        IntErrorKind::Zero => panic!(),
        _ => panic!(),
    }
}

fn map_float_error(_: ParseFloatError) -> NumberParseError {
    NumberParseError::InvalidFloat
}

pub(super) fn parse_int(source: &str) -> Result<i64, NumberParseError> {
    let mut chars = source.chars();
    match chars.next() {
        Some('0') => {
            if let Some(radix) = chars.next() {
                match radix {
                    'b' => Ok(2),
                    'o' => Ok(8),
                    'x' => Ok(16),
                    c if c.is_ascii_digit() => Ok(10),
                    _ => Err(NumberParseError::InvalidRadix(radix.to_string())),
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
        Some(_) | None => Err(NumberParseError::InvalidInt),
    }
}

pub(super) fn parse_float(source: &str) -> Result<f64, NumberParseError> {
    source.parse().map_err(map_float_error)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::lexer::parsing::{NumberParseError, parse_int, parse_unicode};

    use super::UnicodeParseError;

    #[rstest]
    #[case("0", '\0')] // null
    #[case("41", 'A')] // ascii
    #[case("20AC", '€')] // sign
    #[case("1F600", '😀')] // emoji
    #[case("2764", '❤')] // text emoji
    #[case("D2EE", '\u{D2EE}')] // from bytes
    #[case("10FFFF", '\u{10FFFF}')] // maximum
    fn unicode_ok(#[case] source: &str, #[case] expected: char) {
        let result = parse_unicode(source);
        assert!(result.is_ok());
        assert_eq!(expected, result.unwrap());
    }

    #[rstest]
    #[case("l1aa", UnicodeParseError::InvalidHex("l1aa".to_string()))] // invalid hex
    #[case("FFFFFF", UnicodeParseError::InvalidValue("FFFFFF".to_string()))] // out of range of unicode code points
    fn unicode_fail(#[case] source: &str, #[case] expected: UnicodeParseError) {
        let result = parse_unicode(source);
        assert!(result.is_err());
        assert_eq!(expected, result.unwrap_err());
    }

    #[rstest]
    #[case("0", 0)]
    #[case("123", 123)]
    #[case("00123", 123)]
    #[case("0b1100", 12)] // binary
    #[case("0o074", 60)] // octal
    #[case("0xFFAA", 65450)] // hex
    fn int_ok(#[case] source: &str, #[case] expected: i64) {
        let result = parse_int(source);
        assert!(result.is_ok());
        assert_eq!(expected, result.unwrap());
    }

    #[rstest]
    #[case("abc", NumberParseError::InvalidInt)]
    fn int_fail(#[case] source: &str, #[case] expected: NumberParseError) {
        let result = parse_int(source);
        assert!(result.is_err());
        assert_eq!(expected, result.unwrap_err());
    }
}
