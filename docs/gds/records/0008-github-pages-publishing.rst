:gds-number: 0008
:gds-title: Publish documentation to GitHub Pages
:gds-state: committed
:gds-type: Standards Track
:gds-authors: ["Vikrant Rathore"]
:gds-created: 2026-10-12
:gds-updated: 2026-10-15
:gds-discussion: Local review; no external discussion URL assigned
:gds-labels: ["cli","build","security","documentation"]

Publish documentation to GitHub Pages
=====================================

Abstract
--------

``guidedog-pages`` publishes a Guidedog site to GitHub Pages from a local machine. It
builds the project with the installed ``guidedog``, freezes the result, and sends the
whole site to a branch of its own in one Git push. GitHub applies the push only if the
branch still holds the commit the publisher read. The publisher then waits for GitHub Pages to
deploy that commit, and calls the site live only when GitHub says so. Tokens stay in
the system's credential store. A machine-owned binding decides which repository and
branch a profile may publish to. ``guidedog-pages watch`` publishes again when the
sources change. The same program is the ``pages`` publisher of GDS 0009, which
``guidedog publish`` runs through the process protocol.

GitHub Actions remains the second, independent route: the manual's workflow builds
with a pinned Guidedog release and deploys with the job's short-lived token. Guidedog
0.6.0 ships both. The review of the first draft (below) replaced its transport, its
build boundary, and its snapshot mechanism; the rest of its safety design stands.

Motivation and scope
--------------------

A static site is useful only when readers can reach it. A project should be able to
choose a repository, authorize one machine, and publish the same files it previewed,
without keeping a maintainer's personal token in GitHub Actions. Teams that publish
after every push keep the Actions route.

This record owns publication, credentials, remote status, and automatic updates. It
reuses the project engine, templates, translations, diagnostics, and memory policy.
It adds no Python and does not change document semantics. It does not provide a
hosting service, a plugin marketplace, arbitrary build hooks, or a credential broker
on Guidedog's servers.

GitHub.com is the only supported host. GitHub Enterprise Server needs separate
qualification and host binding; a different API URL is not proof of support. DNS,
custom domains, and repository creation remain outside the publisher. A test build
alone (``-define:GUIDEDOG_PAGES_TEST_BUILD=true``) can be pointed at another server,
for the tests; a release cannot send its token anywhere but GitHub.

Review of the first draft
-------------------------

The first draft (2026-10-12) was reviewed before implementation. Its route split,
its refusal to force a branch, its journal, its credential rules, and its outcome
states were sound and are kept. Four parts would not have worked as written:

.. list-table:: Findings and decisions
   :header-rows: 1
   :widths: 30 35 35

   * - Draft
     - Finding
     - Decision
   * - Upload with the Git Database API: one request per blob and tree, then a
       non-forced ref update.
     - GitHub limits content-creating requests to 80 a minute and 500 an hour
       (secondary rate limits). A first publication of a few hundred pages and their
       assets would take hours; base64 JSON adds a third to every byte. The draft
       itself admits the ref update is not a compare-and-swap.
     - Push with Git's smart HTTP protocol: one ``git-receive-pack`` request carries a
       pack of every object the branch lacks and the update from the observed commit
       to the new one. receive-pack applies an update only while the reference still
       has its old ID: a true compare-and-swap. ``lib/git`` writes the objects and
       messages; no git program runs.
   * - A "thin embedded renderer" links the project engine into ``guidedog-pages``.
     - A second full engine (Typst, Graphviz, tree-sitter) to build, qualify, and ship,
       unlike ``dsc``, which builds with the installed ``guidedog``.
     - ``guidedog-pages`` builds with the installed ``guidedog``, as ``dsc`` does.
       ``guidedog publish`` builds with its own engine and gives the frozen site to the
       publisher. One engine; the publisher links none.
   * - Freeze the output "under the build coordinator's output lock".
     - The project coordinator has no output lock: a build publishes by switching
       folders, and adding a cross-process lock to every build is out of proportion.
     - Build into a folder only this publisher writes (its target's cache folder), under
       the target's lock, and freeze it there; a site built elsewhere (``site =``) is
       copied. ``publish`` re-reads every file before it writes, and packing checks
       each file's Git ID again.
   * - Device login "out of the box".
     - The device flow needs a registered GitHub App or OAuth App's client ID, a
       maintainer decision the draft left open.
     - The client ID is a build setting (``-define:GUIDEDOG_PAGES_CLIENT_ID``) or
       ``--client-id``. 0.6.0 ships without one; ``auth import`` stores a fine-grained
       token, and ``auth login --client-id`` works with an app the user registered.

Two smaller corrections: status is correlated through the Pages builds API, which
lists each build's commit (a deployment of a branch has no Actions OIDC deployment to
read); and publication commits carry a trailer, ``Guidedog-Publication:
GENERATION``, by which the publisher tells its commits from others on the branch.

