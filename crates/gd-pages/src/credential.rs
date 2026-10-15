//! Credentials: GitHub tokens, kept in the system's credential store under a profile.
//!
//! The store is the macOS Keychain, the Windows Credential Manager, or a Linux (or other
//! Unix) desktop's Secret Service, through `secret-tool`. A token never enters a
//! project, a configuration file, a journal, or a log, and a store that is missing or
//! locked is a problem, never a reason to write the token to a plain file.
//! [`TOKEN_VARIABLE`] gives a token to one process instead, without storing it.

use gd_plugins::Problem;

use crate::problem::{problem, token_problem};

#[cfg(target_os = "macos")]
#[path = "credential/keychain.rs"]
mod platform;
#[cfg(all(unix, not(target_os = "macos")))]
#[path = "credential/secret_service.rs"]
mod platform;
#[cfg(not(any(unix, windows)))]
#[path = "credential/unsupported.rs"]
mod platform;
#[cfg(windows)]
#[path = "credential/windows.rs"]
mod platform;

/// The name tokens are stored under, with the profile as the account.
pub const SERVICE: &str = "guidedog-pages";
/// Holds a token for one process, read only when the run chooses the environment.
pub const TOKEN_VARIABLE: &str = "GUIDEDOG_PAGES_TOKEN";
/// Set to `env`, chooses [`TOKEN_VARIABLE`] for runs the command line cannot reach,
/// such as `guidedog publish`: the user's explicit choice, as `--credential=env`.
pub const CREDENTIAL_VARIABLE: &str = "GUIDEDOG_PAGES_CREDENTIAL";
/// The profile name that means [`TOKEN_VARIABLE`].
pub const ENV_PROFILE: &str = "env";
/// The longest token accepted, in bytes.
pub const MAX_TOKEN: usize = 1024;
/// The credential store's name, for messages.
pub const STORE_NAME: &str = platform::STORE_NAME;

/// A token: text of its own that is overwritten with zeros when dropped. `Debug` never
/// shows it.
#[derive(Clone, Default)]
pub struct Secret {
    text: String,
}

impl Secret {
    /// Copies `text` into a new secret.
    pub fn new(text: &str) -> Secret {
        Secret { text: text.to_string() }
    }

    /// Takes `text` as a secret, without copying it.
    pub(crate) fn from_string(text: String) -> Secret {
        Secret { text }
    }

    /// Borrows the secret's text.
    pub fn expose(&self) -> &str {
        &self.text
    }

    /// Whether the secret is empty (no token).
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        forget_string(std::mem::take(&mut self.text));
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(<hidden>)")
    }
}

/// Overwrites `text` with zeros before its memory is released.
pub(crate) fn forget_string(text: String) {
    forget_bytes(text.into_bytes());
}

/// Overwrites `bytes` with zeros before their memory is released.
pub(crate) fn forget_bytes(mut bytes: Vec<u8>) {
    bytes.fill(0);
    // Keeps the zeroing from being optimized away as a dead store.
    std::hint::black_box(&bytes);
}

/// Whether `text` looks like a GitHub token: 20 to [`MAX_TOKEN`] letters, digits, and
/// underscores. It does not tell whether GitHub accepts it.
pub fn is_valid_token(text: &str) -> bool {
    (20..=MAX_TOKEN).contains(&text.len())
        && text.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

/// Where a run's token comes from.
#[derive(Debug, Clone, Default)]
pub enum Credential {
    /// The profile's token in the credential store, unless [`CREDENTIAL_VARIABLE`] is
    /// `env`.
    #[default]
    Stored,
    /// [`TOKEN_VARIABLE`], as `--credential=env` chooses.
    Env,
    /// This token, for embedders and tests.
    Token(Secret),
}

impl Credential {
    /// Whether the run uses a token that is not a stored profile's.
    pub fn is_unstored(&self) -> bool {
        match self {
            Credential::Stored => {
                std::env::var(CREDENTIAL_VARIABLE).is_ok_and(|v| v == ENV_PROFILE)
            }
            Credential::Env | Credential::Token(_) => true,
        }
    }
}

/// The system's credential store, under one service name. Tests use another service,
/// so that they never touch a user's tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialStore {
    /// The service tokens are stored under ([`SERVICE`] by default).
    pub service: String,
}

impl Default for CredentialStore {
    fn default() -> CredentialStore {
        CredentialStore { service: SERVICE.to_string() }
    }
}

impl CredentialStore {
    /// Stores `token` under `profile`, replacing the profile's token. Fails with
    /// `pages.token` for text that is not a token and `pages.store` when the store
    /// cannot be used.
    pub fn store_token(&self, profile: &str, token: &Secret) -> Result<(), Problem> {
        if !is_valid_token(token.expose()) {
            return Err(token_problem());
        }
        platform::store(&self.service, profile, token.expose())
    }

