use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::{io, process};

use crate::config;

pub type AmbitResult<T> = Result<T, AmbitError>;

#[derive(Debug)]
pub enum AmbitError {
    Io(io::Error),
    // TODO: As of now, a single ParseError is returned from config::get_entries
    //       Future changes may result in a Vec<ParseError> being returned.
    //       This should be taken care of.
    Parse(config::ParseError),
    Other(String),
}

impl Error for AmbitError {}

impl Display for AmbitError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            AmbitError::Io(ref e) => e.fmt(f),
            AmbitError::Parse(ref e) => e.fmt(f),
            AmbitError::Other(ref s) => f.write_str(&**s),
        }
    }
}

impl From<io::Error> for AmbitError {
    fn from(err: io::Error) -> AmbitError {
        AmbitError::Io(err)
    }
}

impl From<String> for AmbitError {
    fn from(err: String) -> AmbitError {
        AmbitError::Other(err)
    }
}

impl<'a> From<&'a str> for AmbitError {
    fn from(err: &'a str) -> AmbitError {
        AmbitError::Other(err.to_owned())
    }
}

// Report given error
pub fn default_error_handler(error: &AmbitError) {
    eprintln!("ERROR: {}", error);
    process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_io() {
        let err = AmbitError::Io(io::Error::new(io::ErrorKind::NotFound, "File not found"));
        assert_eq!(format!("{}", err), "File not found");
    }

    #[test]
    fn display_other() {
        let err = AmbitError::Other("Error message".to_string());
        assert_eq!(format!("{}", err), "Error message");
    }
}
