use crate::{
    diagnostics::Diagnostic,
    frontend::syntax::{statements::Program, tokens::Token},
};

pub struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<Program, Diagnostic> {
        let _ = self.tokens;
        let _ = self.pos;
        todo!("parser scaffold only")
    }
}
