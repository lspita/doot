use std::collections::LinkedList;

use crate::lexer::{matchers::ChainMatcher, parsing};

use super::{
    TokenizationError,
    matchers::{DefaultMatcher, Matcher},
    tokens::Token,
};

#[derive(PartialEq, Eq)]
pub(super) enum LexerState {
    Normal,
    CompositeString,
    RawString(usize),
}

impl LexerState {
    pub(super) fn matchers(&self) -> Vec<Box<dyn Matcher<Token>>> {
        fn number_literal(prefix: &str) -> Box<dyn Matcher<Token>> {
            ChainMatcher::new(
                [
                    DefaultMatcher::prefix(prefix),
                    DefaultMatcher::take_while(
                        |buff, ch| {
                            if buff.len() == 1 {
                                ch == '.' || ch.is_ascii_digit()
                            } else {
                                ch == '_' || ch == '.' || ch.is_alphanumeric()
                            }
                        },
                        |value, _| Ok(value.to_string()),
                    ),
                ],
                |_, [_, value], _| {
                    if value.contains('.') {
                        parsing::parse_float(&value)
                            .map(Token::FloatLiteral)
                            .map_err(TokenizationError::NumberParse)
                    } else {
                        parsing::parse_int(&value)
                            .map(Token::IntLiteral)
                            .map_err(TokenizationError::NumberParse)
                    }
                },
            )
        }
        match self {
            Self::Normal => vec![
                // symbols
                DefaultMatcher::simple_text("+", Token::Plus),
                DefaultMatcher::simple_text("-", Token::Minus),
                DefaultMatcher::simple_text("*", Token::Asterisk),
                DefaultMatcher::simple_text("/", Token::Slash),
                DefaultMatcher::simple_text("(", Token::LeftParen),
                DefaultMatcher::simple_text(")", Token::RightParen),
                DefaultMatcher::simple_text("[", Token::LeftSquare),
                DefaultMatcher::simple_text("]", Token::RightSquare),
                DefaultMatcher::simple_text("{", Token::LeftBrace),
                DefaultMatcher::simple_text("}", Token::RightBrace),
                DefaultMatcher::simple_text(",", Token::Comma),
                DefaultMatcher::simple_text(".", Token::Dot),
                DefaultMatcher::simple_text("=", Token::Equal),
                DefaultMatcher::simple_text("==", Token::EqualEqual),
                DefaultMatcher::simple_text("!", Token::Bang),
                DefaultMatcher::simple_text("!=", Token::BangEqual),
                DefaultMatcher::simple_text(">", Token::Greater),
                DefaultMatcher::simple_text(">=", Token::GreaterEqual),
                DefaultMatcher::simple_text("<", Token::Less),
                DefaultMatcher::simple_text("<=", Token::LessEqual),
                DefaultMatcher::simple_text("&", Token::Ampersand),
                DefaultMatcher::simple_text("&&", Token::DoubleAmpersand),
                DefaultMatcher::simple_text("|", Token::Pipe),
                DefaultMatcher::simple_text("||", Token::DoublePipe),
                DefaultMatcher::text("\"", |_, state| {
                    state.push(Self::CompositeString);
                    Ok(Token::StringOpen)
                }),
                DefaultMatcher::filtered_collector(
                    ["`"],
                    |_, ch| ch == '#' || ch == '`',
                    |pounds, _, state| {
                        state.push(Self::RawString(pounds.len()));
                        Ok(Token::StringOpen)
                    },
                ),
                // keywords
                DefaultMatcher::simple_text("let", Token::Let),
                DefaultMatcher::simple_text("var", Token::Var),
                DefaultMatcher::simple_text("const", Token::Const),
                DefaultMatcher::simple_text("if", Token::If),
                DefaultMatcher::simple_text("else", Token::Else),
                DefaultMatcher::simple_text("for", Token::For),
                DefaultMatcher::simple_text("while", Token::While),
                DefaultMatcher::simple_text("class", Token::Class),
                DefaultMatcher::simple_text("fn", Token::Fn),
                DefaultMatcher::simple_text("return", Token::Return),
                // literals
                DefaultMatcher::simple_text("null", Token::Null),
                DefaultMatcher::simple_text("true", Token::BoolLiteral(true)),
                DefaultMatcher::simple_text("false", Token::BoolLiteral(false)),
                DefaultMatcher::take_while(
                    |buff, ch| {
                        ch == '_'
                            || if buff.len() == 1 {
                                ch.is_alphabetic()
                            } else {
                                ch.is_alphanumeric()
                            }
                    },
                    |value, _| Ok(Token::Identifier(value.to_string())),
                ),
                number_literal(""),
                number_literal("+"),
                number_literal("-"),
            ],
            Self::CompositeString => vec![
                DefaultMatcher::collector(["\"", "${", "\\"], |value, _, _| {
                    Ok(Token::StringLiteral(value.to_string()))
                }),
                DefaultMatcher::text("\"", |_, state| {
                    state.pop();
                    Ok(Token::StringClose)
                }),
                DefaultMatcher::text("${", |_, state| {
                    state.push(Self::Normal);
                    Ok(Token::DollarLeftBrace)
                }),
                DefaultMatcher::text("\\", |_, _| Err(TokenizationError::NoEscape)),
                ChainMatcher::new(
                    [
                        DefaultMatcher::prefix("\\"),
                        DefaultMatcher::conditions(
                            vec![Box::new(|_: &str, ch: char| !ch.is_whitespace())],
                            |val, _| {
                                parsing::replace_escape(val)
                                    .map(|ch| ch.to_string())
                                    .map_err(TokenizationError::EscapeParse)
                            },
                        ),
                    ],
                    |_, [_, escaped], _| Ok(Token::StringLiteral(escaped.clone())),
                ),
                ChainMatcher::new(
                    [
                        DefaultMatcher::prefix("\\u{"),
                        DefaultMatcher::filtered_collector(
                            ["}"],
                            |_, ch| !ch.is_whitespace(),
                            |hex, _, _| {
                                parsing::parse_unicode(hex)
                                    .map(|c| c.to_string())
                                    .map_err(TokenizationError::UnicodeParse)
                            },
                        ),
                    ],
                    |_, [_, unicode], _| Ok(Token::StringLiteral(unicode.clone())),
                ),
            ],
            Self::RawString(pounds) => {
                let pound_terminator = ['`'] // ` followed by # `pounds` times
                    .into_iter()
                    .chain(std::iter::repeat('#').take(*pounds))
                    .collect::<String>();
                vec![
                    DefaultMatcher::collector([&pound_terminator], |value, _, _| {
                        Ok(Token::StringLiteral(value.to_string()))
                    }),
                    DefaultMatcher::text(&pound_terminator, |_, state| {
                        state.pop();
                        Ok(Token::StringClose)
                    }),
                ]
            }
        }
    }
}

pub(super) struct LexerStateManager {
    states: LinkedList<LexerState>,
}

impl LexerStateManager {
    pub(super) fn new() -> Self {
        let mut instance = Self {
            states: LinkedList::new(),
        };
        instance.push(LexerState::Normal);
        instance
    }

    pub(super) fn get(&self) -> &LexerState {
        self.states.front().unwrap()
    }

    pub(super) fn push(&mut self, state: LexerState) {
        self.states.push_front(state);
    }

    pub(super) fn pop(&mut self) {
        if self.states.len() < 2 {
            panic!()
        }
        self.states.pop_front().unwrap();
    }
}
