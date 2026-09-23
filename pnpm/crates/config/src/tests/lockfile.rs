use super::{
    Config, EnvGuard, EnvVar, EnvVarOs, GetCurrentDir, GetHomeDir, HostNoHome, LinkProbe, OsString,
    Path, PathBuf, assert_eq, fs, io, repo_on_branch, safe_host_var, tempdir,
};

#[test]
fn package_lock_is_the_lockfile_fallback() {
    let package_lock_only = tempdir().unwrap();
    fs::write(package_lock_only.path().join("pnpm-workspace.yaml"), "packageLock: false\n")
        .unwrap();
    let config = Config::new()
        .current::<HostNoHome>(package_lock_only.path())
        .expect("packageLock config loads");
    assert!(!config.package_lock);
    assert!(!config.lockfile);

    let explicit_lockfile = tempdir().unwrap();
    fs::write(
        explicit_lockfile.path().join("pnpm-workspace.yaml"),
        "packageLock: false\nlockfile: true\n",
    )
    .unwrap();
    let config = Config::new()
        .current::<HostNoHome>(explicit_lockfile.path())
        .expect("lockfile config loads");
    assert!(!config.package_lock);
    assert!(config.lockfile);
}

#[test]
pub fn gvs_disabled_or_extend_node_path_off_injects_no_resolution_env() {
    for yaml in [
        "enableGlobalVirtualStore: false\n",
        "enableGlobalVirtualStore: true\nextendNodePath: false\n",
    ] {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("pnpm-workspace.yaml"), yaml)
            .expect("write to pnpm-workspace.yaml");
        let config = Config::new().current::<HostNoHome>(tmp.path()).expect("yaml is valid");
        assert_eq!(config.extra_env.get("NODE_PATH"), None, "yaml: {yaml}");
        assert_eq!(config.extra_env.get("NODE_OPTIONS"), None, "yaml: {yaml}");
    }
}

#[test]
pub fn git_branch_lockfile_names_the_lockfile_after_the_current_branch() {
    let repo = repo_on_branch("ref: refs/heads/feat/Login\n");
    fs::write(repo.path().join("pnpm-workspace.yaml"), "gitBranchLockfile: true\n").unwrap();
    static REPO_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    REPO_DIR
        .set(repo.path().to_path_buf())
        .expect("set once");
    host_in_repo!(HostOnBranch);

    let config = Config::new().current::<HostOnBranch>(repo.path()).expect("yaml is valid");
    assert!(config.use_git_branch_lockfile);
    assert_eq!(config.git_branch_lockfile_name.as_deref(), Some("pnpm-lock.feat!login.yaml"));
    assert_eq!(config.wanted_lockfile_name(), "pnpm-lock.feat!login.yaml");
}

/// Merging collapses the per-branch lockfiles into the shared one, so the
/// install has to be reading and writing `pnpm-lock.yaml` while it does.
#[test]
pub fn merging_puts_the_install_back_on_the_shared_lockfile() {
    let repo = repo_on_branch("ref: refs/heads/main\n");
    fs::write(
        repo.path().join("pnpm-workspace.yaml"),
        "gitBranchLockfile: true\nmergeGitBranchLockfiles: true\n",
    )
    .unwrap();
    static REPO_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    REPO_DIR
        .set(repo.path().to_path_buf())
        .expect("set once");
    host_in_repo!(HostMerging);

    let config = Config::new().current::<HostMerging>(repo.path()).expect("yaml is valid");
    assert!(config.merge_git_branch_lockfiles);
    assert_eq!(config.git_branch_lockfile_name.as_deref(), Some("pnpm-lock.main.yaml"));
    assert_eq!(config.wanted_lockfile_name(), "pnpm-lock.yaml");
}

