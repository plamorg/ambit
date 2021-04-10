// Linker handles sync and move operations.
use crate::{
    cmd,
    config::{self, ast::Spec, Entry},
    directories::{self, AmbitPath, AmbitPathKind, AmbitPaths},
    error::{AmbitError, AmbitResult},
};
use patmatch::{MatchOptions, Pattern};
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::symlink_file as symlink;
use std::{
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Debug)]
pub struct Options {
    pub force: bool,
    pub dry_run: bool,
    pub quiet: bool,
}

// Return if link_name is symlinked to target (link_name -> target).
fn is_symlinked(link_name: &Path, target: &Path) -> bool {
    fs::read_link(link_name)
        .map(|link_path| link_path == *target)
        .unwrap_or(false)
}

// Return a vector of PathBufs that match a pattern relative to the given start_path.
fn get_paths_from_spec(
    spec: &Spec,
    start_path: PathBuf,
    allow_pattern: bool,
) -> AmbitResult<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in spec.into_iter() {
        if !entry.contains('*') && !entry.contains('?') {
            // The entry does not contain any pattern matching characters.
            // This is a definitive path so we can simply push it.
            paths.push(PathBuf::from(&entry));
        } else {
            if !allow_pattern {
                return Err(AmbitError::Other(
                    "Found unexpected pattern character.".to_owned(),
                ));
            }
            // The only valid path at the start is the starting path.
            // This will be replaced at every iteration/depth.
            let mut valid_paths: Vec<PathBuf> = vec![start_path.clone()];
            let components: Vec<_> = Path::new(&entry)
                .components()
                .map(|comp| comp.as_os_str().to_string_lossy())
                .collect();
            // To find matching files and directories, an entry as part of the spec is split into components.
            // For each component, a pattern is compiled and a vector of paths that match this pattern is found.
            // With the vector produced from the previous component, the process is repeated with the ancestor paths equal to the said vector.
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
                for ancestor_path in &valid_paths {
                    for path in fs::read_dir(ancestor_path)? {
                        let path = path?.path();
                        // Validify the current path.
                        if let Some(file_name) = path.file_name() {
                            if match expected_path_kind {
                                AmbitPathKind::File => path.is_file(),
                                AmbitPathKind::Directory => path.is_dir(),
                            } && pattern.matches(&file_name.to_string_lossy())
                            {
                                new_valid_paths.push(path);
                            }
                        }
                    }
                }
                valid_paths = new_valid_paths;
            }
            // Strip prefix from all paths.
            for path in valid_paths {
                paths.push(path.strip_prefix(&start_path)?.to_path_buf());
            }
        }
    }
    Ok(paths)
}

#[derive(Debug)]
pub struct Linker {
    paths: AmbitPaths,
    options: Options,
}

impl Linker {
    pub fn new(paths: &AmbitPaths, options: Options) -> AmbitResult<Self> {
        // Only symlink if repo and git directories exist
        if !paths.repo.exists() || !paths.git.exists() {
            Err(AmbitError::Other(
                "Dotfile repository does not exist. Run `init` or `clone`.".to_owned(),
            ))
        } else {
            Ok(Self {
                paths: paths.clone(),
                options,
            })
        }
    }

    pub fn clean_paths(&self) -> AmbitResult<()> {
        let config_path = self.find_config_path(self.options.force)?;
        let entries = config::get_entries(&config_path)?;
        let mut total_syncs: usize = 0;
        let mut deletions: usize = 0;
        for entry in entries {
            let paths = self.get_ambit_paths_from_entry(&entry)?;
            for (repo_file, host_file) in paths {
                if is_symlinked(&host_file.path, &repo_file.path) {
                    if !self.options.dry_run {
                        host_file.remove()?;
                    }
                    deletions += 1;
                    if !self.options.quiet {
                        let action = if self.options.dry_run {
                            "Ignored"
                        } else {
                            "Removed"
                        };
                        println!("{} {}", action, host_file.path.display());
                    }
                }
                total_syncs += 1;
            }
        }
        // Final clean metrics.
        println!(
            "clean result ({} total): {} deleted: {} ignored",
            total_syncs,
            deletions,
            total_syncs - deletions
        );
        Ok(())
    }

    pub fn sync_paths(&self) -> AmbitResult<()> {
        let mut total: usize = 0;
        let config_path = self.find_config_path(self.options.force)?;
        let mut symlink_pairs = Vec::new();
        for entry in config::get_entries(&config_path)? {
            for (repo_file, host_file) in self.get_ambit_paths_from_entry(&entry)? {
                if !repo_file.exists() {
                    return Err(AmbitError::Other(format!(
                        "Repository file {} must exist to be synced. Consider using `move`.",
                        repo_file.path.display()
                    )));
                }
                // Only push into symlink_pairs if it hasn't been symlinkd already.
                if !is_symlinked(&host_file.path, &repo_file.path) {
                    if host_file.exists() {
                        return Err(AmbitError::Other(format!(
                            "Host file {} already exists and is not correctly symlinked.",
                            host_file.path.display()
                        )));
                    }
                    symlink_pairs.push((repo_file, host_file));
                }
                total += 1;
            }
        }
        for (repo_file, host_file) in &symlink_pairs {
            if !self.options.dry_run {
                host_file.ensure_parent_dirs_exist()?;
                // Attempt to symlink.
                if let Err(e) = symlink(&repo_file.path, &host_file.path) {
                    // Symlink went wrong
                    return Err(AmbitError::Sync {
                        host_file_path: PathBuf::from(&host_file.path),
                        repo_file_path: PathBuf::from(&repo_file.path),
                        error: Box::new(AmbitError::Io(e)),
                    });
                }
            }
            if !self.options.quiet {
                let action = if self.options.dry_run {
                    "Ignored"
                } else {
                    "Synced"
                };
                println!(
                    "{} {} -> {}",
                    action,
                    host_file.path.display(),
                    repo_file.path.display()
                );
            }
        }
        let total_synced: usize = if self.options.dry_run {
            0
        } else {
            symlink_pairs.len()
        };
        // Final sync metrics.
        println!(
            "sync result ({} total): {} synced: {} ignored",
            total,
            total_synced,
            total - total_synced,
        );
        Ok(())
    }

