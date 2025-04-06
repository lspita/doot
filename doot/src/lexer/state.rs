use std::collections::LinkedList;

use crate::lexer::{matchers::ChainMatcher, parsing};

use super::{
    matchers::{BufferedMatcher, Matcher},
    tokens::TokenType,
};

#[derive(PartialEq, Eq)]
pub(super) enum ParseState {
    Normal,
    CompositeString,
    RawString(usize),
}

impl ParseState {
    pub(super) fn ignore_space(&self) -> bool {
        *self == Self::Normal
    }

    pub(super) fn matchers(&self) -> Vec<Box<dyn Matcher<TokenType>>> {
        match self {
            ParseState::Normal => vec![
                // symbols
                BufferedMatcher::simple_text("+", TokenType::Plus),
                BufferedMatcher::simple_text("-", TokenType::Minus),
                BufferedMatcher::simple_text("*", TokenType::Asterisk),
                BufferedMatcher::simple_text("/", TokenType::Slash),
                BufferedMatcher::simple_text("(", TokenType::LeftParen),
                BufferedMatcher::simple_text(")", TokenType::RightParen),
                BufferedMatcher::simple_text("[", TokenType::LeftSquare),
                BufferedMatcher::simple_text("]", TokenType::RightSquare),
                BufferedMatcher::simple_text("{", TokenType::LeftBrace),
                BufferedMatcher::simple_text("}", TokenType::RightBrace),
                BufferedMatcher::simple_text(",", TokenType::Comma),
                BufferedMatcher::simple_text(".", TokenType::Dot),
                BufferedMatcher::simple_text("=", TokenType::Equal),
                BufferedMatcher::simple_text("==", TokenType::EqualEqual),
                BufferedMatcher::simple_text("!", TokenType::Bang),
                BufferedMatcher::simple_text("!=", TokenType::BangEqual),
                BufferedMatcher::simple_text(">", TokenType::Greater),
                BufferedMatcher::simple_text(">=", TokenType::GreaterEqual),
                BufferedMatcher::simple_text("<", TokenType::Less),
                BufferedMatcher::simple_text("<=", TokenType::LessEqual),
                BufferedMatcher::simple_text("&", TokenType::Ampersand),
                BufferedMatcher::simple_text("&&", TokenType::DoubleAmpersand),
                BufferedMatcher::simple_text("|", TokenType::Pipe),
                BufferedMatcher::simple_text("||", TokenType::DoublePipe),
                BufferedMatcher::text("\"", |_, state| {
                    state.push(ParseState::CompositeString);
                    Ok(TokenType::StringOpen)
                }),
                BufferedMatcher::filtered_collector(
                    ["`"],
                    |_, ch| ch == '#' || ch == '`',
                    |pounds, _, state| {
                        state.push(ParseState::RawString(pounds.len()));
                        Ok(TokenType::StringOpen)
                    },
                ),
                // keywords
                BufferedMatcher::simple_text("let", TokenType::Let),
                BufferedMatcher::simple_text("var", TokenType::Var),
                BufferedMatcher::simple_text("const", TokenType::Const),
                BufferedMatcher::simple_text("if", TokenType::If),
                BufferedMatcher::simple_text("else", TokenType::Else),
                BufferedMatcher::simple_text("for", TokenType::For),
                BufferedMatcher::simple_text("while", TokenType::While),
                BufferedMatcher::simple_text("class", TokenType::Class),
                BufferedMatcher::simple_text("fn", TokenType::Fn),
                BufferedMatcher::simple_text("return", TokenType::Return),
                // literals
                BufferedMatcher::simple_text("null", TokenType::Null),
                BufferedMatcher::simple_text("true", TokenType::BoolLiteral(true)),
                BufferedMatcher::simple_text("false", TokenType::BoolLiteral(false)),
                BufferedMatcher::take_while(
                    |buff, ch| {
                        ch == '_'
                            || if buff.len() == 1 {
                                ch.is_alphabetic()
                            } else {
                                ch.is_alphanumeric()
                            }
                    },
                    1,
                    |value, _| Ok(TokenType::Identifier(value.to_string())),
                ),
                BufferedMatcher::take_while(
                    |buff, ch| {
                        if buff.len() == 1 {
                            ch == '.' || ch.is_ascii_digit()
                        } else {
                            "_.".contains(ch) || ch.is_alphanumeric()
                        }
                    },
                    1,
                    |value, _| {
                        if value.contains('.') {
                            parsing::parse_float(value).map(TokenType::FloatLiteral)
                        } else {
                            parsing::parse_int(value).map(TokenType::IntLiteral)
                        }
                    },
                ),
            ],
            ParseState::CompositeString => {
                vec![
                    BufferedMatcher::collector(["\"", "${", "\\"], |value, _, _| {
                        Ok(TokenType::StringLiteral(value.to_string()))
                    }),
                    BufferedMatcher::text("\"", |_, state| {
                        state.pop();
                        Ok(TokenType::StringClose)
                    }),
                    BufferedMatcher::text("${", |_, state| {
                        state.push(ParseState::Normal);
                        Ok(TokenType::DollarLeftBrace)
                    }),
                    ChainMatcher::new(
                        [
                            BufferedMatcher::prefix("\\"),
                            BufferedMatcher::conditions(
                                vec![Box::new(|_: &str, ch: char| !ch.is_whitespace())],
                                |val, _| parsing::replace_escape(val).map(|ch| ch.to_string()),
                            ),
                        ],
                        |_, [_, escaped], _| Ok(TokenType::StringLiteral(escaped.clone())),
                    ),
                    ChainMatcher::new(
                        [
                            BufferedMatcher::prefix("\\u{"),
                            BufferedMatcher::filtered_collector(
                                ["}"],
                                |_, ch| !ch.is_whitespace(),
                                |hex, _, _| parsing::parse_unicode(hex).map(|c| c.to_string()),
                            ),
                        ],
                        |_, [_, unicode], _| Ok(TokenType::StringLiteral(unicode.clone())),
                    ),
                ]
            }
            ParseState::RawString(pounds) => {
                let pound_terminator = format!(
                    "`{}",
                    std::iter::repeat('#').take(*pounds).collect::<String>()
                );
                vec![
                    BufferedMatcher::collector([&pound_terminator], |value, _, _| {
                        Ok(TokenType::StringLiteral(value.to_string()))
                    }),
                    BufferedMatcher::text(&pound_terminator, |_, state| {
                        state.pop();
                        Ok(TokenType::StringClose)
                    }),
                ]
            }
        }
    }
}

pub(super) struct StateManager {
    states: LinkedList<ParseState>,
}

impl StateManager {
    pub(super) fn new() -> Self {
        let mut instance = Self {
            states: LinkedList::new(),
        };
        instance.push(ParseState::Normal);
        instance
    }

    pub(super) fn get(&self) -> &ParseState {
        self.states.front().unwrap()
    }

    pub(super) fn push(&mut self, state: ParseState) {
        self.states.push_front(state);
    }

    pub(super) fn pop(&mut self) {
        if self.states.len() < 2 {
            panic!()
        }
        self.states.pop_front().unwrap();
    }
}
