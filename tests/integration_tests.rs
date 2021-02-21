use assert_cmd::{assert::Assert, Command};
use std::{
    ffi::OsStr,
    fs::{self, File},
    path::PathBuf,
};
use tempfile::TempDir;

#[derive(Debug)]
pub struct AmbitTester {
    config_path: PathBuf,
    repo_path: PathBuf,
    executable: Command,
}
// Builder pattern implementation
impl AmbitTester {
    // Allow temp_dir to be passed so it can be owned from outside of the struct.
    fn from_temp_dir(temp_dir: &TempDir) -> Self {
        let config_path = temp_dir.path().join("config.ambit");
        let repo_path = temp_dir.path().join("repo");
        let mut executable = Command::cargo_bin("ambit").unwrap();
        // Set environment variables.
        // AMBIT_HOME_PATH is set as temp_dir. This is important as it will be the prefix path of potential synced files.
        executable.env("AMBIT_HOME_PATH", temp_dir.path().as_os_str());
        executable.env("AMBIT_CONFIG_PATH", config_path.as_os_str());
        executable.env("AMBIT_REPO_PATH", repo_path.as_os_str());
        Self {
            config_path,
            repo_path,
            executable,
        }
    }

    // Write content to configuration file.
    fn with_config(self, content: &str) -> Self {
        fs::write(&self.config_path, content).expect("Unable to write to file");
        self
    }

    // Create a custom file in repo_path directory. Mimics repo_file.
    fn with_repo_file(self, name: &str) -> Self {
        File::create(self.repo_path.join(name)).unwrap();
        self
    }

    // Creates configuration file and repository directory with .git.
    fn with_default_paths(self) -> Self {
        fs::create_dir_all(&self.repo_path.join(".git")).unwrap();
        File::create(&self.config_path).unwrap();
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
    // The repository directory is already created when calling `with_default_paths()`.
    AmbitTester::default()
        .with_default_paths()
        .arg("init")
        .assert()
        .stderr("ERROR: Dotfile repository already exists.\nUse '-f' flag to overwrite.\n");
}

#[test]
fn init_force_overwrites() {
    AmbitTester::default()
        .with_default_paths()
        .args(vec!["init", "-f"])
        .assert()
        .success();
}

#[test]
fn clone_repo_already_exists() {
    // Expect an error when attempting to clone without force flag.
    AmbitTester::default()
        .with_default_paths()
        .args(vec!["clone", "https://github.com/plamorg/ambit"])
        .assert()
        .stderr("ERROR: Dotfile repository already exists.\nUse '-f' flag to overwrite.\n");
}

#[test]
fn sync_without_repo() {
    // Error should occur if attempting to sync without initializing.
    // `with_default_paths` is omitted here.
    AmbitTester::default().arg("sync").assert().stderr(
        "ERROR: Dotfile repository does not exist. Run `init` or `clone` before syncing.\n",
    );
}

#[test]
fn sync_host_file_already_exists() {
    // The host file already exists but is not symlinked to repo file.
    let temp_dir = TempDir::new().unwrap();
    File::create(temp_dir.path().join("host.txt")).unwrap();
    AmbitTester::from_temp_dir(&temp_dir)
        .with_default_paths()
        .with_repo_file("repo.txt")
        .with_config("repo.txt => host.txt;")
        .arg("sync")
        .assert()
        .failure();
}

#[test]
fn sync_repo_file_does_not_exist() {
    // Repo file should exist for sync to work.
    AmbitTester::default()
        .with_default_paths()
        .with_config("repo.txt => host.txt;")
        .arg("sync")
        .assert()
        .failure();
}

#[test]
fn sync_normal() {
    let temp_dir = TempDir::new().unwrap();
    AmbitTester::from_temp_dir(&temp_dir)
        .with_default_paths()
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
fn sync_dry_run_should_not_symlink() {
    let temp_dir = TempDir::new().unwrap();
    AmbitTester::from_temp_dir(&temp_dir)
        .with_default_paths()
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
        .with_default_paths()
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
