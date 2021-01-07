extern crate dirs;
extern crate lazy_static;

use lazy_static::lazy_static;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

mod lexer;

pub struct AmbitPaths {
    home_dir: PathBuf,
    configuration_dir: PathBuf,
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
            home_dir,
            configuration_dir,
            config_file,
            repo_dir,
            git_dir,
        }
    }

    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }

    pub fn configuration_dir(&self) -> &Path {
        &self.configuration_dir
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

    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    pub fn git_dir_str(&self) -> &str {
        &self
            .git_dir
            .to_str()
            .expect("Could not yield git directory as &str slice")
    }
}

pub fn init(origin: &str) {
    // Handle file and directory creation
    if AMBIT_PATHS.repo_dir().is_dir() {
        // Repo path already exists
        eprintln!(
            "Error: Dotfile repository has already been initialized at: {}",
            AMBIT_PATHS.repo_dir_str()
        );
    } else {
        // Create repo directory
        // create_dir_all will handle the case where ~/.config/ambit itself doesn't exist
        fs::create_dir_all(AMBIT_PATHS.repo_dir()).expect("Could not create repo directory");
    }
    if !AMBIT_PATHS.config_file().is_file() {
        // Create configuration file
        File::create(AMBIT_PATHS.config_file()).expect("Could not create config file");
    }

    // Handle git clone
    if origin.is_empty() {
        // Initialize empty repository
        git(vec!["init"]);
    } else {
        // Clone from origin
        let status = Command::new("git")
            .args(&["clone", origin, AMBIT_PATHS.repo_dir_str()])
            .status()
            .expect("Failed to clone repository");
        if status.success() {
            println!(
                "Successfully initialized repository with origin: {}",
                origin
            );
        }
    }
}

pub fn validate() {
    unimplemented!();
    // TODO: implement validate
}

pub fn git(arguments: Vec<&str>) {
    let output = Command::new("git")
        .args(&[
            ["--git-dir=", AMBIT_PATHS.git_dir_str()].concat(),
            ["--work-tree=", AMBIT_PATHS.repo_dir_str()].concat(),
        ])
        .args(&arguments)
        .output()
        .expect("Failed to execute git command");
    io::stdout().write_all(&output.stdout).unwrap();
    io::stdout().write_all(&output.stderr).unwrap();
}

lazy_static! {
    pub static ref AMBIT_PATHS: AmbitPaths = AmbitPaths::new();
}