/// A detached HEAD has no branch to name a lockfile after, so the install
/// stays on `pnpm-lock.yaml` rather than inventing one.
#[test]
pub fn a_detached_head_leaves_the_install_on_the_shared_lockfile() {
    let _guard = EnvGuard::snapshot(["PNPM_GIT_BRANCH"]);
    // SAFETY: EnvGuard serializes this test against others in the process
    unsafe {
        std::env::remove_var("PNPM_GIT_BRANCH");
    }
    let repo = repo_on_branch("0123456789abcdef0123456789abcdef01234567\n");
    fs::write(repo.path().join("pnpm-workspace.yaml"), "gitBranchLockfile: true\n").unwrap();
    static REPO_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    REPO_DIR
        .set(repo.path().to_path_buf())
        .expect("set once");
    host_in_repo!(HostDetached);

    let config = Config::new().current::<HostDetached>(repo.path()).expect("yaml is valid");
    assert!(config.use_git_branch_lockfile);
    assert_eq!(config.git_branch_lockfile_name, None);
    assert_eq!(config.wanted_lockfile_name(), "pnpm-lock.yaml");
}

#[test]
pub fn a_detached_head_with_pnpm_git_branch_selects_the_branch_lockfile() {
    let _guard = EnvGuard::snapshot(["PNPM_GIT_BRANCH"]);
    let repo = repo_on_branch("0123456789abcdef0123456789abcdef01234567\n");
    fs::write(repo.path().join("pnpm-workspace.yaml"), "gitBranchLockfile: true\n").unwrap();
    static REPO_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    REPO_DIR
        .set(repo.path().to_path_buf())
        .expect("set once");
    host_in_repo!(HostDetachedWithEnv);

    // SAFETY: EnvGuard serializes this test against others in the process
    unsafe {
        std::env::set_var("PNPM_GIT_BRANCH", "feat/my-feature");
    }
    let config = Config::new().current::<HostDetachedWithEnv>(repo.path()).expect("yaml is valid");
    assert!(config.use_git_branch_lockfile);
    assert_eq!(config.git_branch_lockfile_name.as_deref(), Some("pnpm-lock.feat!my-feature.yaml"));
    assert_eq!(config.wanted_lockfile_name(), "pnpm-lock.feat!my-feature.yaml");
}

#[test]
pub fn a_detached_head_matches_candidate_branch_lockfile() {
    let _guard = EnvGuard::snapshot(["PNPM_GIT_BRANCH"]);
    // SAFETY: EnvGuard serializes this test against others in the process
    unsafe {
        std::env::remove_var("PNPM_GIT_BRANCH");
    }
    let dir = tempdir().unwrap();
    let repo = dir.path();
    std::process::Command::new("git")
        .args(["init", "-b", "feat/test-detached"])
        .current_dir(repo)
        .status()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(repo)
        .status()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(repo)
        .status()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "commit.gpgsign", "false"])
        .current_dir(repo)
        .status()
        .unwrap();
    fs::write(repo.join("file"), "content").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo)
        .status()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(repo)
        .status()
        .unwrap();
    let sha = String::from_utf8(
        std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    std::process::Command::new("git")
        .args(["checkout", "--detach", sha.trim()])
        .current_dir(repo)
        .status()
        .unwrap();

    fs::write(repo.join("pnpm-workspace.yaml"), "gitBranchLockfile: true\n").unwrap();
    fs::write(repo.join("pnpm-lock.feat!test-detached.yaml"), "").unwrap();

    static REPO_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    REPO_DIR.set(repo.to_path_buf()).expect("set once");
    host_in_repo!(HostDetachedRealGit);

    let config = Config::new().current::<HostDetachedRealGit>(repo).expect("yaml is valid");
    assert!(config.use_git_branch_lockfile);
    assert_eq!(
        config.git_branch_lockfile_name.as_deref(),
        Some("pnpm-lock.feat!test-detached.yaml"),
    );
    assert_eq!(config.wanted_lockfile_name(), "pnpm-lock.feat!test-detached.yaml");
}
