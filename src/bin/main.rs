use ambit::*;

extern crate clap;
use clap::{App, Arg, SubCommand};

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
