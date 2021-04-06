// Symlink function is dependent on OS
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::symlink_file as symlink;
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};

use patmatch::{MatchOptions, Pattern};
use walkdir::WalkDir;

use ambit::{
    config::{self, ast::Spec, Entry},
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
fn is_symlinked(link_name: &Path, target: &Path) -> bool {
    fs::read_link(link_name)
        .map(|link_path| link_path == *target)
        .unwrap_or(false)
}

fn get_paths_from_spec(spec: &Spec, start_path: PathBuf) -> AmbitResult<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let contains_pattern_char = |s: &str| -> bool { s.contains('*') || s.contains('?') };
    for entry in spec.into_iter() {
        if contains_pattern_char(&entry) {
            fn get_matches(
                ancestor_path: &Path,
                expected_path_kind: &AmbitPathKind,
                does_match: impl Fn(&str) -> bool,
            ) -> AmbitResult<Vec<PathBuf>> {
                let mut matches = Vec::new();
                // Iterate through all entries in ancestor_path.
                for path in fs::read_dir(ancestor_path)? {
                    let path = path?.path();
                    // Validify the current path.
                    if let Some(file_name) = path.file_name() {
                        if match expected_path_kind {
                            AmbitPathKind::File => path.is_file(),
                            AmbitPathKind::Directory => path.is_dir(),
                        } && does_match(&file_name.to_string_lossy().to_string())
                        {
                            matches.push(path);
                        }
                    }
                }
                Ok(matches)
            }
            // The only valid path at the start is the starting path.
            let mut valid_paths: Vec<PathBuf> = vec![start_path.clone()];
            let components: Vec<String> = Path::new(&entry)
                .components()
                .map(|comp| comp.as_os_str().to_string_lossy().to_string())
                .collect();
            for (i, component) in components.iter().enumerate() {
                let mut new_valid_paths: Vec<PathBuf> = Vec::new();
                let expected_path_kind = if i < components.len() - 1 {
                    // There are still more components to go, expect a directory.
                    AmbitPathKind::Directory
                } else {
                    // No more components, expect a file.
                    AmbitPathKind::File
                };
                let pattern = Pattern::compile(
                    &component,
                    MatchOptions::WILDCARDS | MatchOptions::UNKNOWN_CHARS,
                );
                for path in &valid_paths {
                    new_valid_paths.append(&mut get_matches(
                        &path,
                        &expected_path_kind,
                        |name: &str| -> bool { pattern.matches(name) },
                    )?);
                }
                valid_paths = new_valid_paths;
            }
            // Strip prefix from all paths.
            valid_paths = valid_paths
                .iter()
                .map(|path| -> AmbitResult<PathBuf> {
                    Ok(path.strip_prefix(&start_path)?.to_path_buf())
                })
                .collect::<AmbitResult<Vec<PathBuf>>>()?;
            paths.append(&mut valid_paths);
        } else {
            paths.push(PathBuf::from(&entry));
        }
    }
    Ok(paths)
}

// Return iterator over path pairs in the form of `(repo_file, host_file)` from given entry.
fn get_ambit_paths_from_entry(entry: &Entry) -> AmbitResult<Vec<(AmbitPath, AmbitPath)>> {
    let (left_paths, right_paths): (Vec<PathBuf>, Option<Vec<PathBuf>>) =
        if let Some(entry_right) = &entry.right {
            (
                get_paths_from_spec(&entry.left, PathBuf::from(AMBIT_PATHS.repo.to_str()?))?,
                Some(get_paths_from_spec(
                    &entry_right,
                    PathBuf::from(AMBIT_PATHS.home.to_str()?),
                )?),
            )
        } else {
            // The right entry does not exist. Treat the left entry as both the repo and host paths.
            (
                get_paths_from_spec(&entry.left, PathBuf::from(AMBIT_PATHS.home.to_str()?))?,
                None,
            )
        };
    // The number of left and right paths may be different due to pattern matching.
    // An error is thrown if they have different sizes.
    if let Some(right_paths) = &right_paths {
        if left_paths.len() != right_paths.len() {
            // Format the vector of PathBuf as a string delimited by a newline.
            let format_paths = |paths: &Vec<PathBuf>| {
                paths
                    .iter()
                    .map(|path| path.as_path().display().to_string())
                    .collect::<Vec<String>>()
                    .join("\n")
            };
            return Err(AmbitError::Other(format!(
                "Entry has imbalanced left and right side due to pattern matching\nAttempted to sync:\n{}\nWITH:\n{}",
                format_paths(&left_paths), format_paths(right_paths),
            )));
        }
    }
    Ok(left_paths
        .iter()
        .zip(right_paths.unwrap_or_else(|| left_paths.clone()).iter())
        .map(|(repo_path, host_path)| {
            (
                AmbitPath::new(AMBIT_PATHS.repo.path.join(repo_path), AmbitPathKind::File),
                AmbitPath::new(AMBIT_PATHS.home.path.join(host_path), AmbitPathKind::File),
            )
        })
        .collect())
}

