use std::fmt::{Debug, Display};

pub mod lexer;

pub trait Source {
    fn name(&self) -> &str;
    fn chars(&self) -> impl Iterator<Item = char>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    line: u32,
    col: u32,
}

impl SourcePosition {
    pub fn new(line: u32, col: u32) -> Self {
        Self { line, col }
    }

    pub fn line(&self) -> u32 {
        self.line
    }

    pub fn col(&self) -> u32 {
        self.col
    }
}

impl Display for SourcePosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}
