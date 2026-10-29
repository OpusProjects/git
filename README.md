# Git

[![git CI](https://github.com/OpusProjects/git/actions/workflows/build.yaml/badge.svg)](https://github.com/OpusProjects/git/actions/workflows/build.yaml)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-edition%202024-orange.svg?logo=rust)](https://www.rust-lang.org/)

A git checkout library for services that run code from a repository, written in Rust.

Git keeps a local checkout of a repository up to date and reads from it: which commit it is at,
what a pull brought, and a folder as it was at an older commit. It drives the `git` binary, so what
it does is what `git -C <dir> log` shows, and it keeps credentials off the command line.

One crate, `git-rs`: the `GitPort` trait a service codes against, `CliGit` that implements it,
and the functions that remove credentials written into an address before it is shown or logged.

---

## ✨ Features

- **Clone or pull in one call**: `ensure` clones a missing checkout, or fetches and resets one
- **Follows a moved repository**: the address is set from the configuration on every pull
- **Shallow by default**: only the tip is fetched; history is fetched only when it is asked for
- **What a pull brought**: the commits between two revisions and the files that changed
- **Rewrites detected**: a force push is told apart from a history too deep to reach
- **Read an older commit**: export a folder as it was, without moving the checkout
- **Credentials off the command line**: tokens and passwords as a header, SSH keys by path
- **No secrets in errors**: `user:token@` written into an address is removed from every message
- **Never prompts, never lingers**: no terminal prompt, and a dropped operation kills its git
- **Pinned by tag**: a service depends on a `vX.Y.Z` tag of this repository, never on `main`

---

## 📚 Documentation

| Document | What it covers |
|---|---|
| [Usage](docs/usage.md) | `GitPort`, `CliGit`, credentials, each operation, and redacting addresses |
| [Development](docs/development.md) | The layout, the CI gates, the tests, and working on a service against a local copy |
| [Releasing](docs/releasing.md) | How release-please cuts a version, and how a service moves to it |

---

## 🤝 Contributing

Contributions are welcome: [CONTRIBUTING.md](CONTRIBUTING.md) covers the PR workflow, commit style, CI gates and design rules.

Security issues: see [SECURITY.md](SECURITY.md) for private reporting.

---

## 👥 Authors

- [Fernando Roca](https://github.com/fernandorocagonzalez)
- [Blai Peidro](https://github.com/blaipr)

---

## ⚖️ License

[Apache 2.0](LICENSE)
