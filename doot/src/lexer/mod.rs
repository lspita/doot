use std::{char, error::Error, fmt::Display};

use matchers::{Matcher, MatcherState};
use parsing::{EscapeParseError, NumberParseError, UnicodeParseError};
use state::LexerStateManager;
use tokens::Token;

mod matchers;
mod parsing;
mod state;
pub mod tokens;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenizationError {
    InvalidToken(String),
    NoEscape,
    EscapeParse(EscapeParseError),
    UnicodeParse(UnicodeParseError),
    NumberParse(NumberParseError),
}

impl Error for TokenizationError {}
impl Display for TokenizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenizationError::InvalidToken(token) => format!("invalid token {}", token),
            TokenizationError::NoEscape => {
                format!("missing escaped character")
            }
            TokenizationError::EscapeParse(EscapeParseError::InvalidEscape(escape)) => {
                format!("invalid escape {}", escape)
            }
            TokenizationError::EscapeParse(EscapeParseError::NoValue(escape)) => {
                format!("missing value for escape {}", escape)
            }
            TokenizationError::UnicodeParse(UnicodeParseError::InvalidHex(hex)) => {
                format!("invalid hex value {}", hex)
            }
            TokenizationError::UnicodeParse(UnicodeParseError::InvalidValue(unicode)) => {
                format!("invalid unicode value {}", unicode)
            }
            TokenizationError::NumberParse(err) => format!("number parse error: {}", err),
        }
        .fmt(f)
    }
}

pub struct Lexer {
    source: Box<dyn Iterator<Item = char>>,
    buffer: String,
    state: LexerStateManager,
}

impl Lexer {
    pub fn new(source: Box<dyn Iterator<Item = char>>) -> Self {
        Self {
            source,
            buffer: String::new(),
            state: LexerStateManager::new(),
        }
    }
}

type MatcherBox = Box<dyn Matcher<Token>>;
struct Candidate<'a> {
    matcher: &'a mut MatcherBox,
    previous_state: Option<MatcherState>,
}

impl Iterator for Lexer {
    type Item = Result<Token, TokenizationError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut matchers = self.state.get().matchers();
        let mut candidates: Vec<_> = matchers
            .iter_mut()
            .map(|m| Candidate {
                matcher: m,
                previous_state: None,
            })
            .collect();
        let mut matching = false;

        let starting_buffer = self.buffer.clone();
        self.buffer.clear();

        for ch in starting_buffer
            .chars()
            .chain(self.source.by_ref())
            .chain(['\0'])
        {
            if ch.is_whitespace() && !matching {
                continue;
            }
            matching = true;
            self.buffer.push(ch);
            candidates.iter_mut().for_each(|c| {
                c.previous_state = Some(c.matcher.state().clone());
                c.matcher.accept(&self.buffer, ch);
            });
            if candidates
                .iter()
                .any(|c| *c.matcher.state() != MatcherState::Broken)
            {
                candidates = candidates
                    .into_iter()
                    .filter(|c| *c.matcher.state() != MatcherState::Broken)
                    .collect();
            } else {
                let mut candidates: Vec<_> = candidates
                    .into_iter()
                    .filter(|c| c.previous_state == Some(MatcherState::Closeable))
                    .map(|c| c.matcher)
                    .collect();
                candidates.sort_by(|m1, m2| m1.class().cmp(m2.class()));
                return candidates
                    .iter_mut()
                    .next()
                    .map(|m| {
                        m.close(&self.buffer[..&self.buffer.len() - 1], &mut self.state)
                            .map(|(tok, n_drained)| {
                                self.buffer.drain(..n_drained);
                                tok
                            })
                    })
                    .or_else(|| Some(Err(TokenizationError::InvalidToken(self.buffer.clone()))));
            }
        }
        None
    }
}
