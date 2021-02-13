pub mod ast;
pub mod lexer;
pub mod parser;

pub use ast::Entry;
use lexer::Lexer;
pub use parser::Parser;

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::iter::Peekable;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum ParseErrorType {
    Expected(&'static [lexer::TokType]),
    Custom(&'static str),
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct ParseError {
    pub ty: ParseErrorType,
    // Some(_) if it failed at a token, or None if it failed at EOF.
    pub tok: Option<lexer::Token>,
}

impl Error for ParseError {}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        // TODO: Output parse error nicely
        write!(f, "{:?}", self)
    }
}

impl From<ParseErrorType> for ParseError {
    fn from(ty: ParseErrorType) -> Self {
        Self { ty, tok: None }
    }
}

pub type ParseResult<T> = std::result::Result<T, ParseError>;

pub fn get_entries<I: Iterator<Item = char>>(char_iter: Peekable<I>) -> Parser<Lexer<I>> {
    let lex = Lexer::new(char_iter);
    Parser::new(lex.peekable())
}
