use std::collections::LinkedList;

use crate::lexer::{matchers::ChainMatcher, parsing};

use super::{
    TokenizationError,
    matchers::{DefaultMatcher, Matcher},
    tokens::Token,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexerState {
    Normal(bool),
    CompositeString,
    RawString(usize),
    Comment(String),
}

impl LexerState {
    pub(super) fn ignore_whitespace(&self) -> bool {
        match self {
            LexerState::Normal(_) => true,
            _ => false,
        }
    }

    pub(super) fn matchers(&self) -> Vec<Box<dyn Matcher<Token>>> {
        fn int_literal(prefix: &str) -> Box<dyn Matcher<Token>> {
            ChainMatcher::new(
                [
                    DefaultMatcher::fixed_text(prefix),
                    DefaultMatcher::take_while(
                        |buff, ch| {
                            if buff.len() == 1 {
                                ch.is_ascii_digit()
                            } else {
                                ch == '_' || ch.is_alphanumeric()
                            }
                        },
                        1,
                        |value, _| Ok(value.to_string()),
                    ),
                ],
                |val, _, _| {
                    parsing::parse_int(val)
                        .map(Token::IntLiteral)
                        .map_err(TokenizationError::NumberParse)
                },
            )
        }

        fn float_literal(prefix: &str) -> Box<dyn Matcher<Token>> {
            ChainMatcher::new(
                [
                    DefaultMatcher::fixed_text(prefix),
                    DefaultMatcher::take_while(
                        |buff, ch| {
                            if buff.len() == 1 {
                                ch.is_ascii_digit()
                            } else {
                                ch == '_' || ch == '.' || ch.is_alphanumeric()
                            }
                        },
                        1,
                        |value, _| Ok(value.to_string()),
                    ),
                ],
                |val, _, _| {
                    parsing::parse_float(val)
                        .map(Token::FloatLiteral)
                        .map_err(TokenizationError::NumberParse)
                },
            )
        }
        match *self {
            Self::Normal(from_string) => vec![
                // symbols
                DefaultMatcher::simple_text("+", Token::Plus),
                DefaultMatcher::simple_text("-", Token::Minus),
                DefaultMatcher::simple_text("*", Token::Asterisk),
                DefaultMatcher::simple_text("/", Token::Slash),
                DefaultMatcher::simple_text("(", Token::LeftParen),
                DefaultMatcher::simple_text(")", Token::RightParen),
                DefaultMatcher::simple_text("[", Token::LeftSquare),
                DefaultMatcher::simple_text("]", Token::RightSquare),
                DefaultMatcher::text("{", move |_, state| {
                    if from_string {
                        state.push(Self::Normal(true));
                    }
                    Ok(Token::LeftBrace)
                }),
                DefaultMatcher::text("}", move |_, state| {
                    if from_string {
                        state.pop();
                    }
                    Ok(Token::RightBrace)
                }),
                DefaultMatcher::simple_text(",", Token::Comma),
                DefaultMatcher::simple_text(".", Token::Dot),
                DefaultMatcher::simple_text("=", Token::Equal),
                DefaultMatcher::simple_text("==", Token::DoubleEqual),
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
                    true,
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
                    1,
                    |value, _| Ok(Token::Identifier(value.to_string())),
                ),
                int_literal(""),
                int_literal("-"),
                float_literal(""),
                float_literal("-"),
                DefaultMatcher::text("//", |_, state| {
                    state.push(Self::Comment("\n".to_string()));
                    Ok(Token::LineCommentOpen)
                }),
                DefaultMatcher::text("/*", |_, state| {
                    state.push(Self::Comment("*/".to_string()));
                    Ok(Token::BlockCommentOpen)
                }),
            ],
            Self::CompositeString => vec![
                DefaultMatcher::take_while(
                    |buff, _| !["\"", "${", "\\"].iter().any(|t| buff.ends_with(t)), // unclosed string literals
                    0,
                    |value, _| Ok(Token::StringLiteral(value.to_string())),
                ),
                DefaultMatcher::collector(["\"", "${", "\\"], false, |value, _, _| {
                    Ok(Token::StringLiteral(value.to_string()))
                }),
                DefaultMatcher::text("\"", |_, state| {
                    state.pop();
                    Ok(Token::StringClose)
                }),
                DefaultMatcher::text("${", |_, state| {
                    state.push(Self::Normal(true));
                    Ok(Token::DollarLeftBrace)
                }),
                ChainMatcher::new(
                    [
                        DefaultMatcher::fixed_text("\\"),
                        DefaultMatcher::conditions(
                            vec![Box::new(|_: &str, ch: char| !ch.is_whitespace())],
                            |val, _| Ok(val.to_string()),
                        ),
                    ],
                    |_, [prefix, escaped], _| {
                        parsing::escape(&format!("{}{}", prefix, escaped))
                            .map(|ch| Token::StringLiteral(ch.to_string()))
                            .map_err(TokenizationError::EscapeParse)
                    },
                ),
                ChainMatcher::new(
                    [
                        DefaultMatcher::fixed_text("\\"),
                        DefaultMatcher::conditions(
                            vec![Box::new(|_: &str, ch: char| ch.is_whitespace())],
                            |val, _| Ok(val.to_string()),
                        ),
                    ],
                    |_, _, _| Err(TokenizationError::NoEscape),
                ),
                ChainMatcher::new(
                    [
                        DefaultMatcher::fixed_text("\\u{"),
                        DefaultMatcher::filtered_collector(
                            ["}"],
                            |_, ch| !ch.is_whitespace(),
                            true,
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
                    .chain(std::iter::repeat('#').take(pounds))
                    .collect::<String>();
                vec![
                    DefaultMatcher::collector(
                        [pound_terminator.clone().as_ref()],
                        false,
                        |value, _, _| Ok(Token::StringLiteral(value.to_string())),
                    ),
                    DefaultMatcher::text(pound_terminator.clone().as_ref(), |_, state| {
                        state.pop();
                        Ok(Token::StringClose)
                    }),
                    {
                        // unclosed string literals
                        let terminator = pound_terminator.clone();
                        DefaultMatcher::take_while(
                            move |buff, _| !buff.ends_with(&terminator),
                            0,
                            |value, _| Ok(Token::StringLiteral(value.to_string())),
                        )
                    },
                ]
            }
            Self::Comment(ref terminator) => vec![
                DefaultMatcher::collector([terminator.clone().as_ref()], false, |value, _, _| {
                    Ok(Token::CommentLiteral(value.to_string()))
                }),
                DefaultMatcher::text(terminator.clone().as_ref(), |_, state| {
                    state.pop();
                    Ok(Token::CommentClose)
                }),
                {
                    // unclosed string literals
                    let terminator = terminator.clone();
                    DefaultMatcher::take_while(
                        move |buff, _| !buff.ends_with(&terminator),
                        0,
                        |value, _| Ok(Token::CommentLiteral(value.to_string())),
                    )
                },
            ],
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
        instance.push(LexerState::Normal(false));
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
