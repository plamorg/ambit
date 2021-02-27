use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    io,
    path::PathBuf,
    process,
};

use crate::config;

pub type AmbitResult<T> = Result<T, AmbitError>;

#[derive(Debug)]
pub enum AmbitError {
    Io(io::Error),
    // TODO: As of now, a single ParseError is returned from config::get_entries
    //       Future changes may result in a Vec<ParseError> being returned.
    //       This should be taken care of.
    Parse(config::ParseError),
    WalkDir(walkdir::Error),
    // File error is encountered on failed file open operation
    // Provides additional path information
    File {
        path: PathBuf,
        error: io::Error,
    },
    Sync {
        host_file_path: PathBuf,
        repo_file_path: PathBuf,
        error: Box<AmbitError>,
    },
    Other(String),
}

impl Error for AmbitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AmbitError::File { error, .. } => Some(error),
            AmbitError::Sync { error, .. } => Some(error),
            _ => None,
        }
    }
}

impl Display for AmbitError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            AmbitError::Io(ref e) => e.fmt(f),
            AmbitError::Parse(ref e) => e.fmt(f),
            AmbitError::WalkDir(ref e) => e.fmt(f),
            AmbitError::File { path, .. } => {
                f.write_fmt(format_args!("File error with `{}`", path.display()))
            }
            AmbitError::Sync {
                repo_file_path,
                host_file_path,
                ..
            } => f.write_fmt(format_args!(
                "Failed to symlink `{}` -> `{}`",
                host_file_path.display(),
                repo_file_path.display()
            )),
            AmbitError::Other(ref s) => f.write_str(s.as_str()),
        }?;
        if let Some(source) = self.source() {
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

impl From<walkdir::Error> for AmbitError {
    fn from(err: walkdir::Error) -> AmbitError {
        AmbitError::WalkDir(err)
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
            path: PathBuf::from("path"),
            error: io::Error::new(io::ErrorKind::PermissionDenied, "Permission denied"),
        };
        assert_eq!(
            format!("{}", err),
            r#"File error with `path`

Caused by:
  Permission denied"#
        )
    }

    #[test]
    fn display_symlink() {
        let err = AmbitError::Sync {
            host_file_path: PathBuf::from("host"),
            repo_file_path: PathBuf::from("repo"),
            error: Box::new(AmbitError::Other("Error message".to_owned())),
        };
        assert_eq!(
            format!("{}", err),
            r#"Failed to symlink `host` -> `repo`

Caused by:
  Error message"#
        );
    }

    #[test]
    fn display_other() {
        let err = AmbitError::Other("Error message".to_string());
        assert_eq!(format!("{}", err), "Error message");
    }
}