Specification
-------------

Two publication routes
~~~~~~~~~~~~~~~~~~~~~~

GitHub's Pages deployment endpoint requires an Actions-issued ``oidc_token`` and an
artifact of the repository. A local token supplies neither. The local route therefore
publishes a branch, and Pages deploys the branch.

.. list-table:: Publication routes
   :header-rows: 1
   :widths: 23 37 40

   * - Route
     - Local publisher
     - GitHub Actions
   * - Build
     - The installed ``guidedog`` (or ``guidedog publish``'s own engine)
     - A pinned Guidedog release
   * - Transfer
     - One smart-HTTP push of a pack and the branch update
     - Official Pages artifact action
   * - Authorization
     - A stored token (fine-grained, or device flow) and a machine binding
     - Job token and Actions OIDC identity
   * - Pages source
     - Dedicated branch, root folder
     - GitHub Actions
   * - Automatic update
     - ``guidedog-pages watch``, or a machine scheduler
     - Push to the chosen source branch

A repository has one Pages publishing source. ``publish`` reads it and refuses a site
deployed by a workflow or from another branch or folder; ``setup`` changes it, after
asking. The publisher never silently replaces an Actions configuration.

.. graphviz::
   :caption: Both routes publish a successful Guidedog generation.
   :alt: Local builds publish through a branch push; Actions builds publish through an artifact.

   digraph routes {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f"];
     edge [color="#216553"];
     sources [label="RST / Markdown\nconfiguration, templates, translations"];
     local [label="Installed guidedog\nprivate build folder, frozen"];
     ci [label="Actions + Guidedog release\nchecked generation"];
     branch [label="One receive-pack push\npack + update old -> new"];
     artifact [label="Pages artifact\njob token + OIDC"];
     pages [label="GitHub Pages\nbuild of that commit confirmed"];
     sources -> local -> branch -> pages;
     sources -> ci -> artifact -> pages;
   }

Programs and libraries
~~~~~~~~~~~~~~~~~~~~~~

.. list-table:: Where the publisher lives
   :header-rows: 1
   :widths: 30 70

   * - Part
     - What it does
   * - ``lib/git``
     - Git object IDs, trees from a list of files, commits, packs (zlib), and the
       receive-pack advertisement, request, and report. No networking, no git program.
   * - ``lib/fetch``
     - ``send``: any HTTP method, headers, a body in memory or streamed from a file,
       bounded answers, cancellation, and no redirects (an ``Authorization`` header
       reaches only the URL it was meant for). libcurl, and WinHTTP on Windows.
   * - ``lib/pages``
     - The ``pages`` publisher of ``lib/plugins``: configuration, credential stores,
       bindings, journals, locks, the GitHub client, the device flow, and the
       publication itself.
   * - ``guidedog-pages``
     - The command (``lib/pages/command``); started with ``--guidedog-protocol``, it
       serves GDS 0009's process protocol instead.
   * - ``guidedog publish``
     - Builds with the engine, freezes, and drives the registered ``pages`` program.

``lib/pages`` imports ``lib/fetch``, ``lib/git``, ``lib/plugins``, and ``lib/toml``,
and nothing of the document engine. Registration is explicit: ``guidedog plugin
register pages PATH``. Neither a project nor a working directory selects code, and a
normal build never starts a publisher.

Configuration and binding
~~~~~~~~~~~~~~~~~~~~~~~~~

``pages.toml`` records non-secret settings; ``guidedog-pages init`` writes it.
Unknown settings and invalid values are errors with their line.

.. code-block:: toml

   schema = 1
   repository = "example/project"
   branch = "gh-pages"
   project = "docs"          # or: site = "build/site"
   strict = true
   profile = "default"
   site_url = "https://example.github.io/project/"
   wait_seconds = 300

   [watch]
   debounce_ms = 1000
   max_wait_ms = 10000

Paths are relative to the file. ``site_url`` defaults to where GitHub serves the
repository, and is the build's ``html_baseurl``. ``profile`` only names a stored
token; the profile ``env`` is never accepted here (see credentials).

A *binding*, kept in the machine's state folder (``plugins.config_dir()/pages``,
readable only by the user), authorizes a profile for one repository, by its GitHub
ID, and one branch, for one account ID. ``bind`` creates it after asking. Before it asks, it shows the account,
the repository and its ID, its visibility, the branch's state, and the Pages
settings. Publication refuses the following, before it moves anything: a target with no
binding; a profile other than the bound one; a token of another account; a repository
whose ID is not the bound one (deleted and made again, or another took its name); a
renamed or transferred repository, including an API redirect; and the repository's
default branch. A project edit can name a target, but never authorize one.

