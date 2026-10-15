//! `auth login`, `auth import`, `auth status`, and `auth logout`.

use super::display::Console;
use super::secret_input::read_secret;
use super::{Context, check_profile};
use crate::credential::{STORE_NAME, Secret};
use crate::device::{CLIENT_ID, start_device, wait_device};
use crate::github::account;
use crate::problem::problem;
use crate::target::remember_token;

pub(crate) fn run(ctx: &Context<'_>, console: &mut Console<'_>) -> i32 {
    if let Err(p) = check_profile(ctx.o) {
        return console.report(&p);
    }
    match ctx.o.sub.as_str() {
        "login" => login(ctx, console),
        "import" => import(ctx, console),
        "status" => status(ctx, console),
        _ => logout(ctx, console),
    }
}

/// Signs in with GitHub's device flow and stores the token under the profile.
fn login(ctx: &Context<'_>, console: &mut Console<'_>) -> i32 {
    let o = ctx.o;
    let client_id = if o.client_id.is_empty() { CLIENT_ID } else { &o.client_id };
    let client = ctx.settings.client(Secret::default(), ctx.cancel.clone());
    let scope = if o.scope == "public" { "public_repo" } else { "repo" };
    let d = match start_device(&client, client_id, scope) {
        Ok(d) => d,
        Err(p) => return console.report(&p),
    };
    console.say(&format!(
        "\n  Open {} on any device and enter the code:\n\n      {}\n",
        d.verification_uri, d.user_code
    ));
    if o.scope == "private" {
        console.say(
            "The token may read and write all your repositories (the repo scope); \
            --scope=public limits it to public ones.",
        );
    }
    if o.open {
        open_browser(&d.verification_uri);
    }
    console.say("Waiting for GitHub (Ctrl+C stops)...");
    let token = match wait_device(&client, client_id, &d) {
        Ok(t) => t,
        Err(p) => return console.report(&p),
    };
    match remember_token(ctx.settings, &o.profile, &token, "device", o.replace) {
        Ok(a) => {
            console.success(&format!(
                "Signed in as {}; the token is stored in {STORE_NAME} as \
                profile {}",
                a.login, o.profile
            ));
            0
        }
        Err(p) => console.report(&p),
    }
}

/// Stores a token read from standard input under the profile.
fn import(ctx: &Context<'_>, console: &mut Console<'_>) -> i32 {
    let o = ctx.o;
    let token = match read_secret("Paste the token (it is not shown): ") {
        Ok(t) => t,
        Err(p) => return console.report(&p),
    };
    match remember_token(ctx.settings, &o.profile, &token, "token", o.replace) {
        Ok(a) => {
            console.success(&format!(
                "Stored {}'s token in {STORE_NAME} as profile {}",
                a.login, o.profile
            ));
            0
        }
        Err(p) => console.report(&p),
    }
}

/// Says which account the profile's token is, and whether GitHub accepts it.
fn status(ctx: &Context<'_>, console: &mut Console<'_>) -> i32 {
    let profile = &ctx.o.profile;
    let found = ctx.settings.state().and_then(|s| s.find_profile(profile));
    let stored = match found {
        Ok(Some(p)) => p,
        Ok(None) => {
            return console.report(
                &problem(
                    "pages.no-token",
                    "No such profile",
                    profile,
                    &format!("The profile {profile} has no token on this machine."),
                    &format!("Run guidedog-pages auth login --profile={profile}."),
                )
                .exit(2),
            );
        }
        Err(p) => return console.report(&p),
    };
    let checked = ctx
        .settings
        .store
        .load_token(profile)
        .and_then(|token| account(&ctx.settings.client(token, ctx.cancel.clone())));
    match checked {
        Ok(a) => {
            console.success(&format!(
                "Profile {profile}: {} (signed in by {} on {}); GitHub \
                accepts the token",
                a.login, stored.method, stored.stored
            ));
            0
        }
        Err(p) => console.report(&p),
    }
}

/// Deletes the profile's token from this machine. It cannot revoke it at GitHub.
fn logout(ctx: &Context<'_>, console: &mut Console<'_>) -> i32 {
    let profile = &ctx.o.profile;
    let found = match ctx.settings.store.delete_token(profile) {
        Ok(found) => found,
        Err(p) => return console.report(&p),
    };
    if let Err(p) = ctx.settings.state().and_then(|s| s.remove_profile(profile)) {
        return console.report(&p);
    }
    if !found {
        console.success(&format!("The profile {profile} had no token on this machine"));
        return 0;
    }
    console.success(&format!("Deleted the token of profile {profile} from this machine"));
    console.say(
        "GitHub still accepts it until it is revoked: delete a token at \
        https://github.com/settings/tokens, or an app's access at \
        https://github.com/settings/applications.",
    );
    0
}

/// Opens `url` with the system's opener, as the user asked with `--open`.
fn open_browser(url: &str) {
    let mut command = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(windows) {
        let mut c = std::process::Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler");
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    // The code is on the screen too: a missing opener is no failure.
    let _ = command.arg(url).status();
}
