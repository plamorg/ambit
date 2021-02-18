use lazy_static::lazy_static;
use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;

use ambit::error::{AmbitError, AmbitResult};

#[derive(PartialEq, Eq, Debug)]
pub enum AmbitPathKind {
    File,
    Directory,
}

#[derive(PartialEq, Eq, Debug)]
pub struct AmbitPath {
    pub path: PathBuf,
    kind: AmbitPathKind,
}

impl AmbitPath {
    pub fn new(path: PathBuf, kind: AmbitPathKind) -> AmbitPath {
        AmbitPath { path, kind }
    }

    pub fn exists(&self) -> bool {
        match self.kind {
            AmbitPathKind::File => self.path.is_file(),
            AmbitPathKind::Directory => self.path.is_dir(),
        }
    }

    pub fn to_str(&self) -> AmbitResult<&str> {
        // Converts path to string slice representation
        let result = self.path.to_str();
        match result {
            Some(e) => Ok(e),
            None => Err(AmbitError::Other(
                "Could not yield path as &str slice".to_string(),
            )),
        }
    }

    // Fetch the content of a path if it is AmbitPathKind::File
    pub fn as_string(&self) -> AmbitResult<String> {
        match self.kind {
            AmbitPathKind::File => {
                let mut file = match File::open(&self.path) {
                    Ok(file) => file,
                    Err(e) => {
                        return Err(AmbitError::File {
                            path: String::from(self.to_str()?),
                            error: e,
                        })
                    }
                };
                let mut content = String::new();
                file.read_to_string(&mut content)?;
                Ok(content)
            }
            AmbitPathKind::Directory => Err(AmbitError::Other(
                "Getting content of a directory is not supported".to_owned(),
            )),
        }
    }

    pub fn create(&self) -> AmbitResult<()> {
        match self.kind {
            AmbitPathKind::File => {
                File::create(&self.path)?;
            }
            AmbitPathKind::Directory => {
                fs::create_dir_all(&self.path)?;
            }
        };
        Ok(())
    }

    pub fn remove(&self) -> AmbitResult<()> {
        match self.kind {
            AmbitPathKind::File => fs::remove_file(&self.path)?,
            AmbitPathKind::Directory => fs::remove_dir_all(&self.path)?,
        };
        Ok(())
    }
}

pub struct AmbitPaths {
    pub home: AmbitPath,
    pub config: AmbitPath,
    pub repo: AmbitPath,
    pub git: AmbitPath,
}

impl AmbitPaths {
    fn new() -> AmbitPaths {
        let home = dirs::home_dir().expect("Could not get home directory");
        let configuration = home.join(".config/ambit");

        AmbitPaths {
            home: AmbitPath::new(home, AmbitPathKind::Directory),
            config: AmbitPath::new(configuration.join("config.ambit"), AmbitPathKind::File),
            repo: AmbitPath::new(configuration.join("repo"), AmbitPathKind::Directory),
            git: AmbitPath::new(
                configuration.join("repo").join(".git"),
                AmbitPathKind::Directory,
            ),
        }
    }
}

lazy_static! {
    pub static ref AMBIT_PATHS: AmbitPaths = AmbitPaths::new();
}