A branch that holds commits without the publication trailer is someone else's work.
``bind`` refuses it unless ``--adopt`` is given, after which the binding records the
adopted commit and the next publication replaces the branch's files.

Commands
~~~~~~~~

.. code-block:: sh

   guidedog-pages init --project=docs --repository=example/project
   guidedog-pages auth import --profile=default
   guidedog-pages auth login --client-id=APP_ID --scope=public
   guidedog-pages auth status
   guidedog-pages bind [--adopt] [--yes]
   guidedog-pages setup [--yes]
   guidedog-pages doctor
   guidedog-pages publish [--dry-run] [--wait=SECONDS] [--verbose]
   guidedog-pages watch
   guidedog-pages status [--operation=COMMIT]
   guidedog-pages recover
   guidedog-pages unbind
   guidedog-pages auth logout

``init`` writes configuration and changes nothing at GitHub. ``setup`` points Pages at
the branch's root, creating the Pages site when there is none. It needs the Pages and
Administration permissions; manual setup in the repository's settings works too.
GitHub configures Pages only for a branch that exists. So a target without a Pages site
is still published to, and the publication ends ``Accepted`` with ``pages.no-site``. Run
``setup`` then. (GitHub turns Pages on by itself when a ``gh-pages`` branch first
arrives; the qualification below saw it do so.) ``setup`` refuses a branch that does not
exist yet, and says to publish first.
``doctor`` checks the configuration, the builder, the token, the account, the
repository and push permission, the binding, and the Pages source, and writes nothing.
``--dry-run`` builds and reads the target, and writes nothing: no object, no ref, no
setting, no credential.

Credentials
~~~~~~~~~~~

Tokens are stored under the service ``guidedog-pages`` with the profile as account: in
the macOS Keychain (Security framework items), the Windows Credential Manager
(generic credentials), or the Secret Service of a Linux desktop (libsecret, loaded
when first needed, so the program runs where it is absent). ``profiles.toml`` records
each profile's account login, ID, method, and date, never the token. A missing or
locked store is a problem naming the store; there is no plaintext fallback.

``auth import`` reads a token from a hidden terminal prompt or from standard input,
never from an argument. ``auth login`` runs the device flow. It shows GitHub's
verification address (only an address on github.com is shown) and code, opens a
browser only with ``--open``, polls at the interval GitHub asks for, slows down on
``slow_down``, and stops on expiry, denial, or Ctrl+C. Both check the token with
GitHub and refuse to give a profile to another account without ``--replace``.
``--scope=public`` asks an OAuth App for ``public_repo`` instead of ``repo``.

``GUIDEDOG_PAGES_TOKEN`` gives a token to one process, never stored. It is used only
when chosen explicitly: ``--credential=env``, or ``GUIDEDOG_PAGES_CREDENTIAL=env`` for
runs the command line cannot reach, such as ``guidedog publish``. An unattended run
never starts a device login. ``auth logout`` deletes the local token and says how to
revoke it at GitHub; local deletion is not revocation. Token bytes the publisher
owns are cleared when it is done with them; copies a transport library makes are
outside that promise.

