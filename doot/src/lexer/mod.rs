use std::{char, error::Error, fmt::Display};

use parsing::NumberParseError;
use state::StateManager;
use tokens::Token;

mod matchers;
mod parsing;
mod state;
pub mod tokens;

#[derive(Debug, Clone)]
pub enum TokenizationError {
    InvalidToken(String),
    InvalidEscape(String),
    NoEscapeChar,
    NoEscapeValue(String),
    InvalidHex(String),
    InvalidUnicode(String),
    InvalidNumberRadix(char),
    NumberParseError(NumberParseError),
}

impl Error for TokenizationError {}
impl Display for TokenizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenizationError::InvalidToken(token) => format!("invalid token {}", token),
            TokenizationError::InvalidEscape(escape) => format!("invalid escape {}", escape),
            TokenizationError::NoEscapeChar => "missing escaped character".to_string(),
            TokenizationError::NoEscapeValue(escape) => {
                format!("missing value for escape {}", escape)
            }
            TokenizationError::InvalidHex(hex) => format!("invalid hex {}", hex),
            TokenizationError::InvalidUnicode(unicode) => {
                format!("invalid unicode value {}", unicode)
            }
            TokenizationError::InvalidNumberRadix(radix) => {
                format!("invalid number radix {}", radix)
            }
            TokenizationError::NumberParseError(err) => format!("number parse error: {}", err),
        }
        .fmt(f)
    }
}

pub struct Lexer {
    source: Box<dyn Iterator<Item = char>>,
    state: StateManager,
}

impl Lexer {
    pub fn new(source: Box<dyn Iterator<Item = char>>) -> Self {
        Self {
            source,
            state: StateManager::new(),
        }
    }
}

impl Iterator for Lexer {
    type Item = Result<Token, TokenizationError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buffer = String::new();
        let mut matchers = self.state.get().matchers();
        let mut match_candidate = None;
        for ch in self.source.by_ref() {
            if ch.is_whitespace() && self.state.get().ignore_space() {
                if let Some(matcher) = match_candidate {
                    // return Some(matcher.transform(&buffer, &mut self.state));
                }
            }
            match_candidate = Some(&matchers[0]);
            buffer.push(ch);
        }
        return None;
    }
}