// Recursively search dotfile repository for config path.
fn get_repo_config_paths(stop_at_first_found: bool) -> Vec<PathBuf> {
    let mut repo_config_paths = Vec::new();
    for dir_entry in WalkDir::new(&AMBIT_PATHS.repo.path) {
        if let Ok(dir_entry) = dir_entry {
            let path = dir_entry.path();
            if let Some(file_name) = path.file_name() {
                if file_name == CONFIG_NAME {
                    repo_config_paths.push(path.to_path_buf());
                    if stop_at_first_found {
                        break;
                    }
                }
            }
        }
    }
    repo_config_paths
}

// Prompt user for confirmation with message.
fn prompt_confirm(message: &str) -> AmbitResult<bool> {
    print!("{} [Y/n] ", message);
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(answer.trim().to_lowercase() == "y")
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
    use_repo_config_if_required: bool,
    use_any_repo_config: bool,
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
                "No configuration file found in {}",
                AMBIT_PATHS.config.path.display()
            );
            // No need to prompt if `use_repo_config_if_required` is true.
            if !use_repo_config_if_required
                && !prompt_confirm("Search for configuration in repository?")?
            {
                println!("Ignoring sync...");
                return Ok(());
            }
        }
        println!(
            "Searching for {} in {}...",
            CONFIG_NAME,
            AMBIT_PATHS.repo.path.display()
        );
        let repo_config_paths = get_repo_config_paths(use_any_repo_config);
        let mut repo_config = None;
        // Iterate through repo configuration files that were found.
        for path in repo_config_paths {
            if use_any_repo_config
                || prompt_confirm(format!("Repo config found: {}. Use?", path.display()).as_str())?
            {
                // config.ambit file has been found in repo and user has accepted it.
                repo_config = Some(AmbitPath::new(path, AmbitPathKind::File));
                break;
            }
        }
        match repo_config {
            Some(repo_config) => get_config_entries(&repo_config)?,
            None => {
                return Err(AmbitError::Other(
                    "Could not find configuration file in dotfile repository.".to_owned(),
                ));
            }
        }
    } else {
        get_config_entries(&AMBIT_PATHS.config)?
    };
    for entry in entries {
        let paths = get_ambit_paths_from_entry(&entry)?;
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
        let paths = get_ambit_paths_from_entry(&entry)?;
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

#[cfg(test)]
mod tests {
    use super::get_paths_from_spec;
    use ambit::config::ast::Spec;
    use std::{
        collections::HashSet,
        fs::{self, File},
        path::PathBuf,
    };

    fn test_spec(spec_str: &str, existing_paths: &[&str], expected_paths: &[PathBuf]) {
        let spec = Spec::from(spec_str);
        let dir_path = tempfile::tempdir().unwrap().into_path();
        // Create paths.
        for path in existing_paths {
            let path = dir_path.join(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            File::create(path).unwrap();
        }
        let paths = get_paths_from_spec(&spec, dir_path).unwrap();
        let paths: HashSet<&PathBuf> = paths.iter().collect();
        // Use a HashSet as order of paths should not matter.
        assert_eq!(paths, expected_paths.iter().collect::<HashSet<&PathBuf>>());
    }

    #[test]
    fn get_paths_from_spec_without_pattern() {
        test_spec(
            "a/b/c",
            &["c/b/a", "a/b/c"],
            &[PathBuf::from("a").join("b").join("c")],
        );
    }

    #[test]
    fn get_paths_from_spec_adjacent_wildcard() {
        test_spec(
            ".config/*/*",
            &[
                ".config/foo",
                ".config/bar",
                ".config/hello",
                ".config/nvim/init.vim",
                ".config/ambit/config.ambit",
                ".config/ambit/repo/.vimrc",
            ],
            &[
                PathBuf::from(".config").join("nvim").join("init.vim"),
                PathBuf::from(".config").join("ambit").join("config.ambit"),
            ],
        );
    }

    #[test]
    fn get_paths_from_spec_with_unknown_char() {
        test_spec(
            "Pictures/*.???",
            &[
                "Pictures/foo.jpg",
                "Pictures/bar.png",
                "Pictures/hello.svg",
                // The following 2 should be ignored.
                "Pictures/world.webp",
                "Pictures/image.jpeg",
            ],
            &[
                PathBuf::from("Pictures").join("foo.jpg"),
                PathBuf::from("Pictures").join("bar.png"),
                PathBuf::from("Pictures").join("hello.svg"),
            ],
        );
    }

    #[test]
    fn get_paths_from_spec_with_escaped_char() {
        test_spec("x\\*y", &["x*y", "xay", "xaay"], &[PathBuf::from("x*y")]);
    }
}
