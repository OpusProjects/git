# Security Policy

## Supported versions

Only the latest release line receives security fixes.

| Version | Supported |
|---|---|
| 0.x (latest release) | Yes |
| older tags | No |

## Reporting a vulnerability

Please **do not open a public issue** for security problems.

Use GitHub's private vulnerability reporting instead:
[Report a vulnerability](https://github.com/OpusProjects/git/security/advisories/new)
— it opens a private thread with the maintainers.

Include what you can: affected module, the service and version you saw
it in, reproduction steps, and impact. You should hear back within a week. Once
a fix ships, the advisory is published and credited unless you prefer otherwise.

## Scope notes

- This repository is the git code the services that pin it share: a vulnerability here is one in
  each of them. A fix is released here first, then each service moves to the new tag.
- **Credentials never reach a command line.** A token or a username and password travel as an
  `http.extraHeader` set through `GIT_CONFIG_*` environment variables, and an SSH key path through
  `GIT_SSH_COMMAND`, because a process's arguments are readable by every user in `/proc`.
- **Credentials written into an address** (`https://user:token@host/...`) are removed from every
  error before it is returned, so they do not reach a log or an API response. The address itself
  is still on git's command line while it runs: prefer a credential over writing one into the
  address.
- **Git never prompts.** `GIT_TERMINAL_PROMPT=0` is set on every run: a missing credential fails
  instead of hanging on a prompt nobody will answer.
- **A commit id is checked before it reaches a command line**: four to sixty-four hexadecimal
  characters, so a value from a request cannot become an option or a ref expression.
- Dropping a running operation kills its git process, so a caller's timeout really ends it.
