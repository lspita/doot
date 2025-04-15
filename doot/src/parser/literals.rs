use core::f64;
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

fn clean_source(source: &str) -> String {
    source.replace("_", "")
}

pub(super) fn parse_int(source: &str) -> Result<i64, NumberParseError> {
    let cleaned = clean_source(source);
    let mut source = cleaned.as_str();
    let mut chars = source.chars().peekable();
    let sign = match chars.peek().cloned() {
        Some(c) if c == '+' || c == '-' => {
            chars.next();
            source = &source[1..];
            c
        }
        Some('0'..='9') => '+',
        Some(_) | None => return Err(NumberParseError::InvalidInt),
    };
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
                    i64::from_str_radix(
                        &format!(
                            "{}{}",
                            sign,
                            source[if radix == 10 { 0 } else { 2 }..].to_string()
                        ),
                        radix,
                    )
                    .map_err(map_int_error)
                })
            } else {
                Ok(0)
            }
        }
        Some('1'..='9') => format!("{}{}", sign, source).parse().map_err(map_int_error),
        Some(_) | None => Err(NumberParseError::InvalidInt),
    }
}

pub(super) fn parse_float(source: &str) -> Result<f64, NumberParseError> {
    clean_source(source)
        .parse()
        .map_err(map_float_error)
        .and_then(|val| {
            if val > f64::MAX {
                Err(NumberParseError::PositiveOverflow)
            } else if val < -f64::MAX {
                Err(NumberParseError::NegativeOverflow)
            } else {
                Ok(val)
            }
        })
}

#[cfg(test)]
mod tests {
    use core::f64;

    use rstest::rstest;

    use super::{NumberParseError, parse_float, parse_int};

    #[rstest]
    #[case("0", 0)]
    #[case("-0", 0)]
    #[case("123", 123)]
    #[case("-123", -123)]
    #[case("00123", 123)]
    #[case("-00123", -123)]
    #[case(&i64::MAX.to_string(), i64::MAX)]
    #[case(&(i64::MIN).to_string(), i64::MIN)]
    #[case("0b1100", 12)] // binary
    #[case("-0b1100", -12)] // binary
    #[case("0o074", 60)] // octal
    #[case("-0o074", -60)] // octal
    #[case("0xFFAA", 65450)] // hex
    #[case("-0xFFAA", -65450)] // hex
    #[case("0xffaa", 65450)] // hex
    #[case("_1", 1)]
    #[case("1_", 1)]
    #[case("1___2_", 12)]
    fn int_ok(#[case] source: &str, #[case] expected: i64) {
        let result = parse_int(source);
        assert!(result.is_ok());
        assert_eq!(expected, result.unwrap());
    }

    #[rstest]
    #[case("abc", NumberParseError::InvalidInt)]
    #[case("-abc", NumberParseError::InvalidInt)]
    #[case("abc123", NumberParseError::InvalidInt)]
    #[case("-abc123", NumberParseError::InvalidInt)]
    #[case("0a123", NumberParseError::InvalidRadix("a".to_string()))]
    #[case("-0a123", NumberParseError::InvalidRadix("a".to_string()))]
    #[case("0abc123", NumberParseError::InvalidRadix("a".to_string()))]
    #[case("-0abc123", NumberParseError::InvalidRadix("a".to_string()))]
    #[case(&(i64::MAX as i128 + 1).to_string(), NumberParseError::PositiveOverflow)]
    #[case(&(i64::MIN as i128 - 1).to_string(), NumberParseError::NegativeOverflow)]
    #[case(
        "0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
        NumberParseError::PositiveOverflow
    )]
    #[case(
        "-0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
        NumberParseError::NegativeOverflow
    )]
    fn int_fail(#[case] source: &str, #[case] expected: NumberParseError) {
        let result = parse_int(source);
        assert!(result.is_err());
        assert_eq!(expected, result.unwrap_err());
    }

    #[rstest]
    #[case("123.45", 123.45)]
    #[case("-123.45", -123.45)]
    #[case("0.45", 0.45)]
    #[case(".45", 0.45)]
    #[case("-0.45", -0.45)]
    #[case("_123.45", 123.45)]
    #[case("123__.45", 123.45)]
    #[case("1.2e3", 1200.0)]
    #[case(&(f64::MAX).to_string(), f64::MAX)]
    #[case(&(-f64::MAX).to_string(), -f64::MAX)]
    #[case(&(f64::MIN).to_string(), f64::MIN)]
    #[case(&(-f64::MIN).to_string(), -f64::MIN)]
    fn float_ok(#[case] source: &str, #[case] expected: f64) {
        let result = parse_float(source);
        assert!(result.is_ok());
        assert_eq!(expected, result.unwrap());
    }

    #[rstest]
    #[case("abc", NumberParseError::InvalidFloat)]
    #[case(&(f64::MAX * 2.0).to_string(), NumberParseError::PositiveOverflow)]
    #[case(&(-f64::MAX * 2.0).to_string(), NumberParseError::NegativeOverflow)]
    fn float_fail(#[case] source: &str, #[case] expected: NumberParseError) {
        let result = parse_float(source);
        assert!(result.is_err());
        assert_eq!(expected, result.unwrap_err());
    }
}
