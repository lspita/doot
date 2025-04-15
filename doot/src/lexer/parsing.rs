use std::{error::Error, fmt::Display};

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

impl Error for EscapeParseError {}
impl Display for EscapeParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EscapeParseError::NoValue(escape) => format!("missing value for escape {}", escape),
            EscapeParseError::InvalidEscape(escape) => format!("invalid escape {}", escape),
        }
        .fmt(f)
    }
}

impl Error for UnicodeParseError {}
impl Display for UnicodeParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnicodeParseError::InvalidHex(hex) => format!("invalid hex value {}", hex),
            UnicodeParseError::InvalidValue(unicode) => {
                format!("invalid unicode value {}", unicode)
            }
        }
        .fmt(f)
    }
}

pub(super) fn escape(source: &str) -> Result<char, EscapeParseError> {
    // https://doc.rust-lang.org/reference/tokens.html#ascii-escapes
    match source {
        r"\n" => Ok('\n'),
        r"\r" => Ok('\r'),
        r"\t" => Ok('\t'),
        r"\\" => Ok('\\'),
        r"\0" => Ok('\0'),
        r"\$" => Ok('$'), // escape \$ because of ${ in strings
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

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{EscapeParseError, UnicodeParseError, escape, parse_unicode};

    #[rstest]
    #[case(r"\n", '\n')]
    #[case(r"\r", '\r')]
    #[case(r"\t", '\t')]
    #[case(r"\\", '\\')]
    #[case(r"\0", '\0')]
    #[case(r"\$", '$')]
    fn escape_ok(#[case] source: &str, #[case] expected: char) {
        let result = escape(source);
        assert!(result.is_ok());
        assert_eq!(expected, result.unwrap());
    }

    #[rstest]
    #[case(r"\u", EscapeParseError::NoValue(r"\u".to_string()))]
    #[case(r"\a", EscapeParseError::InvalidEscape(r"\a".to_string()))]
    fn escape_fail(#[case] source: &str, #[case] expected: EscapeParseError) {
        let result = escape(source);
        assert!(result.is_err());
        assert_eq!(expected, result.unwrap_err());
    }

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
}
