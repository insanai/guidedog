//! `guidedog pages --help`.

use super::VERSION;
use crate::GUIDEDOG_VARIABLE;
use crate::credential::TOKEN_VARIABLE;

/// The help text.
pub(crate) fn text() -> String {
    format!(
        "guidedog pages {VERSION} - publish Guidedog sites to GitHub Pages from this machine

Usage: guidedog pages COMMAND [options]

Publishing
  publish                                     do whatever is missing, then publish: writes
                                              pages.toml (from the origin remote and the
                                              nearest conf.toml), signs in, binds, sets up
                                              Pages, builds, pushes, waits until it is live
                                              (--repository, --project, --yes, --dry-run,
                                              --wait=SECONDS, --verbose, --guidedog=PATH)
  watch [--serve[=PORT]]                      publish again whenever the sources change;
                                              --serve previews the site locally too
  status [--operation=COMMIT]                 is the last publication live?
  recover                                     settle a publication whose answer was lost

Step by step (for scripts)
  init --repository=OWNER/REPO --project=DIR   write pages.toml (or --site=DIR for a site
                                              you build yourself; --branch, --profile)
  auth login [--profile=NAME]                 sign in on GitHub with a code (--client-id,
                                              --scope=public|private, --open, --replace)
  auth import [--profile=NAME]                store a token read from standard input
  auth status | auth logout [--profile=NAME]  check, or delete, the profile's token
  bind [--config=pages.toml]                  review the target and let this machine
                                              publish there (--adopt, --yes)
  setup [--config=pages.toml]                 make GitHub Pages publish the branch (--yes)
  unbind [--config=pages.toml]                stop this machine publishing there
  doctor [--config=pages.toml]                check everything, changing nothing

GitHub Actions instead
  workflow [--write] [--check]                print, write, or check
                                              .github/workflows/docs-pages.yml (--project,
                                              --branch, --version=X --sha256=Y, --offline,
                                              --release-repository, --force)

Options for every command: --credential=env (the token in {TOKEN_VARIABLE}, for this
command only), --color=auto|always|never, --diagnostics=text|json, --help, --version.

Tokens stay in the system's credential store; pages.toml holds no secret. A project is
built by the installed guidedog (--guidedog, ${GUIDEDOG_VARIABLE}, or PATH). Exit
status: 0 published or unchanged, 1 failed (nothing published), 2 usage, 6 published but
not confirmed live, 7 unknown (run recover).
"
    )
}
