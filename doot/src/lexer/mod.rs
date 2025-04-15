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

pub struct Lexer<'a> {
    source: Box<dyn Iterator<Item = char> + 'a>,
    buffer: String,
    state: LexerStateManager,
    failed: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(source: impl Iterator<Item = char> + 'a) -> Self {
        Self {
            source: Box::new(source),
            buffer: String::new(),
            state: LexerStateManager::new(),
            failed: false,
        }
    }

    fn clean_buffer(&self) -> String {
        self.buffer.trim_end_matches('\0').to_string()
    }
}

type MatcherBox = Box<dyn Matcher<Token>>;
struct Candidate<'a> {
    matcher: &'a mut MatcherBox,
    previous_state: Option<MatcherState>,
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Token, TokenizationError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let mut matchers = self.state.get().matchers();
        let mut candidates: Vec<_> = matchers
            .iter_mut()
            .map(|m| Candidate {
                matcher: m,
                previous_state: None,
            })
            .collect();
        let mut matching = false;

        let starting_buffer = self.clean_buffer();
        let mut source = starting_buffer
            .chars()
            .chain(self.source.by_ref())
            .peekable();
        if source.peek().is_none() {
            return None;
        }
        self.buffer.clear();
        for ch in source.chain(['\0']) {
            if !matching
                && (ch == '\0' || (self.state.get().ignore_whitespace() && ch.is_whitespace()))
            {
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
                let mut matchers: Vec<_> = candidates
                    .into_iter()
                    .filter(|c| c.previous_state == Some(MatcherState::Closeable))
                    .map(|c| c.matcher)
                    .collect();
                matchers.sort_by(|m1, m2| m1.class().cmp(m2.class()));
                return Some(
                    matchers
                        .iter_mut()
                        .next()
                        .map(|m| {
                            m.close(&self.buffer[..&self.buffer.len() - 1], &mut self.state)
                                .map(|(tok, n_drained)| {
                                    self.buffer.drain(..n_drained);
                                    tok
                                })
                        })
                        .unwrap_or_else(|| {
                            Err(TokenizationError::InvalidToken(self.clean_buffer()))
                        })
                        .inspect_err(|_| self.failed = true),
                );
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::lexer::parsing::{EscapeParseError, NumberParseError, UnicodeParseError};

    use super::{Lexer, TokenizationError, tokens::Token};

    fn assert_results<const N: usize>(
        source: &str,
        expected: [Result<Token, TokenizationError>; N],
    ) {
        let tokens: Vec<_> = Lexer::new(source.chars()).collect();
        assert_eq!(tokens, Vec::from(expected))
    }

    fn assert_tokens<const N: usize>(source: &str, expected: [Token; N]) {
        assert_results(source, expected.map(Ok));
    }

    #[rstest]
    #[case("+", Token::Plus)]
    #[case("-", Token::Minus)]
    #[case("*", Token::Asterisk)]
    #[case("/", Token::Slash)]
    #[case("(", Token::LeftParen)]
    #[case(")", Token::RightParen)]
    #[case("[", Token::LeftSquare)]
    #[case("]", Token::RightSquare)]
    #[case("{", Token::LeftBrace)]
    #[case("}", Token::RightBrace)]
    #[case(",", Token::Comma)]
    #[case(".", Token::Dot)]
    #[case("=", Token::Equal)]
    #[case("==", Token::DoubleEqual)]
    #[case("!", Token::Bang)]
    #[case("!=", Token::BangEqual)]
    #[case(">", Token::Greater)]
    #[case(">=", Token::GreaterEqual)]
    #[case("<", Token::Less)]
    #[case("<=", Token::LessEqual)]
    #[case("&", Token::Ampersand)]
    #[case("&&", Token::DoubleAmpersand)]
    #[case("|", Token::Pipe)]
    #[case("||", Token::DoublePipe)]
    #[case("let", Token::Let)]
    #[case("var", Token::Var)]
    #[case("const", Token::Const)]
    #[case("if", Token::If)]
    #[case("else", Token::Else)]
    #[case("for", Token::For)]
    #[case("while", Token::While)]
    #[case("class", Token::Class)]
    #[case("fn", Token::Fn)]
    #[case("return", Token::Return)]
    #[case("null", Token::Null)]
    #[case("true", Token::BoolLiteral(true))]
    #[case("false", Token::BoolLiteral(false))]
    #[case("//", Token::LineCommentOpen)]
    #[case("/*", Token::BlockCommentOpen)]
    #[case(";", Token::SemiColon)]
    fn simple_tokens(#[case] source: &str, #[case] expected: Token) {
        assert_tokens(source, [expected]);
    }

    #[rstest]
    #[case("foo", Token::Identifier("foo".to_string()))]
    #[case("_123", Token::Identifier("_123".to_string()))]
    #[case("123", Token::IntLiteral(123))]
    #[case("-123", Token::IntLiteral(-123))]
    #[case("123.456", Token::FloatLiteral(123.456))]
    #[case("-123.456", Token::FloatLiteral(-123.456))]
    fn normal_literals(#[case] source: &str, #[case] expected: Token) {
        assert_tokens(source, [expected]);
    }

    #[rstest]
    #[case("123.abc")]
    #[case("123.456.789")]
    fn floats_infinite_points(#[case] source: &str) {
        assert_results(
            source,
            [Err(TokenizationError::NumberParse(
                NumberParseError::InvalidFloat,
            ))],
        );
    }

    #[rstest]
    #[case("\"", [Token::StringOpen])]
    #[case("\"\"", [Token::StringOpen, Token::StringClose])]
    #[case(r#"" ""#, [Token::StringOpen, Token::StringLiteral(" ".to_string()), Token::StringClose])]
    #[case(r#"" foo""#, [Token::StringOpen, Token::StringLiteral(" foo".to_string()), Token::StringClose])]
    #[case(r#""foo"#, [Token::StringOpen, Token::StringLiteral("foo".to_string())])]
    #[case(r#""foo""#, [Token::StringOpen, Token::StringLiteral("foo".to_string()), Token::StringClose])]
    #[case(
        r#""fo\to""#, 
        [
            Token::StringOpen,
            Token::StringLiteral("fo".to_string()), 
            Token::StringLiteral("\t".to_string()), 
            Token::StringLiteral("o".to_string()), 
            Token::StringClose,
        ]
    )]
    #[case(r#""foo $"#, [Token::StringOpen, Token::StringLiteral("foo $".to_string())])]
    #[case(
        r#""fo${if}o""#, 
        [
            Token::StringOpen,
            Token::StringLiteral("fo".to_string()), 
            Token::DollarLeftBrace,
            Token::If,
            Token::RightBrace,
            Token::StringLiteral("o".to_string()), 
            Token::StringClose,
        ]
    )]
    #[case(
        r#""fo${if"bar"}o""#, 
        [
            Token::StringOpen,
            Token::StringLiteral("fo".to_string()), 
            Token::DollarLeftBrace,
            Token::If,
            Token::StringOpen,
            Token::StringLiteral("bar".to_string()), 
            Token::StringClose,
            Token::RightBrace,
            Token::StringLiteral("o".to_string()), 
            Token::StringClose,
        ]
    )]
    #[case(
        r#""fo${{}}o""#, 
        [
            Token::StringOpen,
            Token::StringLiteral("fo".to_string()), 
            Token::DollarLeftBrace,
            Token::LeftBrace,
            Token::RightBrace,
            Token::RightBrace,
            Token::StringLiteral("o".to_string()), 
            Token::StringClose,
        ]
    )]
    #[case(
        r#""fo${{}o""#, 
        [
            Token::StringOpen,
            Token::StringLiteral("fo".to_string()), 
            Token::DollarLeftBrace,
            Token::LeftBrace,
            Token::RightBrace,
            Token::Identifier("o".to_string()), // uneven braces
            Token::StringOpen,
        ]
    )]
    #[case(
        r#""fo\${}o""#, 
        [
            Token::StringOpen,
            Token::StringLiteral("fo".to_string()), 
            Token::StringLiteral("$".to_string()),
            Token::StringLiteral("{}o".to_string()),
            Token::StringClose,
        ]
    )]
    fn string_literals<const N: usize>(#[case] source: &str, #[case] expected: [Token; N]) {
        assert_tokens(source, expected);
    }

    #[rstest]
    #[case("`foo`", [Token::StringOpen, Token::StringLiteral("foo".to_string()), Token::StringClose])]
    #[case("` foo`", [Token::StringOpen, Token::StringLiteral(" foo".to_string()), Token::StringClose])]
    #[case("`foo", [Token::StringOpen, Token::StringLiteral("foo".to_string())])]
    #[case("#`foo`#", [Token::StringOpen, Token::StringLiteral("foo".to_string()), Token::StringClose])]
    #[case("###`foo`###", [Token::StringOpen, Token::StringLiteral("foo".to_string()), Token::StringClose])]
    #[case("#`foo`", [Token::StringOpen, Token::StringLiteral("foo`".to_string())])]
    #[case("#`fo ` o`#", [Token::StringOpen, Token::StringLiteral("fo ` o".to_string()), Token::StringClose])]
    #[case("##`fo `# o`##", [Token::StringOpen, Token::StringLiteral("fo `# o".to_string()), Token::StringClose])]
    #[case(r"`fo\no`", [Token::StringOpen, Token::StringLiteral(r"fo\no".to_string()), Token::StringClose])]
    fn raw_string_literals<const N: usize>(#[case] source: &str, #[case] expected: [Token; N]) {
        assert_tokens(source, expected);
    }
    #[rstest]
    #[case("// foo", [Token::LineCommentOpen, Token::CommentLiteral(" foo".to_string())])]
    #[case("// foo\n", [Token::LineCommentOpen, Token::CommentLiteral(" foo".to_string()), Token::CommentClose])]
    #[case("// foo */", [Token::LineCommentOpen, Token::CommentLiteral(" foo */".to_string())])]
    #[case("/* foo", [Token::BlockCommentOpen, Token::CommentLiteral(" foo".to_string())])]
    #[case("/* foo */", [Token::BlockCommentOpen, Token::CommentLiteral(" foo ".to_string()), Token::CommentClose])]
    #[case("/* foo\n", [Token::BlockCommentOpen, Token::CommentLiteral(" foo\n".to_string())])]
    fn comments<const N: usize>(#[case] source: &str, #[case] expected: [Token; N]) {
        assert_tokens(source, expected);
    }

    #[rstest]
    #[case("@", [Err(TokenizationError::InvalidToken("@".to_string()))])]
    #[case("@a", [Err(TokenizationError::InvalidToken("@".to_string()))])] // everything ignored after error
    #[case("${", [Err(TokenizationError::InvalidToken("$".to_string()))])] // invalid outside string literal
    #[case(r#""\ ""#, [Ok(Token::StringOpen), Err(TokenizationError::NoEscape)])]
    // all possible parsing errors are tested in the parsing.rs file, only proper error propagation is tested here
    #[case(r#""\a""#, [Ok(Token::StringOpen), Err(TokenizationError::EscapeParse(EscapeParseError::InvalidEscape(r"\a".to_string())))])]
    #[case(r#""\u{g}""#, [Ok(Token::StringOpen), Err(TokenizationError::UnicodeParse(UnicodeParseError::InvalidHex("g".to_string())))])]
    #[case("0a123", [Err(TokenizationError::NumberParse(NumberParseError::InvalidRadix("a".to_string())))])]
    fn errors<const N: usize>(
        #[case] source: &str,
        #[case] expected: [Result<Token, TokenizationError>; N],
    ) {
        assert_results(source, expected);
    }

    #[rstest]
    #[case("", [])]
    #[case("\t", [])] // character tabulation (u+0009)
    #[case("\n", [])] // line feed (u+000a)
    #[case("\u{000B}", [])] // line tabulation / vertical tab (u+000b)
    #[case("\u{000C}", [])] // form feed (u+000c)
    #[case("\r", [])] // carriage return (u+000d)
    #[case(" ", [])] // space (u+0020)
    #[case("\u{0085}", [])] // next line (nel) (u+0085)
    #[case("\u{00A0}", [])] // no-break space (u+00a0)
    #[case("\u{1680}", [])] // ogham space mark (u+1680)
    #[case("\u{2000}", [])] // en quad (u+2000)
    #[case("\u{2001}", [])] // em quad (u+2001)
    #[case("\u{2002}", [])] // en space (u+2002)
    #[case("\u{2003}", [])] // em space (u+2003)
    #[case("\u{2004}", [])] // three-per-em space (u+2004)
    #[case("\u{2005}", [])] // four-per-em space (u+2005)
    #[case("\u{2006}", [])] // six-per-em space (u+2006)
    #[case("\u{2007}", [])] // figure space (u+2007)
    #[case("\u{2008}", [])] // punctuation space (u+2008)
    #[case("\u{2009}", [])] // thin space (u+2009)
    #[case("\u{200A}", [])] // hair space (u+200a)
    #[case("\u{2028}", [])] // line separator (u+2028)
    #[case("\u{2029}", [])] // paragraph separator (u+2029)
    #[case("\u{202F}", [])] // narrow no-break space (u+202f)
    #[case("\u{205F}", [])] // medium mathematical space (u+205f)
    #[case("\u{3000}", [])] // ideographic space (u+3000)
    #[case(" \t\n", [])] // multiple spaces
    #[case(" let \tif ", [Token::Let, Token::If])]
    fn ignore_spaces<const N: usize>(#[case] source: &str, #[case] expected: [Token; N]) {
        assert_tokens(source, expected);
    }

    #[rstest]
    #[case(
        "let a = 5", 
        [
            Token::Let,
            Token::Identifier("a".to_string()),
            Token::Equal,
            Token::IntLiteral(5),
        ]
    )]
    #[case(
        "(5).attribute", 
        [
            Token::LeftParen,
            Token::IntLiteral(5),
            Token::RightParen,
            Token::Dot,
            Token::Identifier("attribute".to_string()),
        ]
    )]
    #[case(
        "(5.6).attribute", 
        [
            Token::LeftParen,
            Token::FloatLiteral(5.6),
            Token::RightParen,
            Token::Dot,
            Token::Identifier("attribute".to_string()),
        ]
    )]
    #[case(
        ".!<=+", 
        [
            Token::Dot,
            Token::Bang,
            Token::LessEqual,
            Token::Plus,
        ]
    )]
    fn free_text<const N: usize>(#[case] source: &str, #[case] expected: [Token; N]) {
        assert_tokens(source, expected);
    }
}