    /// Reads the token of `profile`. Fails with `pages.no-token` when there is none and
    /// `pages.store` when the store cannot be used.
    pub fn load_token(&self, profile: &str) -> Result<Secret, Problem> {
        platform::load(&self.service, profile)
    }

    /// Removes the token of `profile`; `false` when it had none. It does not revoke the
    /// token at GitHub.
    pub fn delete_token(&self, profile: &str) -> Result<bool, Problem> {
        platform::delete(&self.service, profile)
    }
}

/// Reads the token in [`TOKEN_VARIABLE`]. Fails with `pages.credential` when it is not
/// set and `pages.token` when it is not a token.
pub fn env_token() -> Result<Secret, Problem> {
    let value = std::env::var(TOKEN_VARIABLE).unwrap_or_default();
    let token = Secret::new(value.trim());
    forget_string(value);
    if token.is_empty() {
        return Err(problem(
            "pages.credential",
            "No token in the environment",
            TOKEN_VARIABLE,
            &format!("{TOKEN_VARIABLE} is not set."),
            &format!(
                "Set {TOKEN_VARIABLE} for this command, or use a stored profile instead of \
                 --credential=env."
            ),
        )
        .exit(2));
    }
    if !is_valid_token(token.expose()) {
        return Err(token_problem());
    }
    Ok(token)
}

/// `pages.store` (exit 4): the credential store cannot be used.
#[cfg_attr(not(any(unix, windows)), allow(dead_code))]
pub(crate) fn store_problem(what: &str, detail: &str) -> Problem {
    problem(
        "pages.store",
        "The credential store cannot be used",
        STORE_NAME,
        &format!("{what}{detail}"),
        &format!(
            "Unlock or start {STORE_NAME}, or give a token to one command with \
             {TOKEN_VARIABLE} and --credential=env."
        ),
    )
    .exit(4)
}

/// `pages.no-token` (exit 2): the profile has no stored token.
#[cfg_attr(not(any(unix, windows)), allow(dead_code))]
pub(crate) fn missing_token(profile: &str) -> Problem {
    problem(
        "pages.no-token",
        "No token is stored for this profile",
        profile,
        &format!("The profile {profile} has no token in {STORE_NAME}."),
        &format!(
            "Run guidedog-pages auth login --profile={profile}, or guidedog-pages auth import \
             --profile={profile} to store one."
        ),
    )
    .exit(2)
}

/// A token the store returned: one that is too long or not text is refused.
#[cfg_attr(not(any(unix, windows)), allow(dead_code))]
pub(crate) fn stored_secret(bytes: &[u8]) -> Result<Secret, Problem> {
    if bytes.is_empty() || bytes.len() > MAX_TOKEN {
        return Err(token_problem());
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => Ok(Secret::new(text)),
        Err(_) => Err(token_problem()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "ghp_testtokentesttokentesttoken0001";

    #[test]
    fn tokens_are_one_line_of_word_characters() {
        assert!(is_valid_token(TOKEN));
        assert!(!is_valid_token("short"));
        assert!(!is_valid_token("not a token at all, not at all"));
        assert!(!is_valid_token(&"a".repeat(MAX_TOKEN + 1)));
        assert_eq!(format!("{:?}", Secret::new(TOKEN)), "Secret(<hidden>)");
        assert_eq!(stored_secret(TOKEN.as_bytes()).unwrap().expose(), TOKEN);
        assert_eq!(stored_secret(b"").unwrap_err().code, "pages.token");
    }

    /// The system's credential store keeps, returns, and deletes a token. Where it
    /// cannot be used (no desktop session, no secret-tool), the problem names it, and
    /// nothing is written anywhere else.
    #[test]
    fn the_credential_store_keeps_tokens() {
        let store = CredentialStore { service: "guidedog-pages-test".into() };
        let token = Secret::new(TOKEN);
        if let Err(p) = store.store_token("test-profile", &token) {
            assert_eq!(p.code, "pages.store", "{p}");
            let required = std::env::var_os("GUIDEDOG_TEST_REQUIRE_STORE").is_some();
            assert!(cfg!(all(unix, not(target_os = "macos"))) && !required, "{p}");
            return;
        }
        assert_eq!(store.load_token("test-profile").unwrap().expose(), TOKEN);
        assert!(store.delete_token("test-profile").unwrap());
        assert_eq!(store.load_token("test-profile").unwrap_err().code, "pages.no-token");
        assert!(!store.delete_token("test-profile").unwrap());
        let invalid = Secret::new("not a token");
        assert_eq!(store.store_token("test-profile", &invalid).unwrap_err().code, "pages.token");
    }
}
