mod cmd;
mod directories;

use clap::{App, AppSettings, Arg, SubCommand};

use std::process;

use ambit::error::{self, AmbitResult};

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
                .about("Sync files in dotfile repository to system through symbolic links")
                .arg(
                    Arg::with_name("dry-run")
                        .long("dry-run")
                        .help("If set, do not actually symlink the files"),
                )
                .arg(
                    Arg::with_name("quiet")
                        .long("quiet")
                        .short("q")
                        .help("Don't report individual symlinks"),
                )
                .arg(
                    Arg::with_name("move")
                        .long("move")
                        .short("m")
                        .help("Move host files into dotfile repository if needed")
                        .long_help("Will automatically move host files into repository if they don't already exist in the repository and then symlink them"),
                )
                .arg(
                    Arg::with_name("use-repo-config")
                    .long("use-repo-config")
                    .help("Recursively search dotfile repository for configuration file and use it to sync")
                )
        )
        .subcommand(
            SubCommand::with_name("clean")
            .about("Remove all symlinks and delete host files")
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
    } else if let Some(matches) = matches.subcommand_matches("sync") {
        let dry_run = matches.is_present("dry-run");
        let quiet = matches.is_present("quiet");
        let move_files = matches.is_present("move");
        let use_repo_config = matches.is_present("use-repo-config");
        cmd::sync(dry_run, quiet, move_files, use_repo_config)?;
    } else if matches.is_present("clean") {
        cmd::clean()?;
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

    // Macro to assert that given arguments list fails
    macro_rules! fail_with_arguments_list {
        [$($i:expr),*] => {{
            assert!(get_app().get_matches_from_safe(vec!["ambit", $($i),*]).is_err(), "Did not error");
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
    fn clone_with_git_argument() {
        let matches = arguments_list!(
            "clone",
            // Any arguments passed after the following -- should be passed as git arguments
            "--",
            "https://github.com/plamorg/ambit",
            "--recursive"
        );
        let clone_matches = matches.subcommand_matches("clone").unwrap();
        let git_arguments: Vec<_> = clone_matches.values_of("GIT_ARGUMENTS").unwrap().collect();
        assert_eq!(
            git_arguments,
            vec!["https://github.com/plamorg/ambit", "--recursive"]
        );
    }

    #[test]
    fn clone_normal() {
        // Since this is a regular call without additional git arguments, -- can be omitted
        let matches = arguments_list!("clone", "https://github.com/plamorg/ambit");
        let clone_matches = matches.subcommand_matches("clone").unwrap();
        let git_arguments: Vec<_> = clone_matches.values_of("GIT_ARGUMENTS").unwrap().collect();
        assert_eq!(git_arguments, vec!["https://github.com/plamorg/ambit"]);
    }

    #[test]
    fn clone_with_invalid_argument() {
        // --invalid is passed to ambit where it is known that it is not a valid ambit flag
        fail_with_arguments_list!("clone", "--invalid", "https://github.com/plamorg/ambit");
    }

    #[test]
    fn clone_force() {
        let matches = arguments_list!(
            "clone",
            "https://github.com/plamorg/ambit",
            // Without --, the following -f flag is assumed to be passed as an ambit argument
            "-f"
        );
        let clone_matches = matches.subcommand_matches("clone").unwrap();
        let has_force = clone_matches.is_present("force");
        let git_arguments: Vec<_> = clone_matches.values_of("GIT_ARGUMENTS").unwrap().collect();
        assert!(has_force);
        assert_eq!(git_arguments, vec!["https://github.com/plamorg/ambit"]);
    }

    #[test]
    fn clone_with_force_as_git_argument() {
        let matches = arguments_list!(
            "clone",
            "--",
            "https://github.com/plamorg/ambit",
            // Because the -f flag comes after --, it should be passed as a git argument
            "-f"
        );
        let clone_matches = matches.subcommand_matches("clone").unwrap();
        let has_force = clone_matches.is_present("force");
        let git_arguments: Vec<_> = clone_matches.values_of("GIT_ARGUMENTS").unwrap().collect();
        assert!(!has_force);
        assert_eq!(
            git_arguments,
            vec!["https://github.com/plamorg/ambit", "-f"]
        );
    }
}
