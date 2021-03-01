use assert_cmd::{assert::Assert, Command};
use std::{
    ffi::OsStr,
    fs::{self, File},
    path::{Path, PathBuf},
};
use tempfile::TempDir;

#[derive(Debug)]
pub struct AmbitTester {
    config_path: PathBuf,
    repo_path: PathBuf,
    host_path: PathBuf,
    executable: Command,
}
// Builder pattern implementation
impl AmbitTester {
    // Allow temp_dir to be passed so it can be owned from outside of the struct.
    fn from_temp_dir(temp_dir: &TempDir) -> Self {
        let temp_dir_path = temp_dir.path();
        let config_path = temp_dir_path.join("config.ambit");
        let repo_path = temp_dir_path.join("repo");
        let host_path = temp_dir_path.to_path_buf();
        let mut executable = Command::cargo_bin("ambit").unwrap();
        // Set environment variables.
        // AMBIT_HOME_PATH is set as temp_dir. This is important as it will be the prefix path of potential synced files.
        executable.env("AMBIT_HOME_PATH", host_path.as_os_str());
        executable.env("AMBIT_CONFIG_PATH", config_path.as_os_str());
        executable.env("AMBIT_REPO_PATH", repo_path.as_os_str());
        Self {
            config_path,
            repo_path,
            host_path,
            executable,
        }
    }

    // Write content to configuration file.
    fn with_config(self, content: &str) -> Self {
        File::create(&self.config_path).unwrap();
        fs::write(&self.config_path, content).unwrap();
        self
    }

    // Write content to a given path.
    fn with_file_with_content(self, path: &PathBuf, content: &str) -> Self {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        File::create(&path).unwrap();
        fs::write(&path, content).unwrap();
        self
    }

    // Create a custom file in repo_path directory. Mimics repo_file.
    fn with_repo_file(self, name: &str) -> Self {
        let s = self.with_repo_path();
        File::create(s.repo_path.join(name)).unwrap();
        s
    }

    // Creates a custom file in home_path directory. Mimic host_file.
    fn with_host_file(self, name: &str) -> Self {
        File::create(self.host_path.join(name)).unwrap();
        self
    }

    fn with_repo_path(self) -> Self {
        fs::create_dir_all(&self.repo_path.join(".git")).unwrap();
        self
    }

    fn arg<S: AsRef<OsStr>>(mut self, arg: S) -> Self {
        self.executable.arg(arg);
        self
    }

    fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.executable.args(args);
        self
    }

    fn write_stdin<S>(mut self, buffer: S) -> Self
    where
        S: Into<Vec<u8>>,
    {
        self.executable.write_stdin(buffer);
        self
    }

    fn assert(mut self) -> Assert {
        // Consumes self
        self.executable.assert()
    }
}
impl Default for AmbitTester {
    // Default should be used when direct access to temporary directory is not needed.
    fn default() -> Self {
        AmbitTester::from_temp_dir(&TempDir::new().unwrap())
    }
}

// Returns if a is symlinked to b (a -> b).
fn is_symlinked(a: PathBuf, b: PathBuf) -> bool {
    fs::read_link(a)
        .map(|link_path| link_path == b)
        .unwrap_or(false)
}

#[test]
fn init_repo_already_exists() {
    // Expect an error when attempting to initialize without force flag.
    // The repository directory is already created when calling `with_repo_path()`.
    AmbitTester::default()
        .with_repo_path()
        .arg("init")
        .assert()
        .stderr("ERROR: Dotfile repository already exists.\nUse '-f' flag to overwrite.\n");
}

#[test]
fn init_force_overwrites() {
    AmbitTester::default()
        .with_repo_path()
        .args(vec!["init", "-f"])
        .assert()
        .success();
}

#[test]
fn init_ambit_config_dir_does_not_exist() {
    // Ensure that temp_dir configuration directory is created when init is called.
    let temp_dir = TempDir::new().unwrap();
    // temp_path is equal to temp_dir path which is then removed.
    let temp_path = Path::new(temp_dir.path());
    fs::remove_dir(temp_path).unwrap();
    AmbitTester::from_temp_dir(&temp_dir)
        .arg("init")
        .assert()
        .success();
}

#[test]
fn clone_repo_already_exists() {
    // Expect an error when attempting to clone without force flag.
    AmbitTester::default()
        .with_repo_path()
        .args(vec!["clone", "https://github.com/plamorg/ambit"])
        .assert()
        .stderr("ERROR: Dotfile repository already exists.\nUse '-f' flag to overwrite.\n");
}

