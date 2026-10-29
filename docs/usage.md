# Usage

How a service uses the `git-rs` crate: the trait it codes against, the adapter that implements
it, how credentials reach git, and what each operation does.

- [Depending on the crate](#depending-on-the-crate)
- [GitPort and CliGit](#gitport-and-cligit)
- [Credentials](#credentials)
- [ensure: clone or pull](#ensure-clone-or-pull)
- [revision and tree](#revision-and-tree)
- [commits: what a pull brought](#commits-what-a-pull-brought)
- [export: a folder at an older commit](#export-a-folder-at-an-older-commit)
- [Redacting addresses](#redacting-addresses)
- [Errors and timeouts](#errors-and-timeouts)

---

## Depending on the crate

A service names a release tag of this repository in its `Cargo.toml`, never a branch.

```toml
git-rs = { git = "https://github.com/OpusProjects/git", tag = "v0.1.0" }
```

The machine that runs the service needs the `git` binary (2.31 or newer, for the credential
variables), and `tar` for `export`.

---

## GitPort and CliGit

A service holds a `GitPort` and builds a `CliGit` once, where it wires its adapters together.

```rust
use std::sync::Arc;
use git_rs::{CliGit, GitPort, Remote};

let git: Arc<dyn GitPort> = Arc::new(CliGit::new());
let remote = Remote { url: project.git_url.clone(), branch: project.branch.clone() };
git.ensure(&checkout_dir, &remote, &credentials).await?;
```

`Remote` is the part of a service's project item git needs: the address and the branch. The rest
of the item (where its credentials live, how often it is pulled, how long a pull may take) stays
the service's. Tests replace `CliGit` with their own `GitPort`: only `ensure` is required, and the
other methods default to "cannot tell" or a clear error.

| Method | Answers |
|---|---|
| `ensure(dir, remote, credentials)` | Clones or pulls; `Ok(())` or a `GitError` |
| `revision(dir)` | The commit the checkout is at, or `None` |
| `tree(dir, folder)` | The tree id of a folder at the checkout's HEAD, or `None` |
| `commits(dir, remote, credentials, from, to, max)` | A `CommitRange` |
| `export(dir, remote, credentials, revision, path, dest)` | The full commit id it exported |

---

## Credentials

Credentials arrive already resolved, as a map of field names to values; an empty map is a public repository.

| Fields | How git gets them |
|---|---|
| `ssh_key_path` | `GIT_SSH_COMMAND="ssh -i <path> -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new"` |
| `token`, optionally `username` | An `Authorization: Basic` header for `username:token` (`x-access-token` without a username) |
| `username` and `password` | An `Authorization: Basic` header for `username:password` |

The header is passed as `http.extraHeader` through the `GIT_CONFIG_COUNT`, `GIT_CONFIG_KEY_0` and
`GIT_CONFIG_VALUE_0` environment variables, never as an argument: every user on the machine can
read a process's arguments in `/proc`. An SSH key wins when several fields are present.

---

## ensure: clone or pull

`ensure` makes a directory an up-to-date checkout of the remote's branch, whatever state it is in.

- **No `.git` in the directory:** its parent is created if needed, then `git clone --depth 1
  --branch <branch> <url> <dir>`.
- **A checkout already there:** `git remote set-url origin <url>`, then `git fetch --depth 1 origin
  <branch>` and `git reset --hard FETCH_HEAD`.

Setting the address on every pull is what makes a moved repository work: `origin` was fixed when
the checkout was cloned. The hard reset discards anything changed in the checkout by hand, because
the repository, not the disk, is what the service should be running.

---

## revision and tree

Two read-only questions about the checkout, answered without contacting the remote.

`revision` is the full id of the commit at HEAD: what a service shows as "the commit running
now". `tree` is the id of a folder's tree at HEAD (`"."` for the root): it changes only when
something inside that folder changes, so a service can tell whether one folder moved when any
commit anywhere in the repository moves the commit id. Both answer `None` when there is no
checkout or no such folder.

---

## commits: what a pull brought

`commits(from, to, max)` lists the commits after `from` up to `to`, newest first, and the paths that differ.

Checkouts are shallow, so when `from` is not yet an ancestor of `to`, the history is deepened
once by `max` commits (`git fetch --deepen=<max>`), never fetched whole.

| Field | Meaning |
|---|---|
| `commits` | At most `max`, each with `revision`, `author`, `message` (first line) and `at` (Unix seconds) |
| `more` | Commits were left out: more than `max`, or a history deeper than one deepening |
| `forced` | `from` is not in the new history at all: it was rewritten (a force push) |
| `paths` | Files that differ between `from` and `to`, or `None` when git cannot compare them |
| `fetched` | Answering took a fetch from the remote |

`forced` is only claimed when the whole new history was there to look at. When the deepening did
not reach `from`, the answer is `more` with only the new tip listed, because "too deep to see"
and "rewritten" cannot be told apart from a partial history. Both revisions must be commit ids
(four to sixty-four hexadecimal characters); anything else is refused before git runs.

---

## export: a folder at an older commit

`export(revision, path, dest)` writes `path` as it was at `revision` into `dest`, and answers the full commit id.

`git archive` streams the tree straight from the object store and `tar` unpacks it into `dest`,
so the checkout's files, index and HEAD do not move, and a pull running at the same time is not
disturbed. When the commit is not in the shallow checkout, the branch's history is fetched once
(`--unshallow`) and the commit looked up again. The revision must be a commit id, never a ref
expression: it usually comes from a request, and it ends up on a command line.

---

## Redacting addresses

Two functions remove credentials written into an address, for wherever the address is shown.

| Function | Example |
|---|---|
| `without_userinfo(url)` | `https://bot:t0k@host/x.git` → `https://host/x.git` |
| `redact_userinfo(text)` | Every address in a text, such as a git error message, without its userinfo |

`CliGit` applies `redact_userinfo` to every error it returns, so an address with a token in it does
not reach a log or an API response through an error. A service should still show a configured
address through `without_userinfo`, and prefer a credential to a token written into the address.

---

## Errors and timeouts

Every failure is a `GitError` with a message that names the git command and quotes git's own error.

Git runs with `GIT_TERMINAL_PROMPT=0`, so a missing credential fails at once rather than waiting for
a prompt. Nothing here has a timeout of its own: the service bounds each call, for example with
`tokio::time::timeout`, and dropping the future kills the git process it started, so a hung remote
does not leave one git process behind per attempt.
