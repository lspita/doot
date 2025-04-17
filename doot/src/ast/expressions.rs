#[derive(Debug, PartialEq)]
pub enum Expression {
    Literal(Literal),
    String(Vec<Expression>),
    Unary(UnaryOperation, Box<Expression>),
    Binary(BinaryOperation, Box<Expression>, Box<Expression>),
}

#[derive(Debug, PartialEq)]
pub enum Literal {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}

#[derive(Debug, PartialEq)]
pub enum UnaryOperation {
    Negate,     // !foo
    InvertSign, // -foo
    Optional,   // foo?
}

#[derive(Debug, PartialEq)]
pub enum BinaryOperation {
    Sum,          // foo + bar
    Subtract,     // foo - bar
    Multiply,     // foo * bar
    Divide,       // foo / bar
    Module,       // foo % bar
    Equal,        // foo == bar
    NotEqual,     // foo != bar
    Greater,      // foo > bar
    GreaterEqual, // foo >= bar
    Less,         // foo < bar
    LessEqual,    // foo <= bar
    And,          // foo && bar
    Or,           // foo || bar
}
