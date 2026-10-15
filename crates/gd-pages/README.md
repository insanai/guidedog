# gd-pages cheat sheet

Publishes a complete Guidedog site to GitHub Pages from a local machine (GDS 0008): one
Git push to a branch of its own, conditioned on the branch still holding the commit that
was read, then a wait until Pages deployed exactly that commit. It is the `pages`
publisher of `gd-plugins` and powers `guidedog-pages` (`cmd/guidedog-pages`). It links
the publisher SDK, Git objects (`gd-git`) and HTTP (`gd-fetch`), never the engine.
Guide: `docs/manual/howto/github-pages.rst`.

```rust
// guidedog publish --publisher=pages: put it in the registry, then the usual session.
let registry = Registry::new(vec![gd_pages::plugin()])?;
let mut s = load_publisher(&registry, gd_pages::ID, &LoadOptions::new("pages.toml"))?;
let out = gd_pages::build_dir(&Settings::default(), &read_config(path)?, dry_run)?;
// ... build the project into `out`, snapshot it, s.plan(&snap)?, s.publish(&snap, &plan)

// Tests and embedders point it elsewhere with plain data:
let settings = Settings { roots: Roots::at(&server.url("")),
    credential: Credential::Token(Secret::new(TOKEN)),
    state_dir: Some(dir.join("state")), ..Settings::default() };
let plugin = gd_pages::plugin_with(settings);
```

| Area | Items |
|---|---|
| Publisher | `Pages` (`Publisher`: load, plan, publish, status, recover), `plugin`, `plugin_with`, `Settings`, `build_dir`, `ID`, `VERSION`, `MARK` |
| Configuration | `read_config`, `render_config`, `Config`, `SiteSource::{Project, Built}`, `default_site_url`, `is_valid_{owner,repo,branch,profile}`, `CONFIG_FILE` |
| Credentials | `Secret` (zeroed on drop), `CredentialStore::{store_token, load_token, delete_token}`, `Credential::{Stored, Env, Token}`, `env_token`, `is_valid_token`, `STORE_NAME`, `TOKEN_VARIABLE`, `CREDENTIAL_VARIABLE` |
| Machine state | `State::open` + `profiles`, `find_profile`, `save_profile`, `remove_profile`, `bindings`, `find_binding`, `save_binding`, `remove_binding`, `read_journal`, `write_journal`, `drop_journal`, `read_last`, `write_last`, `lock` (`StateLock`); `Profile`, `Binding`, `Journal`, `target_key`, `today` |
| Targets | `review_target` (`Review`), `bind`, `setup`, `remember_token` |
| GitHub | `Client`, `Roots`, `account`, `read_repository`, `pages_site`, `configure_pages`, `pages_build` (`BuildState`), `commit_tree`, `remote_files`, `branch_head`, `push` (`PushOutcome`), `start_device`, `wait_device` (`DeviceCode`, `CLIENT_ID`) |
| Guided publish | `publish_guided(&GuideOptions, &mut dyn Prompt)`, `Prompt` (`show`, `confirm`, `choose`), `GuideOptions`, `plan_lines`; inference: `parse_github_remote`, `origin_url`, `find_project`, `gh_token` |
| Actions workflow | `render_workflow(&WorkflowOptions)`, `check_workflow`, `release_sha256`, `is_safe_workflow_name`, `ACTION_PINS` (reviewed commits), `WORKFLOW_PATH`, `SHA256_PLACEHOLDER` |
| Building | `freeze_site`, `guidedog_program`, `guidedog_version`, `GUIDEDOG_VARIABLE` |
| Command line | `command::{run, run_with, Program}` (`publish` is guided; `watch --serve[=PORT]` previews with `gd-serve`; `workflow [--write] [--check]`) |

Exit statuses: 0 live or unchanged, 1 failed (nothing published), 2 usage or
configuration, 4 files, network or programs, 6 accepted but not confirmed live, 7
unknown (run `recover`), 130 cancelled.

