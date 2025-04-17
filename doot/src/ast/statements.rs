use super::expressions::Expression;

#[derive(Debug)]
pub enum Statement {
    Block(Vec<Statement>),
    Expression(Expression),
}
