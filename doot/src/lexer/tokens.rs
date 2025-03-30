use std::fmt::Display;

use crate::SourcePosition;

#[derive(Debug)]
pub struct Token {
    value: TokenType,
    pos: SourcePosition,
}

impl Token {
    pub fn new(value: TokenType, pos: SourcePosition) -> Self {
        Self { value, pos }
    }

    pub fn value(&self) -> &TokenType {
        &self.value
    }

    pub fn pos(&self) -> &SourcePosition {
        &self.pos
    }
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}@{}", self.value, self.pos)
    }
}

#[derive(Debug, Clone)]
pub enum TokenType {
    // symbols
    Plus,             // +
    Minus,            // -
    Asterisk,         // *
    Slash,            // /
    LeftParen,        // (
    RightParen,       // )
    LeftSquare,       // [
    RightSquare,      // ]
    LeftBrace,        // {
    RightBrace,       // }
    Comma,            // ,
    Dot,              // .
    Equal,            // =
    DoubleEqual,      // ==
    Bang,             // !
    BangEqual,        // !=
    Greater,          // >
    GreaterEqual,     // >=
    Less,             // <
    LessEqual,        // <=
    Ampersand,        // &
    DoubleAmpersand,  // &&
    Pipe,             // |
    DoublePipe,       // ||
    DoubleQuotes,     // "
    PoundStringOpen,  // #`
    PoundStringClose, // `#
    DollarLeftBrace,  // ${

    // keywords
    Let,    // let
    Var,    // var
    Const,  // const
    If,     // if
    Else,   // else
    For,    // for
    While,  // while
    Class,  // class
    Fn,     // fn
    Return, // return

    // literals
    Identifier(String),    // foo
    StringLiteral(String), // "hello, world"
    BoolLiteral(bool),     // true, false
    IntLiteral(i64),       // 1234
    FloatLiteral(f64),     // 1234.5678
    Null,                  // null
}
