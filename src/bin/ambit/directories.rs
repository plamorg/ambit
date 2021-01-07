use lazy_static::lazy_static;
use std::path::{Path, PathBuf};

pub struct AmbitPaths {
    config_file: PathBuf,
    repo_dir: PathBuf,
    git_dir: PathBuf,
}

impl AmbitPaths {
    fn new() -> AmbitPaths {
        let home_dir = dirs::home_dir().expect("Could not get home directory");
        let configuration_dir = home_dir.join(".config/ambit");
        let config_file = configuration_dir.join("config");
        let repo_dir = configuration_dir.join("repo");
        let git_dir = repo_dir.join(".git");
        AmbitPaths {
            config_file,
            repo_dir,
            git_dir,
        }
    }

    pub fn config_file(&self) -> &Path {
        &self.config_file
    }

    pub fn repo_dir(&self) -> &Path {
        &self.repo_dir
    }

    pub fn repo_dir_str(&self) -> &str {
        &self
            .repo_dir
            .to_str()
            .expect("Could not yield repo directory as &str slice")
    }

    pub fn git_dir_str(&self) -> &str {
        &self
            .git_dir
            .to_str()
            .expect("Could not yield git directory as &str slice")
    }
}

lazy_static! {
    pub static ref AMBIT_PATHS: AmbitPaths = AmbitPaths::new();
}
