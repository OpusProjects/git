// Integration tests for CliGit. They create real repositories in temp
// directories and talk to them over the file:// protocol — no network, but the
// same code paths as an https remote (minus the authentication itself).
use std::collections::HashMap;
use std::path::Path;

use git_rs::{CliGit, GitPort, Remote};

pub async fn run(dir: &Path, args: &[&str]) {
    let status = tokio::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        // Identity for the test commits — independent of the host's gitconfig
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .status()
        .await
        .expect("failed to run git");
    assert!(status.success(), "git {:?} failed", args);
}

// Creates an origin repository with one committed script and returns
// (tempdir guard, file:// url)
async fn make_origin() -> (tempfile::TempDir, String) {
    let origin = tempfile::tempdir().unwrap();
    run(origin.path(), &["init", "--initial-branch", "main"]).await;
    tokio::fs::write(origin.path().join("fetch.py"), "print('v1')\n")
        .await
        .unwrap();
    run(origin.path(), &["add", "."]).await;
    run(origin.path(), &["commit", "-m", "v1"]).await;
    let url = format!("file://{}", origin.path().display());
    (origin, url)
}

fn remote(url: &str) -> Remote {
    Remote {
        url: url.to_string(),
        branch: "main".to_string(),
    }
}

fn made<T: Default>() -> T {
    T::default()
}

fn none() -> HashMap<String, String> {
    HashMap::new()
}

async fn read(path: &Path) -> String {
    tokio::fs::read_to_string(path).await.unwrap()
}

#[tokio::test]
async fn ensure_clones_a_fresh_checkout() {
    let (_origin, url) = make_origin().await;
    let workdir = tempfile::tempdir().unwrap();
    // A parent that does not exist yet is created
    let checkout = workdir.path().join("projects").join("prj-test");

    CliGit::new()
        .ensure(&checkout, &remote(&url), &none())
        .await
        .expect("clone failed");

    assert_eq!(read(&checkout.join("fetch.py")).await, "print('v1')\n");
}

#[tokio::test]
async fn ensure_updates_an_existing_checkout() {
    let (origin, url) = make_origin().await;
    let workdir = tempfile::tempdir().unwrap();
    let checkout = workdir.path().join("prj-test");

    // Through Default, as a service holding it in a generic container would
    let git: CliGit = made();
    git.ensure(&checkout, &remote(&url), &none())
        .await
        .expect("clone failed");

    // The origin moves forward...
    tokio::fs::write(origin.path().join("fetch.py"), "print('v2')\n")
        .await
        .unwrap();
    run(origin.path(), &["commit", "-am", "v2"]).await;

    // ...and a second ensure() brings the checkout to the new tip
    git.ensure(&checkout, &remote(&url), &none())
        .await
        .expect("update failed");

    assert_eq!(read(&checkout.join("fetch.py")).await, "print('v2')\n");
}

#[tokio::test]
async fn ensure_discards_local_modifications() {
    let (_origin, url) = make_origin().await;
    let workdir = tempfile::tempdir().unwrap();
    let checkout = workdir.path().join("prj-test");

    let git = CliGit::new();
    git.ensure(&checkout, &remote(&url), &none()).await.unwrap();

    // Someone (or something) edits the checkout in place — the next ensure
    // must restore the repository state, not carry the drift forward
    tokio::fs::write(checkout.join("fetch.py"), "tampered\n")
        .await
        .unwrap();

    git.ensure(&checkout, &remote(&url), &none()).await.unwrap();

    assert_eq!(read(&checkout.join("fetch.py")).await, "print('v1')\n");
}

#[tokio::test]
async fn ensure_fails_cleanly_on_bad_url() {
    let workdir = tempfile::tempdir().unwrap();
    let checkout = workdir.path().join("prj-test");

    let err = CliGit::new()
        .ensure(&checkout, &remote("file:///nonexistent/repo.git"), &none())
        .await
        .expect_err("expected clone to fail");
    assert!(err.message.contains("git clone"));
}

// The checkout's parent cannot be created (a file is in the way): the error
// names it, before git is ever run.
#[tokio::test]
async fn ensure_names_a_parent_it_cannot_create() {
    let workdir = tempfile::tempdir().unwrap();
    let blocker = workdir.path().join("blocker");
    tokio::fs::write(&blocker, "a file").await.unwrap();

    let err = CliGit::new()
        .ensure(
            &blocker.join("sub").join("prj"),
            &remote("file:///nowhere"),
            &none(),
        )
        .await
        .unwrap_err();
    assert!(err.message.starts_with("create '"), "{}", err.message);
}

