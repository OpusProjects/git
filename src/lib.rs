// unified-git — local checkouts of a git repository, kept up to date.
//
// A service that runs scripts or reads files from a repository needs the same
// few things: clone it, pull it, know which commit it is at, see what a pull
// brought, and read a folder as it was at an older commit. This crate holds
// them:
//
// - `port`: GitPort, the interface a service codes against, and the types it
//   speaks (Remote, Commit, CommitRange, GitError)
// - `cli`: CliGit, the GitPort that runs the `git` binary
// - `redact`: removing credentials written into an address from a URL or an
//   error message, before either is shown or logged
//
// Credentials come in already resolved (a map of field names to values), so
// where they are stored is the service's business, not this crate's.

pub mod cli;
pub mod port;
pub mod redact;

pub use cli::CliGit;
pub use port::{
    Commit, CommitRange, CommitsFuture, ExportFuture, GitError, GitFuture, GitPort, Remote,
    RevisionFuture,
};
pub use redact::{redact_userinfo, without_userinfo};