#[test]
fn sync_without_repo() {
    // Error should occur if attempting to sync without initializing.
    // `with_repo_path` is omitted here.
    AmbitTester::default().arg("sync").assert().stderr(
        "ERROR: Dotfile repository does not exist. Run `init` or `clone` before syncing.\n",
    );
}

#[test]
fn sync_host_file_already_exists() {
    // The host file already exists but is not symlinked to repo file.
    let temp_dir = TempDir::new().unwrap();
    AmbitTester::from_temp_dir(&temp_dir)
        .with_repo_file("repo.txt")
        .with_host_file("host.txt")
        .with_config("repo.txt => host.txt;")
        .arg("sync")
        .assert()
        .failure();
}

#[test]
fn sync_repo_file_does_not_exist() {
    // Repo file should exist for sync to work.
    AmbitTester::default()
        .with_repo_path()
        .with_config("repo.txt => host.txt;")
        .arg("sync")
        .assert()
        .failure();
}

#[test]
fn sync_normal() {
    let temp_dir = TempDir::new().unwrap();
    AmbitTester::from_temp_dir(&temp_dir)
        .with_repo_file("repo.txt")
        .with_config("repo.txt => host.txt;")
        .arg("sync")
        .assert()
        .success();
    // Assert that host.txt is symlinked to repo.txt
    assert!(is_symlinked(
        temp_dir.path().join("host.txt"),
        temp_dir.path().join("repo").join("repo.txt")
    ));
}

#[test]
fn sync_move_normal() {
    let temp_dir = TempDir::new().unwrap();
    AmbitTester::from_temp_dir(&temp_dir)
        .with_repo_path()
        .with_config("repo.txt => host.txt;")
        .with_host_file("host.txt")
        .args(vec!["sync", "-m"])
        .assert()
        .success();
    // The new `repo.txt` should now exist
    assert!(temp_dir.path().join("repo").join("repo.txt").exists());
    // The symlink should still succeed
    assert!(is_symlinked(
        temp_dir.path().join("host.txt"),
        temp_dir.path().join("repo").join("repo.txt")
    ));
}

#[test]
fn sync_dry_run_should_not_symlink() {
    let temp_dir = TempDir::new().unwrap();
    AmbitTester::from_temp_dir(&temp_dir)
        .with_repo_file("repo.txt")
        .with_config("repo.txt => should-not-exist.txt;")
        .args(vec!["sync", "--dry-run"])
        .assert()
        .success();
    // Since this is a dry-run, the host_file should not exist.
    assert!(!temp_dir.path().join("should-not-exist.txt").exists());
}

#[test]
fn sync_creates_host_parent_directories() {
    // Parent directories of the host file should be created if they do not exist.
    // We want to ensure that the following test does not fail due to "No such file or directory" error.
    let temp_dir = TempDir::new().unwrap();
    AmbitTester::from_temp_dir(&temp_dir)
        .with_repo_file("repo.txt")
        .with_config("repo.txt => a/b/host.txt;")
        .arg("sync")
        .assert()
        .success();
    // Assert that a/b/host.txt is symlinked to repo.txt
    assert!(is_symlinked(
        temp_dir.path().join("a").join("b").join("host.txt"),
        temp_dir.path().join("repo").join("repo.txt"),
    ));
}

#[test]
fn sync_use_repo_config_option() {
    // If --use-repo-config flag is passed, recursively search for configuration in repository and use that.
    let temp_dir = TempDir::new().unwrap();
    // Set repo_config_path to /a/b/config.ambit to ensure recursive search works.
    let repo_config_path = temp_dir
        .path()
        .join("repo")
        .join("a")
        .join("b")
        .join("config.ambit");
    AmbitTester::from_temp_dir(&temp_dir)
        .with_repo_file("repo.txt")
        .with_file_with_content(&repo_config_path, "repo.txt => host.txt;")
        .args(vec!["sync", "--use-repo-config"])
        .write_stdin("Y") // 'Y' should be synonymous to 'y'
        .assert()
        .success();
    assert!(is_symlinked(
        temp_dir.path().join("host.txt"),
        temp_dir.path().join("repo").join("repo.txt"),
    ));
}

#[test]
fn sync_with_missing_config_answer_yes() {
    // Sync without existing configuration file and answer yes to use repo configuration instead.
    let temp_dir = TempDir::new().unwrap();
    let repo_config_path = temp_dir.path().join("repo").join("config.ambit");
    AmbitTester::from_temp_dir(&temp_dir)
        .with_repo_file("repo.txt")
        .with_file_with_content(&repo_config_path, "repo.txt => host.txt;")
        .arg("sync")
        // Answer 'y' twice:
        // 1. Accept to search for configuration.
        // 2. Accept to use repo config that was found.
        .write_stdin("y\ny")
        .assert()
        .success();
    assert!(is_symlinked(
        temp_dir.path().join("host.txt"),
        temp_dir.path().join("repo").join("repo.txt"),
    ));
}