// A checkout path git cannot be handed (not UTF-8) is refused by name.
#[tokio::test]
async fn ensure_refuses_a_path_that_is_not_utf8() {
    use std::os::unix::ffi::OsStrExt;
    let workdir = tempfile::tempdir().unwrap();
    let odd = workdir
        .path()
        .join(std::ffi::OsStr::from_bytes(b"prj-\xff"));

    let err = CliGit::new()
        .ensure(&odd, &remote("file:///nowhere"), &none())
        .await
        .unwrap_err();
    assert!(err.message.contains("not valid UTF-8"), "{}", err.message);
}

// A project pointed at another repository: the existing checkout follows the
// new address instead of pulling the one it was cloned from.
#[tokio::test]
async fn ensure_follows_a_changed_url() {
    let (_first, first_url) = make_origin().await;
    let second = tempfile::tempdir().unwrap();
    run(second.path(), &["init", "--initial-branch", "main"]).await;
    tokio::fs::write(second.path().join("fetch.py"), "print('moved')\n")
        .await
        .unwrap();
    run(second.path(), &["add", "."]).await;
    run(second.path(), &["commit", "-m", "moved"]).await;
    let second_url = format!("file://{}", second.path().display());

    let workdir = tempfile::tempdir().unwrap();
    let checkout = workdir.path().join("prj-test");
    let git = CliGit::new();
    git.ensure(&checkout, &remote(&first_url), &none())
        .await
        .unwrap();
    git.ensure(&checkout, &remote(&second_url), &none())
        .await
        .expect("pull from the new address");

    assert_eq!(read(&checkout.join("fetch.py")).await, "print('moved')\n");
}

// A token written into the address is on git's command line and in its
// stderr; neither may reach the error a service logs and shows.
#[tokio::test]
async fn a_token_in_the_url_never_reaches_the_error() {
    let workdir = tempfile::tempdir().unwrap();
    let git = CliGit::new();
    // No repository there: the clone fails at once, with the address (token
    // included) on its command line.
    let url = "file://bot:s3cr3t-token@localhost/nonexistent/repo.git";
    let err = git
        .ensure(&workdir.path().join("prj-a"), &remote(url), &none())
        .await
        .expect_err("nothing to clone from");
    assert!(err.message.contains("git clone"), "{}", err.message);
    assert!(!err.message.contains("s3cr3t-token"), "{}", err.message);

    // And from a pull of an existing checkout.
    let (_origin, origin_url) = make_origin().await;
    let checkout = workdir.path().join("prj-b");
    git.ensure(&checkout, &remote(&origin_url), &none())
        .await
        .unwrap();
    let err = git
        .ensure(&checkout, &remote(url), &none())
        .await
        .expect_err("nothing to fetch from");
    assert!(!err.message.contains("s3cr3t-token"), "{}", err.message);
}

// Each kind of credential is accepted (a file:// remote ignores the
// authentication itself, which is what lets the clone go through here).
#[tokio::test]
async fn ensure_runs_with_every_kind_of_credential() {
    let (_origin, url) = make_origin().await;
    let workdir = tempfile::tempdir().unwrap();
    let kinds: [&[(&str, &str)]; 4] = [
        &[("token", "t0k")],
        &[("token", "t0k"), ("username", "bot")],
        &[("username", "bot"), ("password", "pw")],
        &[("ssh_key_path", "/run/secrets/key")],
    ];
    for (i, kind) in kinds.into_iter().enumerate() {
        let credentials: HashMap<String, String> = kind
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        CliGit::new()
            .ensure(
                &workdir.path().join(format!("p{i}")),
                &remote(&url),
                &credentials,
            )
            .await
            .unwrap_or_else(|e| panic!("{kind:?}: {}", e.message));
    }
}

#[tokio::test]
async fn revision_and_tree_name_what_the_checkout_holds() {
    let (origin, url) = make_origin().await;
    commit_file(origin.path(), "apps/a/x.yaml", "a: 1\n", "add a").await;
    let workdir = tempfile::tempdir().unwrap();
    let checkout = workdir.path().join("prj-test");
    let git = CliGit::new();
    git.ensure(&checkout, &remote(&url), &none()).await.unwrap();

    let head = git.revision(&checkout).await.expect("a revision");
    assert_eq!(head.len(), 40);
    let root = git.tree(&checkout, ".").await.expect("the root tree");
    let apps = git.tree(&checkout, "apps").await.expect("a folder's tree");
    assert_ne!(root, apps);
    assert_eq!(git.tree(&checkout, "missing").await, None);

    // No checkout at all
    let empty = workdir.path().join("empty");
    tokio::fs::create_dir_all(&empty).await.unwrap();
    assert_eq!(git.revision(&empty).await, None);
    assert_eq!(git.tree(&empty, ".").await, None);
}

