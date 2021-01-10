use lazy_static::lazy_static;
use std::fs::{self, File};
use std::path::PathBuf;

use ambit::error::{AmbitError, AmbitResult};

#[derive(PartialEq, Eq, Debug)]
pub enum AmbitPathKind {
    FILE,
    DIRECTORY,
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
            AmbitPathKind::FILE => self.path.is_file(),
            AmbitPathKind::DIRECTORY => self.path.is_dir(),
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

    pub fn create(&self) -> AmbitResult<()> {
        match self.kind {
            AmbitPathKind::FILE => {
                File::create(&self.path)?;
            }
            AmbitPathKind::DIRECTORY => {
                fs::create_dir_all(&self.path)?;
            }
        };
        Ok(())
    }

    pub fn remove(&self) -> AmbitResult<()> {
        match self.kind {
            AmbitPathKind::FILE => fs::remove_file(&self.path)?,
            AmbitPathKind::DIRECTORY => fs::remove_dir_all(&self.path)?,
        };
        Ok(())
    }
}

pub struct AmbitPaths {
    pub config: AmbitPath,
    pub repo: AmbitPath,
    pub git: AmbitPath,
}

impl AmbitPaths {
    fn new() -> AmbitPaths {
        let home = dirs::home_dir().expect("Could not get home directory");
        let configuration = home.join(".config/ambit");

        let config = AmbitPath::new(configuration.join("config"), AmbitPathKind::FILE);
        let repo = AmbitPath::new(configuration.join("repo"), AmbitPathKind::DIRECTORY);
        let git = AmbitPath::new(configuration.join("repo/.git"), AmbitPathKind::DIRECTORY);

        AmbitPaths { config, repo, git }
    }
}

lazy_static! {
    pub static ref AMBIT_PATHS: AmbitPaths = AmbitPaths::new();
}
