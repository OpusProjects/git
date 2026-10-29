// What CliGit says when the git binary is not there at all. Its own test
// binary, because it empties PATH for the whole process.
use std::collections::HashMap;
use std::path::Path;

use git_rs::{CliGit, GitPort, Remote};

#[tokio::test]
async fn every_operation_says_git_is_missing() {
    // SAFETY: the only test in this binary, so no other thread reads the
    // environment while it changes.
    unsafe { std::env::set_var("PATH", "") };

    let git = CliGit::new();
    let dir = tempfile::tempdir().unwrap();
    let remote = Remote {
        url: "file:///nowhere".to_string(),
        branch: "main".to_string(),
    };
    let none = HashMap::new();

    let err = git
        .ensure(&dir.path().join("prj"), &remote, &none)
        .await
        .unwrap_err();
    assert!(
        err.message.starts_with("failed to run git"),
        "{}",
        err.message
    );

    assert_eq!(git.revision(dir.path()).await, None);
    assert_eq!(git.tree(dir.path(), ".").await, None);

    let err = git
        .commits(dir.path(), &remote, &none, "abcd", "dcba", 1)
        .await
        .unwrap_err();
    assert!(err.message.starts_with("git: "), "{}", err.message);

    let err = git
        .export(dir.path(), &remote, &none, "abcd", ".", Path::new("/tmp"))
        .await
        .unwrap_err();
    assert!(
        err.message.starts_with("git rev-parse: "),
        "{}",
        err.message
    );
}
