pub mod lexer;
pub mod parser;

use lexer::Lexer;
pub use parser::{Entry, Parser};

use std::iter::Peekable;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum ParseError {
    Expected(&'static [lexer::TokType]),
    Custom(&'static str),
}

pub type ParseResult<T> = std::result::Result<T, (Option<lexer::Token>, ParseError)>;

pub fn get_entries<I: Iterator<Item = char>>(char_iter: Peekable<I>) -> Parser<Lexer<I>> {
    let lex = Lexer::new(char_iter);
    Parser::new(lex.peekable())
}
