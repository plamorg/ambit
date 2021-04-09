pub mod ast;
pub mod lexer;
pub mod parser;
pub mod strgen;

pub use ast::Entry;
use lexer::Lexer;
pub use parser::Parser;

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::{
    directories::AmbitPath,
    error::{AmbitError, AmbitResult},
};

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum ParseErrorType {
    Expected(&'static [lexer::TokType]),
    Custom(&'static str),
    Lex(&'static str),
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

pub fn get_entries(config_path: &AmbitPath) -> AmbitResult<Vec<Entry>> {
    Parser::new(Lexer::new(config_path.as_string()?.chars().peekable()).peekable())
        .collect::<Result<Vec<_>, _>>()
        .map_err(AmbitError::Parse)
}