.. list-table:: Permissions by operation
   :header-rows: 1
   :widths: 30 35 35

   * - Operation
     - Fine-grained token / GitHub App
     - OAuth App token
   * - Read the account, repository, and branch; push the branch
     - Metadata: read; Contents: read and write
     - ``public_repo`` (public repositories) or ``repo``
   * - Read the Pages settings and builds (confirmation)
     - Pages: read
     - ``repo``; without it, publication ends as ``Accepted``
   * - ``setup``: create or change the Pages source
     - Pages: write and Administration: write
     - ``repo`` and the repository's admin role

Organization policies (SSO, app approval, rulesets) are reported as GitHub states
them: a refused permission names the permission to grant, and a ruleset refusal of the
push names the ruleset as the cause.

Publication
~~~~~~~~~~~

A publication runs under the target's lock, held by one process at a time on the
machine (``KEY.lock``, a kernel lock); another machine is held off by the compare-and-
swap.

#. Build into the target's private folder (or copy ``site``), and freeze it: every
   regular file, path, size, and SHA-256 (GDS 0009's snapshot). Links, special files,
   paths Git or a static host cannot serve, case collisions, a missing
   ``index.html``, and sites over 1 GiB, 100,000 files, or a file of 100 MiB are
   refused before anything is sent.
#. Refuse while a journal of the target is unsettled. Read the Pages source; read
   the branch's commit ``H`` from the receive-pack advertisement; read ``H``'s tree and
   message, and refuse a commit without the trailer that was not adopted; read ``H``'s
   files, recursively, with their blob IDs.
#. Take each file's Git blob ID, add an empty ``.nojekyll`` (so ``_static`` is
   served), and build the trees. When the root tree equals ``H``'s, the target is
   ``Unchanged``: nothing is written.
#. Make commit ``C``: the new tree, parent ``H``, by the account at its private
   ``ID+login@users.noreply.github.com`` address, at ``SOURCE_DATE_EPOCH`` or now, with
   the trailer ``Guidedog-Publication: GENERATION``.
#. Write a pack of the blobs the branch lacks (re-reading and checking each file),
   every tree, and ``C``, then the request ``H -> C`` for ``refs/heads/BRANCH``, to the
   target's cache folder.
#. Write the journal (repository and ID, branch, ``H``, ``C``, generation; no secret).
#. Send the request. A report of ``ok`` is acceptance. ``ng`` is refusal, and the
   journal is dropped: a refusal changes nothing. If the branch no longer has ``H``,
   the problem is a conflict: another publication came first. A lost answer is
   settled by reading the branch again: ``C`` is acceptance, ``H`` means nothing was
   published, anything else is a conflict. When the branch cannot be read, the
   outcome is ``Unknown`` and the journal stays for ``recover``.
#. On acceptance, record ``C`` as the target's last publication and drop the journal.
   Poll the Pages builds for ``C`` until ``wait_seconds``: ``built`` is ``Live``,
   ``errored`` is ``Failed`` (the branch has the site, the previous deployment stays),
   a queued or running build is ``Pending``, and none, or builds the token may not
   read, leave it ``Accepted``.

The update of the branch is the publication's boundary. Objects GitHub received for a
refused update are unreferenced; readers of the branch see the old tree or the new one.
This does not claim that every CDN edge changes at once.

.. graphviz::
   :caption: The branch moves only in the push, and only from the commit read.
   :alt: Build and freeze, plan against the branch, pack, journal, push old to new, confirm the Pages build.

   digraph transaction {
     graph [rankdir=TB, bgcolor="transparent", pad="0.3"];
     node [shape=box, style="rounded,filled", fillcolor="#edf5f2",
           color="#216553", fontname="Helvetica", fontcolor="#16382f"];
     edge [color="#216553"];
     build [label="Strict build + freeze"];
     plan [label="Branch commit H + its files\nnew tree; unchanged?"];
     pack [label="Pack: missing blobs, trees, commit C"];
     journal [label="Journal H -> C"];
     push [label="receive-pack: H -> C\nonly if the branch has H"];
     wait [label="Pages build of C\nLive / Pending / Failed"];
     refused [label="Refused: nothing changed\nconflict if the branch moved", fillcolor="#fff3e5"];
     lost [label="Answer lost: read the branch\nC, H, or another", fillcolor="#fff3e5"];
     build -> plan -> pack -> journal -> push -> wait;
     push -> refused [style=dashed];
     push -> lost [style=dashed];
   }

