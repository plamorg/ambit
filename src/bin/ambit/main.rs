mod directories;

use clap::{App, Arg, SubCommand};

use std::fs::{self, File};
use std::io::{self, Write};
use std::process::Command;

use directories::AMBIT_PATHS;

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

fn main() {
    let matches = App::new("ambit")
        .about("Dotfile manager")
        .subcommand(
            SubCommand::with_name("init")
                .about("Initializes the given origin as a dotfile repository or creates an empty")
                .arg(Arg::with_name("ORIGIN").index(1).required(false)),
        )
        .subcommand(SubCommand::with_name("validate").about(
            "Parses configuration to identify files that are absent from the dotfile repository",
        ))
        .subcommand(
            SubCommand::with_name("git")
                .about("Run git commands from the dotfile repository")
                .arg(Arg::with_name("GIT_ARGUMENTS").required(true).min_values(1)),
        )
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("init") {
        let origin = matches.value_of("ORIGIN").unwrap_or("");
        init(origin);
    }
    if matches.is_present("validate") {
        validate();
    }
    if let Some(matches) = matches.subcommand_matches("git") {
        let git_arguments: Vec<_> = matches.values_of("GIT_ARGUMENTS").unwrap().collect();
        git(git_arguments);
    }
}
