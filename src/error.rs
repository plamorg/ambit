use std::process;
use std::{fmt, io};

pub type AmbitResult<T> = Result<T, AmbitError>;

#[derive(Debug)]
pub enum AmbitError {
    Io(io::Error),
    Other(String),
}

impl fmt::Display for AmbitError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            AmbitError::Io(ref e) => e.fmt(f),
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