Outcomes
~~~~~~~~

``publish``, ``status``, and ``recover`` end in one of GDS 0009's states, and exit
with: 0 for ``Live`` and ``Unchanged`` (and a dry run); 1 for ``Failed`` (nothing
published, or the deployment failed); 2 for a usage or configuration problem; 3 when
memory ran out; 4 for files or the network; 6 for ``Accepted`` and ``Pending`` (the
branch moved, deployment not confirmed: run ``status`` later); and 7 for ``Unknown``
(run ``recover``). Problems name their code (``pages.conflict``, ``pages.ruleset``,
``pages.credential``, ``pages.unsettled``, ...) and the next step. JSON diagnostics
carry no response body, header, or credential.

Watching and recovery
~~~~~~~~~~~~~~~~~~~~~

``watch`` is in the foreground and opt-in. It publishes once, then stamps the
source folder (paths, sizes, and times of its files, without build output, caches,
and hidden folders such as ``.git``) every half second, and publishes once the
sources have been quiet for ``debounce_ms``, or ``max_wait_ms`` after the first
change. Each round has an arena of its own, so a long watch does not grow. A failed
build or publication is reported, and the site stays as it was. A refused or missing
token pauses publication without opening a browser. Network failures are retried
after 2, 4, 8, 16, and 32 seconds. Ctrl+C stops it between steps; a push already sent
is settled as above. It watches this machine's files. Publication after a push to
GitHub belongs to the Actions route. The manual gives launchd, systemd, and Task Scheduler
examples.

``recover`` reads the journal and the branch, and settles as a lost answer is settled.
It never moves the branch back. Without a journal it reports the last publication's
deployment. A damaged journal is a problem with directions, not a reason to touch the
branch. A future rollback would publish an older tree as a new commit.

Rationale and alternatives
--------------------------

The smart-HTTP push is what ``git push`` does, so its limits are Git's: one request,
compressed objects, and only those the branch lacks. Its update is a true
compare-and-swap, which the Git Database API's fast-forward check is not. It needs no
git program, no shell, and no credential helper; the token goes in an
``Authorization`` header of two requests to github.com. A future transport must keep
the same transaction and credential contracts.

Building with the installed ``guidedog`` keeps one engine and one qualification, as
``dsc`` does; ``guidedog publish`` offers the in-process build to those who prefer it.
A private build folder freezes a generation without a lock in the build coordinator;
a copy covers sites built elsewhere.

For teams, the official Actions route offers environment protections, short-lived
authorization, and updates on push. A personal token in repository secrets would
broaden access unnecessarily.

Security and operational considerations
---------------------------------------

Project sources, templates, paths, API answers, and remote file names are untrusted
input. Answers are bounded before they are parsed (4 MiB, 64 MiB for a tree listing,
16 MiB for Git messages), JSON nesting is limited, duplicate keys are refused, and text
from GitHub is escaped before it reaches a terminal. TLS verification is never turned
off. Requests that carry a token never follow a redirect. Only GitHub.com is reached,
except by a test build. The device flow's address is shown only when it is GitHub's.

State files are written beside their place, synced, and renamed over it, with
owner-only permissions. Tokens are never written to a file, an argument, a journal,
a log, or a crash report. The publisher runs no Python, repository hook, shell
command, or Git credential helper.

Publishing a private repository's site does not make the site private; ``bind`` says
so, with the repository's visibility. The publisher publishes the frozen build output
only, never the repository root.

Backwards compatibility
-----------------------

Guidedog's existing commands, ``conf.toml``, rendering, catalogs, and templates are
unchanged. Without ``guidedog-pages`` or ``guidedog publish``, no GitHub request or
authentication happens. The Actions workflow keeps working. ``pages.toml``, the
state files, and the journal have their own schema versions.

Acceptance criteria and implementation status
---------------------------------------------

Guidedog 0.6.0 ships ``guidedog-pages`` in the ``guidedog`` archives and the Homebrew
formula, beside ``guidedog`` and ``dsc``.

