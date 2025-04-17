use std::{error::Error, fmt::Display, iter::Peekable};

use binding::BindingPower;
use literals::NumberParseError;

use crate::{
    ast::{expressions::Expression, statements::Statement},
    lexer::tokens::Token,
};

mod binding;
mod expressions;
mod literals;

#[derive(Debug)]
pub enum ParseError {
    ExpectedAny,
    Expected(Token),
    InvalidToken(Token),
    Number(NumberParseError),
}

impl Error for ParseError {}
impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        "".fmt(f)
    }
}

pub struct Parser<'a> {
    source: Peekable<Box<dyn Iterator<Item = Token> + 'a>>,
}

impl<'a> Parser<'a> {
    pub fn new(source: Box<dyn Iterator<Item = Token> + 'a>) -> Self {
        Self {
            source: source.peekable(),
        }
    }

    pub fn parse(&mut self) -> Result<Statement, ParseError> {
        Ok(Statement::Block(vec![]))
    }

    fn consume(&mut self) -> Result<Token, ParseError> {
        self.source.next().ok_or(ParseError::ExpectedAny)
    }

    fn peek(&mut self) -> Option<Token> {
        self.source.peek().cloned()
    }

    fn expect(&mut self, token: Token) -> Result<Token, ParseError> {
        self.source
            .next()
            .filter(|t| token.eq(t))
            .ok_or(ParseError::Expected(token))
    }

    fn invalid_token_op(token: &Token) -> impl FnOnce() -> ParseError {
        || ParseError::InvalidToken(token.clone())
    }

    fn parse_expression(&mut self) -> Result<Box<Expression>, ParseError> {
        self.parse_expression_capped(BindingPower::Default)
    }

    fn parse_expression_capped(
        &mut self,
        min_bp: BindingPower,
    ) -> Result<Box<Expression>, ParseError> {
        let token = self.consume()?;
        let parselet = expressions::nud(&token).ok_or_else(Self::invalid_token_op(&token))?;
        let mut left = parselet.parse(self)?;
        while let Some(token) = self.peek() {
            let parselet = expressions::led(&token).ok_or_else(Self::invalid_token_op(&token))?;
            if parselet.bp() > &min_bp {
                self.consume()?;
                left = parselet.parse(self, left)?;
            } else {
                break;
            }
        }
        return Ok(left);
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::{
        ast::expressions::{BinaryOperation, Expression, Literal},
        lexer::tokens::Token,
    };

    use super::Parser;

    #[rstest]
    #[case(
        [
            Token::IntLiteral("1".to_string()),
            Token::Plus,
            Token::IntLiteral("2".to_string())
        ],
        Box::new(Expression::Binary(
            BinaryOperation::Sum,
            Box::new(Expression::Literal(Literal::Int(1))),
            Box::new(Expression::Literal(Literal::Int(2)))
        ))
    )]
    #[case(
        [
            Token::IntLiteral("1".to_string()), 
            Token::Plus,
            Token::IntLiteral("2".to_string()),
            Token::Asterisk,
            Token::IntLiteral("3".to_string()),
        ],
        Box::new(Expression::Binary(
            BinaryOperation::Sum,
            Box::new(Expression::Literal(Literal::Int(1))),
            Box::new(Expression::Binary(
                BinaryOperation::Multiply,
                Box::new(Expression::Literal(Literal::Int(2))),
                Box::new(Expression::Literal(Literal::Int(3))),
            )),
        ))
    )]
    #[case(
        [
            Token::IntLiteral("1".to_string()), 
            Token::Asterisk,
            Token::IntLiteral("2".to_string()),
            Token::Plus,
            Token::IntLiteral("3".to_string()),
        ],
        Box::new(Expression::Binary(
            BinaryOperation::Sum,
            Box::new(Expression::Binary(
                BinaryOperation::Multiply,
                Box::new(Expression::Literal(Literal::Int(1))),
                Box::new(Expression::Literal(Literal::Int(2))),
            )),
            Box::new(Expression::Literal(Literal::Int(3))),
        ))
    )]
    #[case(
        [
            Token::IntLiteral("1".to_string()), 
            Token::Plus,
            Token::IntLiteral("2".to_string()),
            Token::Asterisk,
            Token::IntLiteral("3".to_string()),
            Token::Plus,
            Token::IntLiteral("4".to_string()),
        ],
        Box::new(Expression::Binary(
            BinaryOperation::Sum,
            Box::new(Expression::Binary(
                BinaryOperation::Sum,
                Box::new(Expression::Literal(Literal::Int(1))),
                Box::new(Expression::Binary(
                    BinaryOperation::Multiply,
                    Box::new(Expression::Literal(Literal::Int(2))),
                    Box::new(Expression::Literal(Literal::Int(3))),
                )),
            )),
            Box::new(Expression::Literal(Literal::Int(4))),
        ))
    )]
    fn parse_expression_ok<const N: usize>(
        #[case] tokens: [Token; N],
        #[case] expected: Box<Expression>,
    ) {
        let mut parser = Parser::new(Box::new(tokens.into_iter()));
        assert_eq!(expected, parser.parse_expression().unwrap())
    }
}