#[test]
fn sync_with_missing_config_answer_no() {
    // Sync without existing configuration file but answer no to using repo configuration.
    let temp_dir = TempDir::new().unwrap();
    let repo_config_path = temp_dir.path().join("repo").join("config.ambit");
    AmbitTester::from_temp_dir(&temp_dir)
        .with_repo_file("repo.txt")
        .with_file_with_content(&repo_config_path, "repo.txt => host.txt;")
        .arg("sync")
        .write_stdin("n")
        .assert()
        .success();
    // Should not be symlinked.
    assert!(!is_symlinked(
        temp_dir.path().join("host.txt"),
        temp_dir.path().join("repo").join("repo.txt"),
    ));
}

#[test]
fn sync_use_repo_config_if_required() {
    // Sync without existing configuration file and answer yes to use repo configuration instead.
    let temp_dir = TempDir::new().unwrap();
    let repo_config_path = temp_dir.path().join("repo").join("config.ambit");
    AmbitTester::from_temp_dir(&temp_dir)
        .with_repo_file("repo.txt")
        .with_file_with_content(&repo_config_path, "repo.txt => host.txt;")
        .args(vec!["sync", "--use-repo-config-if-required"])
        // Only need to input 'y' once as --use-repo-config-if-required means that it will always
        // search for repo config if config in default location does not exist.
        .write_stdin("y")
        .assert()
        .success();
    assert!(is_symlinked(
        temp_dir.path().join("host.txt"),
        temp_dir.path().join("repo").join("repo.txt"),
    ));
}

#[test]
fn sync_use_any_repo_config_found() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("repo");
    AmbitTester::from_temp_dir(&temp_dir)
        .with_repo_file("repo.txt")
        .with_file_with_content(&repo_path.join("config.ambit"), "repo.txt => host.txt;")
        .args(vec!["sync", "--use-any-repo-config-found"])
        // With --use-any-repo-config-found flag, only one 'y' needs to be passed
        .write_stdin("y")
        .assert()
        .success();
    assert!(is_symlinked(
        temp_dir.path().join("host.txt"),
        temp_dir.path().join("repo").join("repo.txt"),
    ));
}

#[test]
fn sync_use_any_repo_config_found_if_required() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("repo");
    AmbitTester::from_temp_dir(&temp_dir)
        .with_repo_file("repo.txt")
        .with_file_with_content(&repo_path.join("config.ambit"), "repo.txt => host.txt;")
        // Combine flags --use-any-repo-config-found and --use-repo-config-if-required.
        // This should mean that nothing has to be written to stdin.
        .args(vec![
            "sync",
            "--use-any-repo-config-found",
            "--use-repo-config-if-required",
        ])
        .assert()
        .success();
    assert!(is_symlinked(
        temp_dir.path().join("host.txt"),
        temp_dir.path().join("repo").join("repo.txt"),
    ));
}

#[test]
fn clean_after_sync() {
    let temp_dir = TempDir::new().unwrap();
    let host_path = temp_dir.path().join("host.txt");
    AmbitTester::from_temp_dir(&temp_dir)
        .with_repo_file("repo.txt")
        .with_config("repo.txt => host.txt;")
        .arg("sync")
        .assert()
        .success();
    // host.txt should exist after sync.
    assert!(host_path.exists());
    AmbitTester::from_temp_dir(&temp_dir)
        .with_config("repo.txt => host.txt;")
        .arg("clean")
        .assert()
        .success();
    // host.txt should be deleted after clean.
    assert!(!host_path.exists());
}

#[test]
fn clean_ignores_parent_directories() {
    let temp_dir = TempDir::new().unwrap();
    let host_file_directory = temp_dir.path().join("a").join("b");
    AmbitTester::from_temp_dir(&temp_dir)
        .with_repo_file("repo.txt")
        .with_config("repo.txt => a/b/host.txt;")
        .arg("sync")
        .assert()
        .success();
    // a/b/host.txt should exist after sync.
    assert!(host_file_directory.join("host.txt").exists());
    AmbitTester::from_temp_dir(&temp_dir)
        .with_config("repo.txt => a/b/host.txt;")
        .arg("clean")
        .assert()
        .success();
    // a/b/host.txt should be deleted after clean.
    assert!(!host_file_directory.join("host.txt").exists());
    // a/b path should still exist after clean although it was created from sync invocation.
    assert!(host_file_directory.exists());
}