.. list-table:: Criteria and evidence
   :header-rows: 1
   :widths: 46 54

   * - Criterion
     - Status
   * - Native Odin, explicit ownership, checked bounds and allocations, bounded HTTP,
       cancellation, no Python.
     - Done. ``lib/plugins`` refuses each allocation of a publication in turn; the
       suites run under AddressSanitizer on Linux and macOS; ``tests/no_python_test``.
   * - A fresh site, a changed site, byte-identical output that writes nothing,
       removed files, underscore paths.
     - Done, against a real ``git receive-pack`` behind a local GitHub stand-in
       (``lib/pages/fake_test.odin``); ``git fsck --strict`` checks every push.
   * - Concurrent writers, a branch of other work, lost answers, journals, failed and
       pending deployments.
     - Done in the same tests: conflict, adoption, lost-before and lost-after, recover,
       ``errored``, ``building``, and no build.
   * - Unbound, renamed, re-created, and default-branch targets; another account.
     - Done.
   * - The same outcomes linked in and through the process protocol.
     - Done: the ``guidedog-pages`` program, driven through ``lib/plugins``, publishes
       as the linked publisher does.
   * - Device flow: pending, slow down, success; denial, expiry, cancellation.
     - Done against the stand-in. The release ships no app: an app registered by the
       maintainers, with its permissions published, is still to be decided.
   * - Real Keychain, Credential Manager, and Secret Service.
     - The Keychain is tested on macOS; Windows qualification requires a real
       Credential Manager round trip. Linux qualification starts an isolated D-Bus
       session and GNOME keyring, and requires Secret Service storage and removal.
       Headless environments still refuse unavailable stores without a plaintext fallback.
   * - A real repository on GitHub.com.
     - Done on 2026-10-13, with a disposable public repository,
       ``insanai/guidedog-pages-check``, made for the purpose, and the maintainer's OAuth token
       (``repo`` scope): ``init``, ``bind``, a first publication that made ``gh-pages``
       (3.3 s; ``Accepted``, ``pages.no-site`` until GitHub enabled Pages), ``status``
       ``Live`` once the Pages builds API listed that commit as ``built``; a changed page
       ``Live`` after 28 s of waiting; the same site again ``Unchanged``; a publication
       through ``guidedog publish`` and the process protocol; one with a token stored in
       the macOS Keychain; ``auth logout``. The served site showed each change and
       ``_static/``; a clone passed ``git fsck --strict``, with one linear commit per
       publication, each with its trailer. A conflict on GitHub itself was not staged;
       receive-pack's refusal is qualified against git.
   * - The manual in every maintained language; scheduler examples; release on all
       five platforms.
     - The English manual is complete; translations ship with 0.6.0. Release
       qualification covers the five platforms with an offline smoke test.

The maintainer accepted the implemented scope on 14 October 2026. The
credential adapters are qualified against the native stores. A maintainer-owned
device-flow app is a separate deployment decision; importing a token or using an
explicit client ID remains supported. See :doc:`../evidence/plugin-acceptance-20261014`.

0.6.1 reliability review
------------------------

Credential copies remember their allocation owner and are wiped before release.
Native-store allocations are checked before crossing C bindings. Local metadata
updates hold an exclusive lock across the whole read/replace operation, including
separate sessions in one process. Dry runs have separate build folders.

An accepted push remains accepted if saving the local report fails. The journal is
deleted only after that report is durable, so ``recover`` can reconcile a lost reply
or local disk error. It never resets the remote branch or repeats a push blindly.
The plugin host now bounds whole requests; a timeout after a possible remote write
reports ``Unknown`` and asks for recovery. These are executable regression fixtures.

The manual teaches the native publisher interface using this implementation and
the smaller folder publisher. All seven editions share the tested source listings.

Open questions
--------------


Who registers and owns the device-flow app, and whether it is a GitHub App (repository
selection, fine-grained permissions) or an OAuth App (simpler, broader scopes). The Pages builds API reported the branch deployment
in the qualification. Whether it does on every plan, or the deployments API is needed
as a second source, is still to be seen. Enterprise Server, custom domains, and forward
rollback each need their own qualification.

Implementation in Rust
----------------------

GDS 0011 moves the implementation to Rust. Both routes, the configuration, bindings,
commands, outcomes and exit statuses (0, 1, 2, 4, 6, 7, 130) are unchanged.

