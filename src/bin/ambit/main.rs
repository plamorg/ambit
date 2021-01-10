mod directories;

use clap::{App, Arg, SubCommand};

use std::process;

use ambit::error::{self, AmbitError, AmbitResult};
use directories::AMBIT_PATHS;

// Initialize config and repository directory
fn create_paths(force: bool) -> AmbitResult<()> {
    if !AMBIT_PATHS.config.exists() {
        AMBIT_PATHS.config.create()?;
    }
    if AMBIT_PATHS.repo.exists() && !force {
        // Dotfile repository should not be overwritten if force is false
        return Err(AmbitError::Other(
            "Dotfile repository already exists.\nUse '-f' flag to overwrite.".to_string(),
        ));
    } else if AMBIT_PATHS.repo.exists() {
        // Repository directory exists but force is enabled
        AMBIT_PATHS.repo.remove()?;
    }
    Ok(())
}

mod cmd {
    use super::create_paths;
    use ambit::error::{AmbitError, AmbitResult};
    use std::io::{self, Write};
    use std::process::Command;

    use super::directories::AMBIT_PATHS;

    // Initialize an empty dotfile repository
    pub fn init(force: bool) -> AmbitResult<()> {
        match create_paths(force) {
            Ok(()) => {
                AMBIT_PATHS.repo.create()?;
                // Initialize an empty git repository
                git(vec!["init"])?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    // Clone an existing dotfile repository with given origin
    pub fn clone(force: bool, origin: &str) -> AmbitResult<()> {
        match create_paths(force) {
            Ok(()) => {
                // Clone will handle creating the repository directory
                let status = Command::new("git")
                    .args(&["clone", origin, AMBIT_PATHS.repo.to_str()?])
                    .status()?;
                if status.success() {
                    println!("Successfully cloned repository with origin: {}", origin);
                    return Ok(());
                }
                Err(AmbitError::Other(format!(
                    "Failed to clone repository with origin: {}",
                    origin
                )))
            }
            Err(e) => Err(e),
        }
    }

    // Parse configuration to identify files that are absent from the dotfile repository
    pub fn validate() -> AmbitResult<()> {
        unimplemented!();
        // TODO: implement validate
    }

    // Run git commands from the dotfile repository
    pub fn git(arguments: Vec<&str>) -> AmbitResult<()> {
        // The path to repository (git-dir) and the working tree (work-tree) is
        // passed to ensure that git commands are run from the dotfile repository
        let output = Command::new("git")
            .args(&[
                ["--git-dir=", AMBIT_PATHS.git.to_str()?].concat(),
                ["--work-tree=", AMBIT_PATHS.repo.to_str()?].concat(),
            ])
            .args(&arguments)
            .output()?;
        io::stdout().write_all(&output.stdout)?;
        io::stdout().write_all(&output.stderr)?;
        Ok(())
    }
}

fn run() -> AmbitResult<()> {
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
        cmd::init(force)?;
    }
    if let Some(matches) = matches.subcommand_matches("clone") {
        let force = matches.is_present("force");
        let origin = matches.value_of("ORIGIN").unwrap_or("");
        cmd::clone(force, origin)?;
    }
    if matches.is_present("validate") {
        cmd::validate()?;
    }
    if let Some(matches) = matches.subcommand_matches("git") {
        let git_arguments: Vec<_> = matches.values_of("GIT_ARGUMENTS").unwrap().collect();
        cmd::git(git_arguments)?;
    }
    Ok(())
}

fn main() {
    let result = run();
    match result {
        Err(error) => error::default_error_handler(&error),
        Ok(()) => process::exit(0),
    };
}
