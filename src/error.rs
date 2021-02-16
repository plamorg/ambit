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
    // File error is encountered on failed file open operation
    // Provides additional path information
    File { path: String, error: io::Error },
    Other(String),
}

impl Error for AmbitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AmbitError::File { error, .. } => Some(error),
            _ => None,
        }
    }
}

impl Display for AmbitError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let result = match self {
            AmbitError::Io(ref e) => e.fmt(f),
            AmbitError::Parse(ref e) => e.fmt(f),
            AmbitError::File { path, .. } => f.write_fmt(format_args!("Failed to read `{}`", path)),
            AmbitError::Other(ref s) => f.write_str(s.as_str()),
        };
        if result.is_err() {
            // Error encountered from previous match
            return result;
        } else if let Some(source) = self.source() {
            // Report error with additional causation if there is a source
            f.write_fmt(format_args!("\n\nCaused by:\n  {}", source))?;
        }
        Ok(())
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
    fn display_file() {
        let err = AmbitError::File {
            path: "path".to_string(),
            error: io::Error::new(io::ErrorKind::PermissionDenied, "Permission denied"),
        };
        assert_eq!(
            format!("{}", err),
            r#"Failed to read `path`

Caused by:
  Permission denied"#
        )
    }

    #[test]
    fn display_other() {
        let err = AmbitError::Other("Error message".to_string());
        assert_eq!(format!("{}", err), "Error message");
    }
}
