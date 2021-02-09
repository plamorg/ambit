mod directories;

use clap::{App, AppSettings, Arg, SubCommand};

use std::process;

use ambit::config;
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

// Fetch entries from config file and return as vector
fn get_config_entries() -> AmbitResult<Vec<config::parser::Entry>> {
    let content = AMBIT_PATHS.config.as_string()?;
    match config::get_entries(content.chars().peekable()).collect::<Result<Vec<_>, _>>() {
        Ok(entries) => Ok(entries),
        Err(e) => Err(AmbitError::Parse(e)),
    }
}

mod cmd {
    use super::directories::AMBIT_PATHS;
    use super::{create_paths, get_config_entries};

    use std::io::{self, Write};
    use std::process::Command;

    use ambit::error::{AmbitError, AmbitResult};

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
    pub fn clone(force: bool, arguments: Vec<&str>) -> AmbitResult<()> {
        match create_paths(force) {
            Ok(()) => {
                // Clone will handle creating the repository directory
                let repo_path = AMBIT_PATHS.repo.to_str()?;
                let status = Command::new("git")
                    .arg("clone")
                    .args(arguments)
                    // Pass in ambit repo path as last argument to ensure that it is always cloned to the known path
                    .arg(repo_path)
                    .status()?;
                if status.success() {
                    println!("Successfully cloned repository to {}", repo_path);
                    return Ok(());
                }
                Err(AmbitError::Other("Failed to clone repository".to_string()))
            }
            Err(e) => Err(e),
        }
    }

    // Check ambit configuration for errors
    pub fn check() -> AmbitResult<()> {
        get_config_entries()?;
        Ok(())
    }

    // Sync files in dotfile repository to system through symbolic links
    pub fn sync() -> AmbitResult<()> {
        unimplemented!();
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
            .args(arguments)
            .output()?;
        io::stdout().write_all(&output.stdout)?;
        io::stdout().write_all(&output.stderr)?;
        Ok(())
    }
}

// Return instance of ambit application
fn get_app() -> App<'static, 'static> {
    let force_arg = Arg::with_name("force")
        .short("f")
        .long("force")
        .help("Overwrite currently initialized dotfile repository");

    App::new("ambit")
        .about("Dotfile manager")
        .setting(AppSettings::ArgRequiredElseHelp)
        .setting(AppSettings::VersionlessSubcommands)
        .subcommand(
            SubCommand::with_name("init")
                .about("Initialize an empty dotfile repository")
                .arg(&force_arg),
        )
        .subcommand(
            SubCommand::with_name("clone")
                .arg(&force_arg)
                .about("Clone an existing dotfile repository with given origin")
                .setting(AppSettings::AllowLeadingHyphen)
                .arg(Arg::with_name("GIT_ARGUMENTS").required(true).min_values(1)),
        )
        .subcommand(
            SubCommand::with_name("git")
                .about("Run git commands from the dotfile repository")
                .setting(AppSettings::AllowLeadingHyphen)
                .arg(Arg::with_name("GIT_ARGUMENTS").required(true).min_values(1)),
        )
        .subcommand(
            SubCommand::with_name("sync")
                .about("Sync files in dotfile repository to system through symbolic links"),
        )
        .subcommand(SubCommand::with_name("check").about("Check ambit configuration for errors"))
}

// Fetch application matches and run commands accordingly
fn run() -> AmbitResult<()> {
    let matches = get_app().get_matches();

    if let Some(matches) = matches.subcommand_matches("init") {
        let force = matches.is_present("force");
        cmd::init(force)?;
    } else if let Some(matches) = matches.subcommand_matches("clone") {
        let force = matches.is_present("force");
        let git_arguments = matches.values_of("GIT_ARGUMENTS").unwrap().collect();
        cmd::clone(force, git_arguments)?;
    } else if let Some(matches) = matches.subcommand_matches("git") {
        let git_arguments = matches.values_of("GIT_ARGUMENTS").unwrap().collect();
        cmd::git(git_arguments)?;
    } else if matches.is_present("check") {
        cmd::check()?;
    } else if matches.is_present("sync") {
        cmd::sync()?;
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

#[cfg(test)]
mod tests {
    use super::*;

    // Convenient macro to construct matches from arguments and to assert parsing succeeds
    macro_rules! arguments_list {
        [$($i:expr),*] => {{
            let matches = get_app().get_matches_from_safe(vec!["ambit", $($i),*]);
            assert!(matches.is_ok());
            matches.unwrap()
        }}
    }

    #[test]
    fn force_flag() {
        let matches = arguments_list!("init", "-f");
        let has_force = match matches.subcommand_matches("init") {
            Some(matches) => matches.is_present("force"),
            None => false,
        };
        assert!(has_force);
    }

    #[test]
    fn git_arguments_with_hyphen() {
        let matches = arguments_list!("git", "status", "-v", "--short");
        let git_arguments: Option<Vec<_>> = match matches.subcommand_matches("git") {
            Some(matches) => Some(matches.values_of("GIT_ARGUMENTS").unwrap().collect()),
            None => None,
        };
        assert_eq!(git_arguments, Some(vec!["status", "-v", "--short"]));
    }

    #[test]
    fn clone_git_arguments() {
        let matches = arguments_list!(
            "clone",
            "https://github.com/plamorg/ambit",
            // In terms of intended behavior, the following -f flag should be treated as a flag to ambit rather than git
            "-f",
            "--recursive"
        );
        let clone_matches = matches.subcommand_matches("clone").unwrap();
        let git_arguments: Vec<_> = clone_matches.values_of("GIT_ARGUMENTS").unwrap().collect();
        // Although git arguments are allowed, we want to ensure that passing -f passes as an ambit flag
        assert_eq!(
            git_arguments,
            vec!["https://github.com/plamorg/ambit", "--recursive"]
        );
    }
}