## Platforms

- Credentials: Linux and other Unix desktops use the Secret Service through the
  `secret-tool` program (libsecret-tools), with the token on its standard input and
  output, never on a command line; items keep the Odin schema's attributes
  (`xdg:schema org.guidedog.Pages`, `service`, `profile`), so existing tokens are found.
  macOS uses the Keychain (Security framework FFI, `credential/keychain.rs`); Windows the
  Credential Manager (`credential/windows.rs`). Elsewhere every store call is
  `pages.store`, and `--credential=env` is the way. There is never a plaintext fallback.
- Hidden token input: `stty -g` / `stty -echo` on Unix (see `command/secret_input.rs`
  for why not a `termios` binding), `SetConsoleMode` on Windows.

## Guided publish, workflow, preview

- `publish_guided` does each missing step once and skips what is done: `pages.toml`
  (repository from `git remote get-url origin`, confirmed unless `--repository` names
  it; project from the nearest `conf.toml`), a token, the binding (review shown, asked
  once), the Pages source (asked once; after the first push when the repository had no
  Pages site), then build, push and wait. With `interactive` false it never asks: a
  step that needs an answer fails with `pages.guide` and a hint naming the flag or step
  command. `assume_yes` (`--yes`) answers confirmations, but never signs in and never
  adopts a branch holding others' work.
- **The GitHub CLI's token** (`gh auth token`) is offered only on a terminal and is
  used for that run only, never stored: it is gh's credential, with gh's scopes, and
  gh refreshes or revokes it; a stored copy would outlive that. The device flow stores
  its token in the credential store as `auth login` does. Without a usable store only
  the gh route is offered.
- `render_workflow` reproduces the manual's workflow exactly (golden test); action
  commits live in `ACTION_PINS`, which needs a security review on every update.
  `--offline` writes a placeholder checksum that makes the workflow's own checksum test
  fail, and exits 1.
- `watch --serve` runs the `gd-serve` preview on a thread over the build folder (or the
  built site); the run's cancel stops the watch, which stops the server. The preview is
  always available: gd-serve uses gd-host without its `artifact` feature, so no Typst
  compiler or engine renderer is linked for it.

## Redesign notes (from Odin `lib/pages` and `lib/pages/command`)

- **No package globals.** Odin's `test_roots`, `credential_override`, `service`,
  `poll_unit` and the `TEST_BUILD` root variable became plain data: `Settings` (roots,
  credential, store, state and cache folders) and `Client::poll_unit`. A release still
  cannot be pointed away from GitHub by the environment; only code chooses `Roots`. The
  protocol tests run `examples/pages_fixture.rs`, which reads a fixture file beside it.
- **`State` value** replaces the free state procedures; `StateLock` releases its lock on
  drop (`File::try_lock`), and `Secret` zeroes itself on drop instead of `forget`.
- **Types instead of sentinels.** `SiteSource` instead of two strings of which one must
  be empty; `Option` for a missing branch head, adopted commit and observed commit;
  `BuildState` instead of `found`/`readable` flags; `PushOutcome` carries its problem.
- **Linux store through `secret-tool`** instead of `dlopen`ing libsecret and GLib:
  no unsafe code, the same items.
- **Cancellation** is the SDK's `Cancel` probe; a watcher thread joins it to
  `gd-fetch`'s flag for the length of each transfer.
- **Reports** use `gd-report` (Elm style, `Hint:`), text or JSON lines, instead of the
  custom "Try:" layout.
- **Arenas, `release_*` and memory-refusal checks** are gone with Odin's allocators.
- **Not wired yet:** `gd-host`'s crash reporter in `guidedog-pages` (gd-host is now
  linked for the preview, so it could be). Tokens never appear on a command line: `gd-fetch`
  hands curl its headers in a private file (`--header @file`, mode 0600, removed after
  the request).