async fn head(dir: &Path) -> String {
    CliGit::new().revision(dir).await.expect("a revision")
}

async fn commit_file(origin: &Path, file: &str, body: &str, message: &str) {
    let path = origin.join(file);
    tokio::fs::create_dir_all(path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&path, body).await.unwrap();
    run(origin, &["add", "."]).await;
    run(origin, &["commit", "-m", message]).await;
}

#[tokio::test]
async fn commits_lists_what_a_shallow_pull_brought() {
    let (origin, url) = make_origin().await;
    let workdir = tempfile::tempdir().unwrap();
    let checkout = workdir.path().join("prj-test");
    let git = CliGit::new();
    let remote = remote(&url);
    git.ensure(&checkout, &remote, &none()).await.unwrap();
    let from = head(&checkout).await;

    // Three commits land; the pull fetches only the tip.
    commit_file(origin.path(), "apps/a/x.yaml", "a: 1\n", "add a").await;
    commit_file(origin.path(), "apps/b/x.yaml", "b: 1\n", "add b").await;
    commit_file(origin.path(), "apps/a/x.yaml", "a: 2\n", "change a").await;
    git.ensure(&checkout, &remote, &none()).await.unwrap();
    let to = head(&checkout).await;

    let range = git
        .commits(&checkout, &remote, &none(), &from, &to, 20)
        .await
        .expect("commits");
    let messages: Vec<&str> = range.commits.iter().map(|c| c.message.as_str()).collect();
    assert_eq!(messages, ["change a", "add b", "add a"]);
    assert_eq!(range.commits[0].revision, to);
    assert_eq!(range.commits[0].author, "test");
    assert!(range.commits[0].at > 0);
    assert!(range.fetched, "the shallow checkout needed deepening");
    assert!(!range.more && !range.forced);
    let mut paths = range.paths.expect("paths");
    paths.sort();
    assert_eq!(paths, ["apps/a/x.yaml", "apps/b/x.yaml"]);

    // A cap shorter than what came in says there is more.
    let capped = git
        .commits(&checkout, &remote, &none(), &from, &to, 2)
        .await
        .unwrap();
    assert_eq!(capped.commits.len(), 2);
    assert!(capped.more);
}

#[tokio::test]
async fn commits_tells_a_rewritten_history() {
    let (origin, url) = make_origin().await;
    commit_file(origin.path(), "apps/a/x.yaml", "a: 1\n", "add a").await;
    let workdir = tempfile::tempdir().unwrap();
    let checkout = workdir.path().join("prj-test");
    let git = CliGit::new();
    let remote = remote(&url);
    git.ensure(&checkout, &remote, &none()).await.unwrap();
    let from = head(&checkout).await;

    // The last commit is replaced (amend + force push, as far as a clone
    // can tell).
    tokio::fs::write(origin.path().join("apps/a/x.yaml"), "a: 9\n")
        .await
        .unwrap();
    run(
        origin.path(),
        &["commit", "-a", "--amend", "-m", "add a, again"],
    )
    .await;
    git.ensure(&checkout, &remote, &none()).await.unwrap();
    let to = head(&checkout).await;
    assert_ne!(from, to);

    let range = git
        .commits(&checkout, &remote, &none(), &from, &to, 20)
        .await
        .unwrap();
    assert!(range.forced, "{range:?}");
    assert_eq!(range.commits[0].message, "add a, again");
    assert_eq!(range.paths.unwrap(), ["apps/a/x.yaml"]);
}

// A history deeper than one deepening reaches: only the new tip is certain,
// and it is not called a rewrite, because the history was not all there.
#[tokio::test]
async fn commits_past_the_deepening_are_more_not_a_rewrite() {
    let (origin, url) = make_origin().await;
    let workdir = tempfile::tempdir().unwrap();
    let checkout = workdir.path().join("prj-test");
    let git = CliGit::new();
    let remote = remote(&url);
    git.ensure(&checkout, &remote, &none()).await.unwrap();
    let from = head(&checkout).await;

    for i in 0..4 {
        commit_file(origin.path(), "x.txt", &format!("{i}\n"), &format!("c{i}")).await;
    }
    git.ensure(&checkout, &remote, &none()).await.unwrap();
    let to = head(&checkout).await;

    let range = git
        .commits(&checkout, &remote, &none(), &from, &to, 1)
        .await
        .unwrap();
    assert!(range.fetched && range.more, "{range:?}");
    assert!(!range.forced, "{range:?}");
    assert_eq!(range.commits.len(), 1);
    assert_eq!(range.commits[0].message, "c3");
}

