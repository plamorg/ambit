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
    get_config_entries()?;
    Ok(())
}

// Sync files in dotfile repository to system through symbolic links
pub fn sync(dry_run: bool, quiet: bool, move_files: bool) -> AmbitResult<()> {
    // Only symlink if repo and git directories exist
    if !(AMBIT_PATHS.repo.exists() && AMBIT_PATHS.git.exists()) {
        return Err(AmbitError::Other(
            "Dotfile repository does not exist. Run `init` or `clone` before syncing.".to_owned(),
        ));
    }
    let mut successful_syncs: usize = 0; // Number of syncs that actually occurred
    let mut total_syncs: usize = 0;
    let mut link = |repo_filename: &str, host_filename: &str| -> AmbitResult<()> {
        let host_file = AmbitPath::new(
            AMBIT_PATHS.home.path.join(host_filename),
            AmbitPathKind::File,
        );
        let repo_file = AmbitPath::new(
            AMBIT_PATHS.repo.path.join(repo_filename),
            AmbitPathKind::File,
        );

        // already_symlinked holds whether host_file already links to repo_file
        let already_symlinked = fs::read_link(&host_file.path)
            .map(|link_path| link_path == repo_file.path)
            .unwrap_or(false);

        // cache for later
        let host_file_exists = host_file.exists();
        let repo_file_exists = repo_file.exists();

        if host_file_exists && !already_symlinked && !move_files {
            // Host file already exists but is not symlinked correctly
            return Err(AmbitError::Sync {
                host_file_path: host_file.path,
                repo_file_path: repo_file.path,
                error: Box::new(AmbitError::Other(
                    "Host file already exists and is not correctly symlinked".to_owned(),
                )),
            });
        }
        if !repo_file_exists && !move_files {
            return Err(AmbitError::Sync {
                host_file_path: host_file.path,
                repo_file_path: repo_file.path,
                error: Box::new(AmbitError::Other(
                    "Repository file does not exist".to_owned(),
                )),
            });
        }
        if !already_symlinked {
            let mut moved = false;
            if !dry_run {
                fn ensure_parent_dirs_exist(file: &AmbitPath) -> AmbitResult<()> {
                    if let Some(parent) = file.path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    Ok(())
                }
                if host_file_exists && !repo_file_exists && move_files {
                    // Automatically move the file into the repo
                    ensure_parent_dirs_exist(&repo_file)?;
                    fs::rename(&host_file.path, &repo_file.path)?;
                    moved = true;
                } else {
                    ensure_parent_dirs_exist(&host_file)?;
                }
                // Attempt to perform symlink
                if let Err(e) = symlink(&repo_file.path, &host_file.path) {
                    // Symlink went wrong
                    return Err(AmbitError::Sync {
                        host_file_path: host_file.path,
                        repo_file_path: repo_file.path,
                        error: Box::new(AmbitError::Io(e)),
                    });
                }
                successful_syncs += 1;
            }
            if !quiet {
                let action = match moved {
                    true => "Moved",
                    false => match !dry_run {
                        true => "Synced",
                        false => "Ignored",
                    },
                };
                println!(
                    "{} {} -> {}",
                    action,
                    host_file.path.display(),
                    repo_file.path.display()
                );
            }
        }
        total_syncs += 1;
        Ok(())
    };
    let entries = get_config_entries()?;
    for entry in entries {
        let left_paths = entry.left.into_iter();
        match entry.right {
            Some(right_spec) => {
                let right_paths = right_spec.into_iter();
                for (repo_file, host_file) in left_paths.zip(right_paths) {
                    // Attempt to link REPO/left => HOME/right.
                    link(&repo_file, &host_file)?;
                }
            }
            None => {
                // If there is no right spec, the path from home directory is equal to left spec.
                // Hence, we effectively link REPO/left => HOME/left.
                for file in left_paths {
                    link(&file, &file)?;
                }
            }
        };
    }
    // Report the number of files symlinked
    println!(
        "sync result ({} total): {} synced; {} ignored",
        total_syncs,
        successful_syncs,
        total_syncs - successful_syncs,
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
