//! Which proxy a URL goes through, from the environment, by the rules curl uses.
//!
//! An http URL reads `http_proxy` (only in lowercase: CGI programs receive a request's
//! `Proxy:` header as `HTTP_PROXY`), an https URL `https_proxy` or `HTTPS_PROXY`; either
//! falls back to `all_proxy` or `ALL_PROXY`. `no_proxy` or `NO_PROXY` lists hosts that
//! go direct: `*` for all, or names that match a host or end its domain.

/// A proxy setting found in the environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Found {
    /// The variable that held it, such as `HTTPS_PROXY`.
    pub variable: &'static str,
    /// Its value, such as `http://proxy.example:3128`.
    pub value: String,
}

/// The proxy for `url`, reading variables through `var`; `None` to go direct.
pub(crate) fn proxy_for(url: &str, var: impl Fn(&str) -> Option<String>) -> Option<Found> {
    let set = |name: &'static str| var(name).filter(|v| !v.trim().is_empty());
    let host = host_of(url);
    let excluded = ["no_proxy", "NO_PROXY"].into_iter().find_map(set);
    if excluded.is_some_and(|list| is_excluded(host, &list)) {
        return None;
    }
    let names: &[&'static str] = if url.starts_with("https://") {
        &["https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"]
    } else {
        &["http_proxy", "all_proxy", "ALL_PROXY"]
    };
    names.iter().find_map(|&variable| set(variable).map(|value| Found { variable, value }))
}

/// The host of an http or https URL, without user, port or IPv6 brackets.
fn host_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    if let Some(bracketed) = host_port.strip_prefix('[') {
        return bracketed.split(']').next().unwrap_or("");
    }
    host_port.split(':').next().unwrap_or("")
}

/// Whether `host` matches an entry of the comma-separated `list`.
fn is_excluded(host: &str, list: &str) -> bool {
    let host = host.trim_end_matches('.');
    list.split([',', ' ']).map(str::trim).filter(|e| !e.is_empty()).any(|entry| {
        if entry == "*" {
            return true;
        }
        let entry = entry.trim_start_matches('.').trim_end_matches('.');
        let Some(prefix_len) = host.len().checked_sub(entry.len()) else {
            return false;
        };
        let tail_matches = host.get(prefix_len..).is_some_and(|t| t.eq_ignore_ascii_case(entry));
        tail_matches && (prefix_len == 0 || host.as_bytes()[prefix_len - 1] == b'.')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&str, &str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| pairs.iter().find(|(n, _)| *n == name).map(|(_, v)| (*v).to_owned())
    }

    #[test]
    fn hosts_are_taken_from_urls() {
        assert_eq!(host_of("https://user:pw@Docs.Example:8443/x?y"), "Docs.Example");
        assert_eq!(host_of("http://[::1]:80/"), "::1");
        assert_eq!(host_of("http://example.com"), "example.com");
    }

    #[test]
    fn schemes_pick_their_variables() {
        let vars = [("HTTPS_PROXY", "http://p:1"), ("HTTP_PROXY", "http://bad:2")];
        let found = proxy_for("https://x.test/", env(&vars));
        assert_eq!(found.map(|f| f.variable), Some("HTTPS_PROXY"));
        // Uppercase HTTP_PROXY is ignored, as curl ignores it.
        assert_eq!(proxy_for("http://x.test/", env(&vars)), None);
        let all = [("ALL_PROXY", "p:3")];
        assert_eq!(proxy_for("http://x.test/", env(&all)).map(|f| f.value), Some("p:3".into()));
    }

    #[test]
    fn no_proxy_lists_hosts_and_domains() {
        assert!(is_excluded("docs.python.org", "localhost, .python.org"));
        assert!(is_excluded("python.org", "python.org"));
        assert!(!is_excluded("notpython.org", "python.org"));
        assert!(is_excluded("anything", "*"));
        assert!(!is_excluded("example.com", ""));
        let vars = [("https_proxy", "p:1"), ("NO_PROXY", "x.test")];
        assert_eq!(proxy_for("https://a.x.test/", env(&vars)), None);
    }
}
