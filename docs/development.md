# Development

How the crate is laid out, what CI checks, how the tests work, and how to build a service against a
local copy before anything is released.

- [Layout](#layout)
- [CI gates](#ci-gates)
- [Tests and coverage](#tests-and-coverage)
- [Working on a service against a local copy](#working-on-a-service-against-a-local-copy)

---

## Layout

One crate, `unified-git`, at the root of the repository.

```
Cargo.toml                    # the crate: version, MSRV, dependencies
src/
├── lib.rs                    # module tree and re-exports
├── port.rs                   # GitPort, Remote, Commit, CommitRange, GitError
├── cli.rs                    # CliGit: the GitPort that runs the git binary
└── redact.rs                 # without_userinfo, redact_userinfo
tests/                        # integration tests against real repositories
docs/                         # this documentation
release-please-config.json    # release-please: version file, changelog sections
.release-please-manifest.json # the last released version
```

---

## CI gates

`.github/workflows/build.yaml` runs these on every pull request; `test`, `audit` and `deny` are required.

| Job | What it checks |
|---|---|
| `test` | `cargo fmt --check`, clippy with warnings as errors, a check under the MSRV, tests with 100% line coverage |
| `audit` | `cargo audit`: no RUSTSEC advisory in `Cargo.lock` (also nightly, on `main`) |
| `deny` | `cargo deny check`: licenses, sources and advisories, per `deny.toml` (also nightly) |

The MSRV is `rust-version` in `Cargo.toml`, the same as the services': a language feature newer
than their pinned toolchain must fail here, not in their builds.

---

## Tests and coverage

Every line of the crate runs in a test, and the `test` check fails a pull request that changes that.

| Where | What goes there |
|---|---|
| `#[cfg(test)] mod tests` at the bottom of a file in `src/` | Unit tests of that file: credential variables, redaction, the trait's defaults |
| `tests/cli_test.rs` | `CliGit` against real repositories created in temporary directories, over `file://` |
| `tests/no_git_test.rs` | What every operation answers when there is no `git` on `PATH` |
| `tests/no_tar_test.rs` | What `export` answers when `git` is there but `tar` is not |

The two `no_*` files are their own test binaries because each changes `PATH` for the whole
process. Test code under `tests/` is not counted by `cargo llvm-cov`.

```bash
cargo llvm-cov --summary-only          # the number CI checks (lines: 100%)
cargo llvm-cov --html --open           # which lines a change left uncovered
```

An error that cannot happen with the program as written (a pipe this code just opened) is an
`expect` naming why, not an error path no test can reach. Errors from starting a program go
through one function, `io_fail`, so the tests that remove `git` and `tar` cover them all.

---

## Working on a service against a local copy

A change here is a change in every service that pins it, so build one against it before it is merged.

With this repository checked out next to the service, add a `[patch]` section to the service's
`Cargo.toml` **without committing it**:

```toml
[patch."https://github.com/OpusProjects/git"]
unified-git = { path = "../git" }
```

Then run the service's own gates (`cargo clippy --all-targets -- -D warnings` and `cargo test`):
its tests are what prove a change here did not alter what the service does. Remove the section
before committing anything in the service.
