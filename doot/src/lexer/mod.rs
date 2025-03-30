use std::collections::LinkedList;

use regex::{Captures, Regex};
use tokens::{Token, TokenType};

pub mod tokens;

#[derive(Debug)]
pub enum TokenizationError {
    InvalidToken(String),
}

enum ParseState {
    Normal,
    CompositeString,
    RawString(usize),
}

struct StateManager {
    states: LinkedList<ParseState>,
}

impl StateManager {
    const NO_STATE_ERROR: &str = "Lexer has not state";

    fn new() -> Self {
        Self {
            states: LinkedList::from([ParseState::Normal]),
        }
    }

    fn get(&self) -> &ParseState {
        self.states.front().expect(Self::NO_STATE_ERROR)
    }

    fn push(&mut self, state: ParseState) {
        self.states.push_front(state);
    }

    fn pop(&mut self) {
        self.states.pop_front().expect(Self::NO_STATE_ERROR);
    }
}

struct Matcher {
    regex: Regex,
    transformer: Box<dyn FnMut(Captures, &mut StateManager) -> TokenType>,
}

impl Matcher {
    fn regex(
        re: &str,
        transformer: Box<dyn FnMut(Captures, &mut StateManager) -> TokenType>,
    ) -> Self {
        Self {
            regex: Regex::new(re).unwrap(),
            transformer,
        }
    }

    fn regex_full(
        re: &str,
        mut mapper: Box<dyn FnMut(String, &mut StateManager) -> TokenType>,
    ) -> Self {
        Self::regex(
            re,
            Box::new(move |c, s| mapper(String::from(c.get(0).unwrap().as_str()), s)),
        )
    }

    fn text(source: &str, mapper: Box<dyn FnMut(String, &mut StateManager) -> TokenType>) -> Self {
        Self::regex_full(&regex::escape(source), mapper)
    }

    fn text_simple(source: &str, token: TokenType) -> Self {
        Self::text(source, Box::new(move |_, _| token.clone()))
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

    fn matchers(&self) -> Vec<Matcher> {
        match self.state.get() {
            ParseState::Normal => vec![
                // symbols
                Matcher::text_simple("+", TokenType::Plus),
                Matcher::text_simple("-", TokenType::Minus),
                Matcher::text_simple("*", TokenType::Asterisk),
                Matcher::text_simple("/", TokenType::Slash),
                Matcher::text_simple("(", TokenType::LeftParen),
                Matcher::text_simple(")", TokenType::RightParen),
                Matcher::text_simple("[", TokenType::LeftSquare),
                Matcher::text_simple("]", TokenType::RightSquare),
                Matcher::text_simple("{", TokenType::LeftBrace),
                Matcher::text_simple("}", TokenType::RightBrace),
                Matcher::text_simple(",", TokenType::Comma),
                Matcher::text_simple(".", TokenType::Dot),
                Matcher::text_simple("=", TokenType::Equal),
                Matcher::text_simple("==", TokenType::DoubleEqual),
                Matcher::text_simple("!", TokenType::Bang),
                Matcher::text_simple("!=", TokenType::BangEqual),
                Matcher::text_simple(">", TokenType::Greater),
                Matcher::text_simple(">=", TokenType::GreaterEqual),
                Matcher::text_simple("<", TokenType::Less),
                Matcher::text_simple("<=", TokenType::LessEqual),
                Matcher::text_simple("&", TokenType::Ampersand),
                Matcher::text_simple("&&", TokenType::DoubleAmpersand),
                Matcher::text_simple("|", TokenType::Pipe),
                Matcher::text_simple("||", TokenType::DoublePipe),
                Matcher::text(
                    "\"",
                    Box::new(|_, state| {
                        state.push(ParseState::CompositeString);
                        TokenType::DoubleQuotes
                    }),
                ),
                Matcher::regex_full(
                    r"#*`",
                    Box::new(|token, state| {
                        state.push(ParseState::RawString(
                            token.chars().take_while(|c| *c == '#').count(),
                        ));
                        TokenType::PoundStringOpen
                    }),
                ),
                // keywords
                Matcher::text_simple("let", TokenType::Let),
                Matcher::text_simple("var", TokenType::Var),
                Matcher::text_simple("const", TokenType::Const),
                Matcher::text_simple("if", TokenType::If),
                Matcher::text_simple("else", TokenType::Else),
                Matcher::text_simple("for", TokenType::For),
                Matcher::text_simple("while", TokenType::While),
                Matcher::text_simple("class", TokenType::Class),
                Matcher::text_simple("fn", TokenType::Fn),
                Matcher::text_simple("return", TokenType::Return),
                // literals
                Matcher::text_simple("null", TokenType::Null),
                Matcher::text_simple("true", TokenType::BoolLiteral(true)),
                Matcher::text_simple("false", TokenType::BoolLiteral(false)),
            ],
            ParseState::CompositeString => vec![],
            ParseState::RawString(pounds) => vec![],
        }
    }

    fn parse_buffer(s: &str) -> Option<TokenType> {}
}

impl Iterator for Lexer {
    type Item = Result<Token, TokenizationError>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}
