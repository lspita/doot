#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // symbols
    Plus,            // +
    Dash,            // -
    Asterisk,        // *
    Slash,           // /
    Percent,         // %
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
    DoubleAmpersand, // &&
    DoublePipe,      // ||
    StringQuotes,    // ", #`, `#
    DollarLeftBrace, // ${
    Newline,         // \n
    SemiColon,       // ;
    Questionmark,    // ?

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
    Null,                   // null
    BoolLiteral(bool),      // true, false
    Identifier(String),     // foo
    IntLiteral(String),     // 1234
    FloatLiteral(String),   // 1234.5678
    StringLiteral(String),  // "hello, world" (the content)
    CommentLiteral(String), // // hello, world, /* hello world */
}