    pub fn move_paths(&self) -> AmbitResult<()> {
        let mut total: usize = 0;
        let mut total_moved: usize = 0;
        let config_path = self.find_config_path(self.options.force)?;
        for entry in config::get_entries(&config_path)? {
            for (repo_file, host_file) in self.get_ambit_paths_from_entry(&entry)? {
                total += 1;
                if !repo_file.exists() && host_file.exists() {
                    if !self.options.dry_run {
                        total_moved += 1;
                        fs::rename(host_file.as_path(), repo_file.as_path())?;
                    }
                    if !self.options.quiet {
                        let action = if self.options.dry_run {
                            "Ignored moving"
                        } else {
                            "Moved"
                        };
                        println!(
                            "{} {} to {}",
                            action,
                            host_file.display(),
                            repo_file.display()
                        );
                    }
                }
            }
        }
        // Final moved metrics.
        println!(
            "move result ({} total): {} moved: {} ignored",
            total,
            total_moved,
            total - total_moved,
        );
        Ok(())
    }

    fn find_config_path(&self, force: bool) -> AmbitResult<AmbitPath> {
        let mut new_config_path = None;
        if !self.paths.config.exists() {
            if force || cmd::prompt_confirm("Search for configuration in repository?")? {
                println!(
                    "Searching for {} in {}...",
                    directories::CONFIG_NAME,
                    self.paths.repo.display()
                );
                // Pass force because only the first repo config path is needed.
                for path in self.get_repo_config_paths(force) {
                    if force || cmd::prompt_confirm(format!("Use {}?", path.display()).as_str())? {
                        new_config_path = Some(AmbitPath::new(path, AmbitPathKind::File));
                        break;
                    }
                }
            }
        } else {
            new_config_path = Some(AmbitPath::from(&self.paths.config.path));
        }
        new_config_path
            .ok_or_else(|| AmbitError::Other("Could not locate configuration file.".to_owned()))
    }

    // Recursively search dotfile repository for config path.
    fn get_repo_config_paths(&self, stop_at_first_found: bool) -> Vec<PathBuf> {
        let mut repo_config_paths = Vec::new();
        for dir_entry in WalkDir::new(self.paths.repo.as_path()) {
            if let Ok(dir_entry) = dir_entry {
                let path = dir_entry.path();
                if let Some(file_name) = path.file_name() {
                    if file_name == directories::CONFIG_NAME {
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

    // Return vector over path pairs in the form of `(repo_file, host_file)` from given entry.
    fn get_ambit_paths_from_entry(
        &self,
        entry: &Entry,
    ) -> AmbitResult<Vec<(AmbitPath, AmbitPath)>> {
        // Only search left paths from repo.
        let left_paths =
            get_paths_from_spec(&entry.left, PathBuf::from(self.paths.repo.to_str()?), true)?;
        let right_paths = if let Some(entry_right) = &entry.right {
            Some(get_paths_from_spec(
                &entry_right,
                PathBuf::from(self.paths.home.to_str()?),
                false,
            )?)
        } else {
            // The right entry does not exist. Treat the left entry as both the repo and host paths.
            None
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
                "Entry has imbalanced left and right side due to pattern matching\nAttempted to sync:\n{}\nwith:\n{}",
                format_paths(&left_paths), format_paths(right_paths),
            )));
            }
        }
        let mut paths = Vec::new();
        for (i, repo_path) in left_paths.iter().enumerate() {
            let host_path = if let Some(ref right_paths) = right_paths {
                &right_paths[i]
            } else {
                repo_path
            };
            paths.push((
                AmbitPath::new(self.paths.repo.join(repo_path), AmbitPathKind::File),
                AmbitPath::new(self.paths.home.path.join(host_path), AmbitPathKind::File),
            ))
        }
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::get_paths_from_spec;
    use crate::config::ast::Spec;
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
        let paths = get_paths_from_spec(&spec, dir_path, true).unwrap();
        // Assert that there are no duplicates as they would be removed when collected into a HashSet.
        assert_eq!(paths.len(), expected_paths.len());
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
    fn get_paths_from_spec_ignore_parent() {
        // This will resolve to a/b/c because if the user explicitly specifies a file (without pattern matching characters)
        // its existence has to be verified at the symlinking stage which would error if it doesn't exist.
        // This is to inform the user that the file does not exist.
        // This differs from a pattern matching spec that will not resolve if the file does not exist.
        test_spec("a/b/c", &["a/b"], &[PathBuf::from("a").join("b").join("c")]);
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

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn get_paths_from_spec_with_escaped_char() {
        test_spec("x\\*y", &["x*y", "xay", "xaay"], &[PathBuf::from("x*y")]);
    }

    // TODO: Add more tests
}
