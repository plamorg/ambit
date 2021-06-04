// Symlink function is dependent on OS
use crate::{
    config,
    directories::AMBIT_PATHS,
    error::{AmbitError, AmbitResult},
};
use std::{
    io::{self, Write},
    process::Command,
};

// Initialize config and repository directory
fn ensure_no_repo_conflicts(force: bool) -> AmbitResult<()> {
    let repo_exists = AMBIT_PATHS.repo.exists();
    if repo_exists
        // No need to prompt if force is true.
        && !force
        // Ask user if they want to overwrite.
        && !prompt_confirm("A repository already exists. Overwrite?")?
    {
        return Err(AmbitError::Other(
            "Dotfile repository already exists.\nUse '-f' flag to overwrite.".to_owned(),
        ));
    } else if repo_exists {
        // Remove if either force is enabled or if the user confirmed to overwrite.
        AMBIT_PATHS.repo.remove()?;
    }
    Ok(())
}

// Prompt user for confirmation with message.
pub fn prompt_confirm(message: &str) -> AmbitResult<bool> {
    print!("{} [Y/n] ", message);
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(answer.trim().to_lowercase() == "y")
}

// Initialize an empty dotfile repository
pub fn init(force: bool) -> AmbitResult<()> {
    ensure_no_repo_conflicts(force)?;
    AMBIT_PATHS.repo.create()?;
    // Initialize an empty git repository
    git(vec!["init"])?;
    Ok(())
}

// Clone an existing dotfile repository with given origin
pub fn clone(force: bool, clone_arguments: Vec<&str>) -> AmbitResult<()> {
    ensure_no_repo_conflicts(force)?;
    // Clone will handle creating the repository directory
    let repo_path = AMBIT_PATHS.repo.to_str()?;
    let status = Command::new("git")
        .arg("clone")
        .args(clone_arguments)
        .args(vec!["--", repo_path])
        .status()?;
    match status.success() {
        true => {
            println!("Successfully cloned repository to {}", repo_path);
            Ok(())
        }
        false => Err(AmbitError::Other("Failed to clone repository".to_owned())),
    }
}

// Check ambit configuration for errors
pub fn check() -> AmbitResult<()> {
    config::get_entries(&AMBIT_PATHS.config)?;
    Ok(())
}

// Run git commands from the dotfile repository
pub fn git(git_arguments: Vec<&str>) -> AmbitResult<()> {
    // The path to repository (git-dir) and the working tree (work-tree) is
    // passed to ensure that git commands are run from the dotfile repository
    let mut command = Command::new("git");
    command.args(&[
        ["--git-dir=", AMBIT_PATHS.git.to_str()?].concat(),
        ["--work-tree=", AMBIT_PATHS.repo.to_str()?].concat(),
    ]);
    command.args(git_arguments);
    // Conditional compilation so that this still compiles on Windows.
    #[cfg(unix)]
    fn exec_git_command(mut command: Command) -> AmbitResult<()> {
        use std::os::unix::process::CommandExt;
        // Try to replace this process with the `git` process.
        // This is to allow stuff like terminal colors.
        // We just want `ambit git` to act like `cd ~/.config/ambit/repo; git`.
        // If the `.exec()` method returns, it failed to execute, so it's automatically an error.
        Err(AmbitError::Io(command.exec()))
    }
    #[cfg(not(unix))]
    fn exec_git_command(mut command: Command) -> AmbitResult<()> {
        // Not easy to do this on other systems, just use defaults
        let output = command.output()?;
        io::stdout().write_all(&output.stdout)?;
        io::stdout().write_all(&output.stderr)?;
        Ok(())
    }
    exec_git_command(command)
}