// Both ids are known locally but share no history and the checkout is not
// shallow: nothing to fetch, and the old tip is simply not in the new history.
#[tokio::test]
async fn commits_between_unrelated_histories_in_a_full_clone() {
    let (_origin, url) = make_origin().await;
    let other = tempfile::tempdir().unwrap();
    run(other.path(), &["init", "--initial-branch", "main"]).await;
    commit_file(other.path(), "y.txt", "y\n", "unrelated").await;

    // A full (not shallow) clone that also holds the unrelated history
    let workdir = tempfile::tempdir().unwrap();
    let checkout = workdir.path().join("full");
    let source = url.trim_start_matches("file://").to_string();
    run(workdir.path(), &["clone", "--no-local", &source, "full"]).await;
    run(
        &checkout,
        &["fetch", other.path().to_str().unwrap(), "main:unrelated"],
    )
    .await;
    let from = head(&checkout).await;
    let to = String::from_utf8(
        std::process::Command::new("git")
            .args(["-C", checkout.to_str().unwrap(), "rev-parse", "unrelated"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();

    let range = CliGit::new()
        .commits(&checkout, &remote(&url), &none(), &from, &to, 20)
        .await
        .unwrap();
    assert!(!range.fetched && range.more && range.forced, "{range:?}");
    assert_eq!(range.commits[0].message, "unrelated");
}

#[tokio::test]
async fn commits_refuses_what_is_not_a_commit_id() {
    let git = CliGit::new();
    let remote = remote("file:///nowhere");
    let dir = tempfile::tempdir().unwrap();
    let err = git
        .commits(dir.path(), &remote, &none(), "--all", "abcd", 20)
        .await
        .unwrap_err();
    assert!(err.message.contains("not a commit id"), "{}", err.message);
}

// Ids that look right but name nothing: git answers with failures, which
// leave the range empty rather than wrong.
#[tokio::test]
async fn commits_of_unknown_ids_come_back_empty() {
    let (_origin, url) = make_origin().await;
    let workdir = tempfile::tempdir().unwrap();
    let checkout = workdir.path().join("full");
    let source = url.trim_start_matches("file://").to_string();
    run(workdir.path(), &["clone", "--no-local", &source, "full"]).await;

    let range = CliGit::new()
        .commits(&checkout, &remote(&url), &none(), "abcd", "dcba", 20)
        .await
        .unwrap();
    assert!(
        range.commits.is_empty() && range.paths.is_none(),
        "{range:?}"
    );
}

#[tokio::test]
async fn commits_name_a_failed_deepening() {
    let (_origin, url) = make_origin().await;
    let workdir = tempfile::tempdir().unwrap();
    let checkout = workdir.path().join("prj-test");
    let git = CliGit::new();
    git.ensure(&checkout, &remote(&url), &none()).await.unwrap();
    let from = head(&checkout).await;

    // The branch is gone by the time the history is needed
    let gone = Remote {
        url: url.clone(),
        branch: "gone".to_string(),
    };
    let err = git
        .commits(&checkout, &gone, &none(), &from, "abcd", 20)
        .await
        .unwrap_err();
    assert!(err.message.contains("git -C"), "{}", err.message);
}

#[tokio::test]
async fn commits_refuse_a_path_that_is_not_utf8() {
    use std::os::unix::ffi::OsStrExt;
    let odd = std::path::PathBuf::from(std::ffi::OsStr::from_bytes(b"/tmp/prj-\xff"));
    let err = CliGit::new()
        .commits(&odd, &remote("file:///nowhere"), &none(), "abcd", "dcba", 1)
        .await
        .unwrap_err();
    assert!(err.message.contains("not valid UTF-8"), "{}", err.message);
}

// =========================================================================
// export: a folder as it was at an older commit
// =========================================================================

#[tokio::test]
async fn export_writes_a_folder_as_it_was() {
    let (origin, url) = make_origin().await;
    commit_file(origin.path(), "apps/a/x.yaml", "a: 1\n", "add a").await;
    let workdir = tempfile::tempdir().unwrap();
    let checkout = workdir.path().join("prj-test");
    let git = CliGit::new();
    let remote = remote(&url);
    git.ensure(&checkout, &remote, &none()).await.unwrap();
    let old = head(&checkout).await;

    commit_file(origin.path(), "apps/a/x.yaml", "a: 2\n", "change a").await;
    git.ensure(&checkout, &remote, &none()).await.unwrap();

    // The old commit is not in the shallow checkout any more: export fetches
    // the history once and finds it.
    let dest = workdir.path().join("out");
    let sha = git
        .export(&checkout, &remote, &none(), &old[..12], "apps", &dest)
        .await
        .expect("export");
    assert_eq!(sha, old);
    assert_eq!(read(&dest.join("apps/a/x.yaml")).await, "a: 1\n");
    // The checkout itself did not move
    assert_eq!(read(&checkout.join("apps/a/x.yaml")).await, "a: 2\n");

    // Now in the full history: no fetch needed this time
    let again = workdir.path().join("again");
    git.export(&checkout, &remote, &none(), &old, "apps", &again)
        .await
        .unwrap();
    assert_eq!(read(&again.join("apps/a/x.yaml")).await, "a: 1\n");
}

#[tokio::test]
async fn export_refuses_what_is_not_a_commit_id() {
    let dir = tempfile::tempdir().unwrap();
    let err = CliGit::new()
        .export(
            dir.path(),
            &remote("file:///nowhere"),
            &none(),
            "HEAD~1",
            ".",
            dir.path(),
        )
        .await
        .unwrap_err();
    assert!(err.message.contains("not a commit id"), "{}", err.message);
}

#[tokio::test]
async fn export_names_what_it_cannot_find() {
    let (_origin, url) = make_origin().await;
    let workdir = tempfile::tempdir().unwrap();
    let checkout = workdir.path().join("prj-test");
    let git = CliGit::new();
    let remote = remote(&url);
    git.ensure(&checkout, &remote, &none()).await.unwrap();
    let dest = workdir.path().join("out");

    // A commit the repository never had
    let err = git
        .export(&checkout, &remote, &none(), "abcdef12", ".", &dest)
        .await
        .unwrap_err();
    assert!(
        err.message.contains("not in the project's history"),
        "{}",
        err.message
    );

    // A path the commit does not hold
    let sha = head(&checkout).await;
    let err = git
        .export(&checkout, &remote, &none(), &sha, "missing", &dest)
        .await
        .unwrap_err();
    assert!(err.message.starts_with("git archive"), "{}", err.message);

    // A destination that cannot be created
    let blocker = workdir.path().join("blocker");
    tokio::fs::write(&blocker, "a file").await.unwrap();
    let err = git
        .export(&checkout, &remote, &none(), &sha, ".", &blocker.join("x"))
        .await
        .unwrap_err();
    assert!(err.message.contains("blocker"), "{}", err.message);

    // Not shallow: the history is fetched as it is, and the commit is still
    // not there
    let full = workdir.path().join("full");
    let source = url.trim_start_matches("file://").to_string();
    run(workdir.path(), &["clone", "--no-local", &source, "full"]).await;
    let err = git
        .export(&full, &remote, &none(), "abcdef12", ".", &dest)
        .await
        .unwrap_err();
    assert!(
        err.message.contains("not in the project's history"),
        "{}",
        err.message
    );

    // A destination tar cannot write into (it exists, but read-only)
    let locked = workdir.path().join("locked");
    tokio::fs::create_dir_all(&locked).await.unwrap();
    let mut perms = std::fs::metadata(&locked).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o555);
    std::fs::set_permissions(&locked, perms).unwrap();
    let err = git
        .export(&checkout, &remote, &none(), &sha, ".", &locked)
        .await
        .unwrap_err();
    assert!(err.message.starts_with("tar: "), "{}", err.message);
}

#[tokio::test]
async fn export_names_a_failed_fetch() {
    let (_origin, url) = make_origin().await;
    let workdir = tempfile::tempdir().unwrap();
    let checkout = workdir.path().join("prj-test");
    let git = CliGit::new();
    git.ensure(&checkout, &remote(&url), &none()).await.unwrap();

    let err = git
        .export(
            &checkout,
            &Remote {
                url: url.clone(),
                branch: "gone".to_string(),
            },
            &none(),
            "abcdef12",
            ".",
            &workdir.path().join("out"),
        )
        .await
        .unwrap_err();
    assert!(err.message.contains("git -C"), "{}", err.message);
}

#[tokio::test]
async fn export_refuses_a_path_that_is_not_utf8() {
    use std::os::unix::ffi::OsStrExt;
    let odd = std::path::PathBuf::from(std::ffi::OsStr::from_bytes(b"/tmp/prj-\xff"));
    let err = CliGit::new()
        .export(&odd, &remote("file:///nowhere"), &none(), "abcd", ".", &odd)
        .await
        .unwrap_err();
    assert!(err.message.contains("not valid UTF-8"), "{}", err.message);
}
