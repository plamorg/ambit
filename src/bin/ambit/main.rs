mod directories;

use clap::{App, Arg, SubCommand};

use std::io::{self, Write};
use std::process::Command;

use directories::AMBIT_PATHS;

// Initialize config and repository directory
fn initialize(force: bool) -> Result<(), &'static str> {
    if !AMBIT_PATHS.config.exists() {
        AMBIT_PATHS.config.create();
    }
    if AMBIT_PATHS.repo.exists() && !force {
        // Dotfile repository should not be overwritten if force is false
        return Err("ERROR: Dotfile repository already exists.\nUse '-f' flag to overwrite.");
    } else if AMBIT_PATHS.repo.exists() {
        // Repository directory exists but force is enabled
        AMBIT_PATHS.repo.remove();
    }
    Ok(())
}

// Initialize an empty dotfile repository
fn init(force: bool) {
    match initialize(force) {
        Ok(()) => {
            AMBIT_PATHS.repo.create();
            // Initialize an empty git repository
            git(vec!["init"]);
        }
        Err(e) => eprintln!("{}", e),
    };
}

// Clone an existing dotfile repository with given origin
fn clone(force: bool, origin: &str) {
    match initialize(force) {
        Ok(()) => {
            // Clone will handle creating the repository directory
            let status = Command::new("git")
                .args(&["clone", origin, AMBIT_PATHS.repo.to_str()])
                .status()
                .expect("Failed to clone repository");
            if status.success() {
                println!(
                    "Successfully initialized repository with origin: {}",
                    origin
                );
            }
        }
        Err(e) => eprintln!("{}", e),
    };
}

// Parse configuration to identify files that are absent from the dotfile repository
fn validate() {
    unimplemented!();
    // TODO: implement validate
}

// Run git commands from the dotfile repository
fn git(arguments: Vec<&str>) {
    // The path to repository (git-dir) and the working tree (work-tree) is
    // passed to ensure that git commands are run from the dotfile repository
    let output = Command::new("git")
        .args(&[
            ["--git-dir=", AMBIT_PATHS.git.to_str()].concat(),
            ["--work-tree=", AMBIT_PATHS.repo.to_str()].concat(),
        ])
        .args(&arguments)
        .output()
        .expect("Failed to execute git command");
    io::stdout().write_all(&output.stdout).unwrap();
    io::stdout().write_all(&output.stderr).unwrap();
}

fn main() {
    let force_arg = Arg::with_name("force")
        .short("f")
        .long("force")
        .help("Overwrite currently initialized dotfile repository");

    let matches = App::new("ambit")
        .about("Dotfile manager")
        .subcommand(
            SubCommand::with_name("init")
                .about("Initialize an empty dotfile repository")
                .arg(&force_arg),
        )
        .subcommand(
            SubCommand::with_name("clone")
                .about("Clone an existing dotfile repository with given origin")
                .arg(&force_arg)
                .arg(Arg::with_name("ORIGIN").index(1).required(true)),
        )
        .subcommand(SubCommand::with_name("validate").about(
            "Parse configuration to identify files that are absent from the dotfile repository",
        ))
        .subcommand(
            SubCommand::with_name("git")
                .about("Run git commands from the dotfile repository")
                .arg(Arg::with_name("GIT_ARGUMENTS").required(true).min_values(1)),
        )
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("init") {
        let force = matches.is_present("force");
        init(force);
    }
    if let Some(matches) = matches.subcommand_matches("clone") {
        let force = matches.is_present("force");
        let origin = matches.value_of("ORIGIN").unwrap_or("");
        clone(force, origin);
    }
    if matches.is_present("validate") {
        validate();
    }
    if let Some(matches) = matches.subcommand_matches("git") {
        let git_arguments: Vec<_> = matches.values_of("GIT_ARGUMENTS").unwrap().collect();
        git(git_arguments);
    }
}
