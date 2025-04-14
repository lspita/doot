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

#[derive(Debug)]
pub struct SourceElement<T> {
    value: T,
    start: SourcePosition,
    stop: SourcePosition,
}

impl<T> SourceElement<T> {
    pub fn new(value: T, start: SourcePosition, stop: SourcePosition) -> Self {
        Self { value, start, stop }
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn start(&self) -> &SourcePosition {
        &self.start
    }

    pub fn stop(&self) -> &SourcePosition {
        &self.stop
    }
}
