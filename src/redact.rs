// Credentials written into an address, kept out of what is shown and logged.
//
// `https://bot:<token>@host/x.git` clones fine, which is why people write it.
// But that address is on git's command line and git quotes it back in its own
// error messages, and both end up in logs, in API responses and in pages. The
// two functions here are applied at those exits; git itself still clones from
// the address as configured.

// "https://user:pass@host/x.git" -> "https://host/x.git". For an address shown
// back to people.
pub fn without_userinfo(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let rest = &url[scheme_end + 3..];
    let authority_end = rest.find('/').unwrap_or(rest.len());
    match rest[..authority_end].rfind('@') {
        Some(at) => format!("{}{}", &url[..scheme_end + 3], &rest[at + 1..]),
        None => url.to_string(),
    }
}

// Every address in a piece of text, without its userinfo: what a git error
// (its command line, and git's own stderr, which quotes the remote) becomes
// before it reaches a log or an API response.
pub fn redact_userinfo(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("://") {
        let (before, after) = rest.split_at(at + 3);
        out.push_str(before);
        // The authority ends where the path, the line or the quoting does.
        let end = after
            .find(|c: char| c == '/' || c.is_whitespace() || matches!(c, '\'' | '"' | '`'))
            .unwrap_or(after.len());
        let authority = &after[..end];
        match authority.rfind('@') {
            Some(i) => out.push_str(&authority[i + 1..]),
            None => out.push_str(authority),
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn userinfo_is_removed_from_an_address() {
        assert_eq!(
            without_userinfo("https://bot:s3cr3t@git.example.com/x.git"),
            "https://git.example.com/x.git"
        );
        // Only the authority's `@` counts, not one in the path
        assert_eq!(
            without_userinfo("https://edge.example/a@b"),
            "https://edge.example/a@b"
        );
        // An authority with no path at all
        assert_eq!(
            without_userinfo("https://bot@edge.example"),
            "https://edge.example"
        );
        assert_eq!(without_userinfo("git@host:x.git"), "git@host:x.git");
    }

    #[test]
    fn userinfo_is_removed_from_every_address_in_a_text() {
        let text = "git clone https://bot:s3cr3t@git.example.com/x.git /p failed: \
                    fatal: unable to access 'https://bot:s3cr3t@git.example.com/x.git/': refused";
        let clean = redact_userinfo(text);
        assert!(!clean.contains("s3cr3t"), "{clean}");
        assert!(
            clean.contains("https://git.example.com/x.git /p"),
            "{clean}"
        );
        assert!(
            clean.contains("'https://git.example.com/x.git/'"),
            "{clean}"
        );
        // Nothing to remove: unchanged.
        let plain = "fetch origin main from https://git.example.com/x.git";
        assert_eq!(redact_userinfo(plain), plain);
        assert_eq!(redact_userinfo("token@x no scheme"), "token@x no scheme");
        // An address at the very end of the text
        assert_eq!(redact_userinfo("at https://u:p@host"), "at https://host");
    }
}
