// Symlink function is dependent on OS
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::symlink_file as symlink;
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    process::Command,
};

use walkdir::WalkDir;

use ambit::{
    config::{self, Entry},
    error::{AmbitError, AmbitResult},
};

use crate::directories::{AmbitPath, AmbitPathKind, AMBIT_PATHS, CONFIG_NAME};

// Initialize config and repository directory
fn ensure_paths_exist(force: bool) -> AmbitResult<()> {
    if !AMBIT_PATHS.config.exists() {
        AMBIT_PATHS.config.ensure_parent_dirs_exist()?;
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
fn get_config_entries(config_path: &AmbitPath) -> AmbitResult<Vec<Entry>> {
    let content = config_path.as_string()?;
    config::get_entries(content.chars().peekable())
        .collect::<Result<Vec<_>, _>>()
        .map_err(AmbitError::Parse)
}

// Return if link_name is symlinked to target (link_name -> target).
fn is_symlinked(link_name: &PathBuf, target: &PathBuf) -> bool {
    fs::read_link(link_name)
        .map(|link_path| link_path == *target)
        .unwrap_or(false)
}

// Return iterator over path pairs in the form of `(repo_file, host_file)` from given entry.
fn get_ambit_paths_from_entry<'a>(
    entry: &'a Entry,
) -> Box<dyn Iterator<Item = (AmbitPath, AmbitPath)> + 'a> {
    Box::new(
        entry
            .left
            .into_iter()
            // If entry.right is None, entry.left is considered to be both repo and host path.
            .zip(entry.right.as_ref().unwrap_or(&entry.left).into_iter())
            .map(|(repo_path, host_path)| {
                // Wrap the given paths as AmbitPath.
                (
                    AmbitPath::new(AMBIT_PATHS.repo.path.join(repo_path), AmbitPathKind::File),
                    AmbitPath::new(AMBIT_PATHS.home.path.join(host_path), AmbitPathKind::File),
                )
            }),
    )
}

// Recursively search dotfile repository for config path.
fn get_repo_config_path() -> AmbitResult<Option<PathBuf>> {
    for entry in WalkDir::new(&AMBIT_PATHS.repo.path) {
        if let Ok(entry) = entry {
            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                if file_name == CONFIG_NAME {
                    return Ok(Some(path.to_path_buf()));
                }
            }
        }
    }
    Ok(None)
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
    get_config_entries(&AMBIT_PATHS.config)?;
    Ok(())
}

// Sync files in dotfile repository to system through symbolic links
pub fn sync(
    dry_run: bool,
    quiet: bool,
    move_files: bool,
    use_repo_config: bool,
) -> AmbitResult<()> {
    // Only symlink if repo and git directories exist
    if !(AMBIT_PATHS.repo.exists() && AMBIT_PATHS.git.exists()) {
        return Err(AmbitError::Other(
            "Dotfile repository does not exist. Run `init` or `clone` before syncing.".to_owned(),
        ));
    }
    let mut successful_syncs: usize = 0; // Number of syncs that actually occurred
    let mut total_syncs: usize = 0;
    let mut link = |repo_file: AmbitPath, host_file: AmbitPath| -> AmbitResult<()> {
        // already_symlinked holds whether host_file already links to repo_file
        let already_symlinked = is_symlinked(&host_file.path, &repo_file.path);
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
                if host_file_exists && !repo_file_exists && move_files {
                    // Automatically move the file into the repo
                    repo_file.ensure_parent_dirs_exist()?;
                    fs::rename(&host_file.path, &repo_file.path)?;
                    moved = true;
                } else {
                    host_file.ensure_parent_dirs_exist()?;
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
    let entries = if use_repo_config || !AMBIT_PATHS.config.exists() {
        if !use_repo_config {
            // Ask user if they want to search for repo config.
            println!(
                "No configuration file in {}",
                AMBIT_PATHS.config.path.display()
            );
            print!("Search for configuration in repository? [y/n]: ");
            let mut answer = String::new();
            io::stdin().read_line(&mut answer)?;
            if answer.to_lowercase() != "y" {
                println!("Cancelling sync...");
                return Ok(());
            }
        }
        println!(
            "Searching for {} in {}...",
            CONFIG_NAME,
            AMBIT_PATHS.repo.path.display()
        );
        let repo_config = match get_repo_config_path()? {
            Some(path) => AmbitPath::new(path, AmbitPathKind::File),
            None => {
                return Err(AmbitError::Other(
                    "Could not find configuration file in dotfile repository.".to_owned(),
                ))
            }
        };
        get_config_entries(&repo_config)?
    } else {
        get_config_entries(&AMBIT_PATHS.config)?
    };
    for entry in entries {
        let paths = get_ambit_paths_from_entry(&entry);
        for (repo_file, host_file) in paths {
            link(repo_file, host_file)?;
        }
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

// Remove all symlinks and delete host files.
pub fn clean() -> AmbitResult<()> {
    let entries = get_config_entries(&AMBIT_PATHS.config)?;
    let mut total_syncs: usize = 0;
    let mut deletions: usize = 0;
    for entry in entries {
        let paths = get_ambit_paths_from_entry(&entry);
        for (repo_file, host_file) in paths {
            if is_symlinked(&host_file.path, &repo_file.path) {
                host_file.remove()?;
                deletions += 1;
            }
            total_syncs += 1;
        }
    }
    println!(
        "clean result ({} total): {} deleted: {} ignored",
        total_syncs,
        deletions,
        total_syncs - deletions
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
