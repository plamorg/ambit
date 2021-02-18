// Symlink function is dependent on OS
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::symlink_file as symlink;
use std::{
    fs,
    io::{self, Write},
    process::Command,
};

use ambit::{
    config,
    error::{AmbitError, AmbitResult},
};

use crate::directories::{AmbitPath, AmbitPathKind, AMBIT_PATHS};

// Initialize config and repository directory
fn ensure_paths_exist(force: bool) -> AmbitResult<()> {
    if !AMBIT_PATHS.config.exists() {
        AMBIT_PATHS.config.create()?;
    }
    if AMBIT_PATHS.repo.exists() && !force {
        // Dotfile repository should not be overwritten if force is false
        return Err(AmbitError::Other(
            "Dotfile repository already exists.\nUse '-f' flag to overwrite.".to_owned(),
        ));
    } else if AMBIT_PATHS.repo.exists() {
        // Repository directory exists but force is enabled
        AMBIT_PATHS.repo.remove()?;
    }
    Ok(())
}

// Fetch entries from config file and return as vector
fn get_config_entries() -> AmbitResult<Vec<config::Entry>> {
    let content = AMBIT_PATHS.config.as_string()?;
    config::get_entries(content.chars().peekable())
        .collect::<Result<Vec<_>, _>>()
        .map_err(AmbitError::Parse)
}

// Initialize an empty dotfile repository
pub fn init(force: bool) -> AmbitResult<()> {
    ensure_paths_exist(force)?;
    AMBIT_PATHS.repo.create()?;
    // Initialize an empty git repository
    git(vec!["init"])?;
    Ok(())
}

// Clone an existing dotfile repository with given origin
pub fn clone(force: bool, arguments: Vec<&str>) -> AmbitResult<()> {
    ensure_paths_exist(force)?;
    // Clone will handle creating the repository directory
    let repo_path = AMBIT_PATHS.repo.to_str()?;
    let status = Command::new("git")
        .arg("clone")
        .args(arguments)
        // Pass in ambit repo path as last argument to ensure that it is always cloned to the known path
        .arg(repo_path)
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
    get_config_entries()?;
    Ok(())
}

// Sync files in dotfile repository to system through symbolic links
pub fn sync(dry_run: bool, quiet: bool) -> AmbitResult<()> {
    // Only symlink if repo and git directories exist
    if !(AMBIT_PATHS.repo.exists() && AMBIT_PATHS.git.exists()) {
        return Err(AmbitError::Other(
            "Dotfile repository does not exist. Run `init` or `clone` before syncing.".to_owned(),
        ));
    }
    let mut successful_symlinks = 0; // Number of symlinks that actually occurred
    let mut total_symlinks = 0;
    // link is a closure as it must have access to outside variables
    let mut link = |repo_filename: &str, host_filename: &str| -> AmbitResult<()> {
        let host_file = AmbitPath::new(
            AMBIT_PATHS.home.path.join(host_filename),
            AmbitPathKind::File,
        );
        let host_file_str = host_file.to_str()?;
        let repo_file = AmbitPath::new(
            AMBIT_PATHS.repo.path.join(repo_filename),
            AmbitPathKind::File,
        );
        let repo_file_str = repo_file.to_str()?;

        // already_symlinked holds whether host_file already links to repo_file
        let already_symlinked = if let Ok(link_path) = fs::read_link(&host_file.path) {
            link_path == repo_file.path
        } else {
            false
        };

        if host_file.exists() && !already_symlinked {
            // Host file already exists but is not symlinked correctly
            return Err(AmbitError::Symlink {
                host_file: host_file_str.to_owned(),
                repo_file: repo_file_str.to_owned(),
                error: Box::new(AmbitError::Other(
                    "Host file already exists and is not correctly symlinked".to_owned(),
                )),
            });
        } else if !repo_file.exists() {
            return Err(AmbitError::Symlink {
                host_file: host_file_str.to_owned(),
                repo_file: repo_file_str.to_owned(),
                error: Box::new(AmbitError::Other(
                    "Repository file does not exist".to_owned(),
                )),
            });
        } else if !already_symlinked {
            if !dry_run {
                // Attempt to perform symlink
                if let Err(e) = symlink(&repo_file.path, &host_file.path) {
                    // Symlink went wrong
                    return Err(AmbitError::Symlink {
                        host_file: host_file_str.to_owned(),
                        repo_file: repo_file_str.to_owned(),
                        error: Box::new(AmbitError::Io(e)),
                    });
                }
                successful_symlinks += 1;
            }
            if !quiet {
                println!("{} -> {}", host_file_str, repo_file.to_str()?);
            }
        }
        total_symlinks += 1;
        Ok(())
    };
    let entries = get_config_entries()?;
    for entry in entries {
        let left_specs = entry.left.into_iter();
        match entry.right {
            Some(right_specs) => {
                let right_specs = right_specs.into_iter();
                for path_pair in left_specs.zip(right_specs) {
                    let (repo_file, host_file) = path_pair;
                    // Attempt to link REPO/left => HOME/right.
                    link(&repo_file, &host_file)?;
                }
            }
            None => {
                // If there is no right spec, the path from home directory is equal to left spec.
                // Hence, we effectively link REPO/left => HOME/left.
                for file in left_specs {
                    link(&file, &file)?;
                }
            }
        };
    }
    // Report the number of files symlinked
    println!(
        "sync result ({} total): {} synced; {} ignored",
        total_symlinks,
        successful_symlinks,
        total_symlinks - successful_symlinks,
    );
    Ok(())
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