.. list-table:: Where the publisher lives in Rust
   :header-rows: 1
   :widths: 30 70

   * - Odin
     - Rust
   * - ``lib/git``
     - ``gd-git``: objects, trees, commits, packs (``PackWriter`` removes an unfinished
       pack on drop), receive-pack messages.
   * - ``lib/fetch``
     - ``gd-fetch``: the ``curl`` program with an argument vector instead of libcurl and
       WinHTTP. Headers go to curl in a private file (``--header @file``, mode 0600,
       removed after the request), never on its command line; the body on its stdin.
   * - ``lib/pages``, ``lib/pages/command``
     - ``gd-pages`` (the ``pages`` publisher of ``gd-plugins``) and
       ``cmd/guidedog-pages``.

Credentials: on Linux and other Unix desktops the Secret Service is reached through the
``secret-tool`` program with the token on standard input and output, keeping the Odin
schema's attributes so existing tokens are found. macOS uses the Keychain and Windows the
Credential Manager through small FFI modules. There is never a plaintext fallback, and
``Secret`` zeroes itself on drop. Package globals became plain data (``Settings``,
``Roots``), so only code, never the environment, can point a release away from GitHub.
``guidedog-pages workflow`` writes or checks the GitHub Actions workflow of the second
route; ``gd-cli`` is planned to offer the guided ``guidedog publish pages`` and
``guidedog pages workflow`` over the same library.

The acceptance evidence above, including the GitHub.com run of 2026-10-13, was made with
the Odin implementation and stays as history; the Rust crates carry the stand-in tests.

Review history
--------------

2026-10-12: created at the maintainer's request. The design separates local branch
publication from Actions artifacts because the Pages deployment API requires
Actions OIDC. The manual gains the currently usable Actions route.

2026-10-13: reviewed and implemented for Guidedog 0.6.0. The review replaced the Git
Database API with one smart-HTTP push (secondary rate limits; a true compare-and-swap),
built with the installed ``guidedog`` instead of a linked engine, froze a private build
folder instead of an output lock that does not exist, and made the device-flow app a
build setting. The record now describes the implementation.

2026-10-15: the implementation moved from Odin to Rust (GDS 0011). Added
"Implementation in Rust"; the decisions, evidence and lifecycle above are kept as
they were recorded.

References
----------

* :doc:`0002-guidedog-cli`, the command line and toolchain.
* :doc:`0004-budgets-and-trust`, memory and trust boundaries.
* :doc:`0006-sphinx-replacement`, local project publication.
* :doc:`0007-discussion-catalogs`, the catalog used to manage this record.
* :doc:`0009-native-plugin-api`, the plugin foundation.
* `GitHub Pages publishing sources <https://docs.github.com/en/pages/getting-started-with-github-pages/configuring-a-publishing-source-for-your-github-pages-site>`_.
* `Pages REST API <https://docs.github.com/en/rest/pages/pages>`_.
* `Rate limits for the REST API <https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api>`_, secondary limits.
* `Git's HTTP protocol <https://git-scm.com/docs/http-protocol>`_ and `pack format <https://git-scm.com/docs/pack-format>`_.
* `OAuth device authorization <https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps>`_.
* `GitHub App user tokens <https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app>`_.
* `GitHub Pages limits <https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits>`_.
* ``docs/manual/howto/github-pages.rst``, both routes, step by step.
* ``lib/pages``, ``lib/git``, ``lib/fetch``, and ``lib/pages/command``.

.. rubric:: Lifecycle event

::

   2026-10-12: prediscussion → discussion. Open the maintainer-requested
   publishing plugin design for review

.. rubric:: Lifecycle event

::

   2026-10-14: discussion → accepted. Maintainer requested acceptance after
   review of the implemented 0.6.2 scope.

.. rubric:: Lifecycle event

::

   2026-10-14: accepted → committed. Implemented scope reviewed against source
   and published qualification.
   docs/gds/evidence/plugin-acceptance-20261014.rst;
   docs/manual/evidence/plugin-review-20261014.md; successful 0.6.2 release run
   38434657070.

.. rubric:: Implementation revision, 2026-10-15

The implementation moved from Odin to Rust; see GDS 0011 and the section
"Implementation in Rust". This is an implementation revision. It does not change
this record's review state or assert new acceptance evidence.
