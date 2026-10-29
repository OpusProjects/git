// What export says when git is there but tar is not. Its own test binary,
// because it narrows PATH for the whole process.
use std::collections::HashMap;
use std::path::Path;

use git_rs::{CliGit, GitPort, Remote};

async fn git(dir: &Path, args: &[&str]) {
    let status = tokio::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .status()
        .await
        .unwrap();
    assert!(status.success());
}

#[tokio::test]
async fn export_says_tar_is_missing() {
    let origin = tempfile::tempdir().unwrap();
    git(origin.path(), &["init", "--initial-branch", "main"]).await;
    tokio::fs::write(origin.path().join("a.txt"), "a\n")
        .await
        .unwrap();
    git(origin.path(), &["add", "."]).await;
    git(origin.path(), &["commit", "-m", "a"]).await;
    let remote = Remote {
        url: format!("file://{}", origin.path().display()),
        branch: "main".to_string(),
    };
    let work = tempfile::tempdir().unwrap();
    let checkout = work.path().join("prj");
    let cli = CliGit::new();
    cli.ensure(&checkout, &remote, &HashMap::new())
        .await
        .unwrap();
    let sha = cli.revision(&checkout).await.unwrap();

    // A PATH that holds git (and what git runs internally) but no tar
    let which = std::process::Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    let git_path = String::from_utf8(which.stdout).unwrap().trim().to_string();
    let bin = work.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::os::unix::fs::symlink(&git_path, bin.join("git")).unwrap();
    // SAFETY: the only test in this binary, so no other thread reads the
    // environment while it changes.
    unsafe { std::env::set_var("PATH", &bin) };

    let err = cli
        .export(
            &checkout,
            &remote,
            &HashMap::new(),
            &sha,
            ".",
            &work.path().join("out"),
        )
        .await
        .unwrap_err();
    assert!(err.message.starts_with("tar: "), "{}", err.message);
}
