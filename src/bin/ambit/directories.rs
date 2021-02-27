use lazy_static::lazy_static;
use std::{
    env,
    fs::{self, File},
    io::Read,
    path::PathBuf,
};

use ambit::error::{AmbitError, AmbitResult};

pub const CONFIG_NAME: &str = "config.ambit";

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
    pub fn new(path: PathBuf, kind: AmbitPathKind) -> Self {
        Self { path, kind }
    }

    pub fn exists(&self) -> bool {
        match self.kind {
            AmbitPathKind::File => self.path.is_file(),
            AmbitPathKind::Directory => self.path.is_dir(),
        }
    }

    pub fn ensure_parent_dirs_exist(&self) -> AmbitResult<()> {
        if let Some(parent) = &self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(())
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
                            path: self.path.clone(),
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
    fn new() -> Self {
        // Source home path from environment variable. This is mainly for integration testing purposes.
        let home_path = AmbitPaths::get_path_from_env("AMBIT_HOME_PATH")
            .unwrap_or_else(|| dirs::home_dir().expect("Could not get home directory"));

        let configuration_path = home_path.join(".config/ambit");

        let config_path = AmbitPaths::get_path_from_env("AMBIT_CONFIG_PATH")
            .unwrap_or_else(|| configuration_path.join(CONFIG_NAME));

        let repo_path = AmbitPaths::get_path_from_env("AMBIT_REPO_PATH")
            .unwrap_or_else(|| configuration_path.join("repo"));

        let git_path = repo_path.join(".git");

        Self {
            home: AmbitPath::new(home_path, AmbitPathKind::Directory),
            config: AmbitPath::new(config_path, AmbitPathKind::File),
            repo: AmbitPath::new(repo_path, AmbitPathKind::Directory),
            git: AmbitPath::new(git_path, AmbitPathKind::Directory),
        }
    }

    // Attempt to fetch path from env if set
    fn get_path_from_env(key: &str) -> Option<PathBuf> {
        match env::var_os(key) {
            Some(path) => Some(PathBuf::from(path)),
            None => None,
        }
    }
}

lazy_static! {
    pub static ref AMBIT_PATHS: AmbitPaths = AmbitPaths::new();
}
