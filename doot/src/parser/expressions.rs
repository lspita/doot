use crate::{
    ast::expressions::{BinaryOperation, Expression, Literal, UnaryOperation},
    lexer::tokens::Token,
};

use super::{BindingPower, ParseError, Parser, literals};

pub(super) struct NUD<'a> {
    bp: BindingPower,
    op: Box<dyn FnOnce(&mut Parser, BindingPower) -> Result<Expression, ParseError> + 'a>,
}

impl<'a> NUD<'a> {
    pub fn bp(&self) -> &BindingPower {
        &self.bp
    }

    pub fn parse(self, parser: &mut Parser) -> Result<Box<Expression>, ParseError> {
        (self.op)(parser, self.bp).map(Box::new)
    }

    fn new(
        bp: BindingPower,
        op: impl FnOnce(&mut Parser, BindingPower) -> Result<Expression, ParseError> + 'a,
    ) -> Option<Self> {
        Some(Self {
            bp,
            op: Box::new(op),
        })
    }

    fn literal(op: impl FnOnce() -> Result<Literal, ParseError> + 'a) -> Option<Self> {
        NUD::new(BindingPower::Literal, |_, _| op().map(Expression::Literal))
    }

    fn simple_literal(value: Literal) -> Option<Self> {
        NUD::literal(|| Ok(value))
    }

    fn prefix(op: UnaryOperation) -> Option<Self> {
        NUD::new(BindingPower::Prefix, |p, bp| {
            Ok(Expression::Unary(op, p.parse_expression_capped(bp)?))
        })
    }
}

pub(super) struct LED<'a> {
    bp: BindingPower,
    op: Box<
        dyn FnOnce(&mut Parser, BindingPower, Box<Expression>) -> Result<Expression, ParseError>
            + 'a,
    >,
}

impl<'a> LED<'a> {
    pub fn parse(
        self,
        parser: &mut Parser,
        left: Box<Expression>,
    ) -> Result<Box<Expression>, ParseError> {
        (self.op)(parser, self.bp, left).map(Box::new)
    }

    pub fn bp(&self) -> &BindingPower {
        &self.bp
    }

    fn new(
        bp: BindingPower,
        op: impl FnOnce(&mut Parser, BindingPower, Box<Expression>) -> Result<Expression, ParseError>
        + 'a,
    ) -> Option<Self> {
        Some(Self {
            bp,
            op: Box::new(op),
        })
    }

    fn binary(bp: BindingPower, op: BinaryOperation) -> Option<Self> {
        LED::new(bp, |p, bp, left| {
            p.parse_expression_capped(bp)
                .map(|right| Expression::Binary(op, left, right))
        })
    }

    fn postfix(op: UnaryOperation) -> Option<Self> {
        LED::new(BindingPower::Postfix, |_, _, left| {
            Ok(Expression::Unary(op, left))
        })
    }
}

pub(super) fn nud<'a>(token: &'a Token) -> Option<NUD<'a>> {
    match token {
        Token::Null => NUD::simple_literal(Literal::Null),
        Token::BoolLiteral(value) => NUD::simple_literal(Literal::Bool(*value)),
        Token::IntLiteral(value) => NUD::literal(move || {
            literals::parse_int(&value)
                .map(|value| Literal::Int(value))
                .map_err(ParseError::Number)
        }),
        Token::FloatLiteral(value) => NUD::literal(move || {
            literals::parse_float(&value)
                .map(|value| Literal::Float(value))
                .map_err(ParseError::Number)
        }),
        Token::StringLiteral(value) => NUD::simple_literal(Literal::Text(value.clone())),
        _ => None,
    }
}
pub(super) fn led<'a>(token: &Token) -> Option<LED<'a>> {
    match token {
        Token::Plus => LED::binary(BindingPower::Additive, BinaryOperation::Sum),
        Token::Asterisk => LED::binary(BindingPower::Multiplicative, BinaryOperation::Multiply),
        _ => None,
    }
}
