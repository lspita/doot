#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // symbols
    Plus,            // +
    Minus,           // -
    Asterisk,        // *
    Slash,           // /
    LeftParen,       // (
    RightParen,      // )
    LeftSquare,      // [
    RightSquare,     // ]
    LeftBrace,       // {
    RightBrace,      // }
    Comma,           // ,
    Dot,             // .
    Equal,           // =
    DoubleEqual,     // ==
    Bang,            // !
    BangEqual,       // !=
    Greater,         // >
    GreaterEqual,    // >=
    Less,            // <
    LessEqual,       // <=
    Ampersand,       // &
    DoubleAmpersand, // &&
    Pipe,            // |
    DoublePipe,      // ||
    StringOpen,      // ", #`
    StringClose,     // ", `#
    DollarLeftBrace, // ${

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
    Null,                  // null
    BoolLiteral(bool),     // true, false
    Identifier(String),    // foo
    IntLiteral(i64),       // 1234
    FloatLiteral(f64),     // 1234.5678
    StringLiteral(String), // "hello, world" (the content inside the quotes)
}
