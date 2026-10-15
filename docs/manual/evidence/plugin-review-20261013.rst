.. _native-publisher-review--13-october-2026:

Native publisher review, 13 October 2026
=========================================

This review compares the released 0.6.0 implementation with GDS 0008 and 0009. The 0.6.1
changes address failures in lifetime, blocking I/O, local metadata transactions and
publication evidence. They keep the existing architecture: Odin procedure tables, explicit
owners, one process per session, one conditional Git ref update, and an explicit machine
binding. There is no Python plugin loader or runtime compiler.

Architectural assessment
------------------------

The boundary is appropriately small. A publisher receives a finished site's files, not the
engine AST, parser state or cache. ``lib/plugins`` depends on Odin core and the TOML reader
and a small core-only filesystem adapter. ``lib/pages`` adds HTTP and Git, not another
document engine. The standalone Pages command invokes the installed Guidedog; the main CLI
builds in process.

The API follows the useful part of raylib's style: ordinary structs, named operations,
options with documented defaults, and paired cleanup. The procedure table is confined to the
interchangeable provider boundary. There is no hidden registration, inheritance hierarchy or
global error state. Keep it that way when adding providers.

Ownership is the essential API contract. A session, snapshot, plan and result each owns its
arena. A plan borrows its snapshot and session. It must be released before those owners.
Result text belongs to the result and cannot be retained by a session. ``defer`` handles
local cleanup; ``os.exit`` must follow a returned ``run`` procedure so it does not skip
cleanup. The manual now teaches those lifetimes before publication.

The source SDK, process protocol, snapshot schema and product version remain separate. SDK
1.1 adds cancellation, request deadlines and allocation/lock helpers while accepting 1.0
publisher tables. Protocol 1.0 and snapshot schema 1 retain their required meanings.
Optional manifest bounds let a new child recheck a custom-sized site; an older child may
safely refuse a site above its limits. This is source and protocol compatibility, not a
stable Odin dynamic-library ABI.

Defects and fundamental corrections
-----------------------------------

+-----------------------------+-----------------------------+-----------------------------+
| Defect                      | Cause                       | Correction and regression   |
|                             |                             | evidence                    |
+=============================+=============================+=============================+
| A silent publisher could    | Blocking writes and reads   | Parent pipe I/O is          |
| block indefinitely.         | had no whole-operation      | nonblocking. A monotonic    |
|                             | deadline. A read-only timer | deadline covers every write |
|                             | would still leave a full    | and response byte. Tests    |
|                             | pipe blocked.               | fill a real pipe, withhold  |
|                             |                             | responses and stall         |
|                             |                             | shutdown.                   |
+-----------------------------+-----------------------------+-----------------------------+
| A late reply or failure     | Request scratch and         | ``serve`` releases a        |
| could lose or misuse        | long-lived provider state   | scratch arena per request.  |
| memory.                     | were not separated, and     | Stable backing allocators   |
|                             | cleanup inherited the       | own the session, snapshot   |
|                             | caller's allocator.         | and plan. Unload uses its   |
|                             |                             | session allocator even      |
|                             |                             | after failed load. Tests    |
|                             |                             | free returned-line memory   |
|                             |                             | between adjacent messages   |
|                             |                             | and refuse allocations in   |
|                             |                             | turn.                       |
+-----------------------------+-----------------------------+-----------------------------+
| A snapshot root could       | The root was borrowed from  | Clone the root into         |
| outlive its source string.  | the request instead of      | snapshot memory. A private  |
|                             | owned with the manifest.    | copied or adopted folder is |
|                             |                             | released by its owner.      |
|                             |                             | Rechecks use accepted       |
|                             |                             | manifest bounds rather than |
|                             |                             | resetting to GitHub         |
|                             |                             | defaults. A sparse-file     |
|                             |                             | test crosses the default    |
|                             |                             | file-size boundary.         |
+-----------------------------+-----------------------------+-----------------------------+
| A registry update could     | Builder and append helpers  | Registry mutations use a    |
| silently omit data after    | hid allocation errors       | guarded owner and refuse    |
| allocation refusal.         | before filesystem           | before writing the next     |
|                             | publication.                | file. A fault test refuses  |
|                             |                             | each allocation and         |
|                             |                             | requires either a complete  |
|                             |                             | registry or no replacement. |
+-----------------------------+-----------------------------+-----------------------------+
| Two sessions in one process | POSIX record locks are      | Use exclusive nonblocking   |
| could modify metadata       | process-owned and do not    | descriptor locks, hold them |
| concurrently.               | exclude another open by     | across the whole            |
|                             | that process. Forked        | read/replace operation, and |
|                             | children can also briefly   | explicitly unlock before    |
|                             | retain inherited lock       | closing. Tests open the     |
|                             | descriptors.                | same file twice and require |
|                             |                             | conflict, then successful   |
|                             |                             | acquisition after release.  |
+-----------------------------+-----------------------------+-----------------------------+
| An accepted remote write    | The journal was dropped     | Save the last report first; |
| could lose recovery         | before durable local        | only then delete the        |
| evidence.                   | settlement completed.       | journal. A failed save      |
|                             |                             | returns ``Accepted`` with a |
|                             |                             | problem and leaves recovery |
|                             |                             | evidence. The fixture makes |
|                             |                             | the report path unwritable  |
|                             |                             | and recovers after          |
|                             |                             | repairing it.               |
+-----------------------------+-----------------------------+-----------------------------+
| Credential cleanup could    | A secret byte slice did not | ``Secret`` records its      |
| free through the wrong      | record its allocation       | allocator. Wipe copies      |
| allocator.                  | owner. Native conversions   | before release; check       |
|                             | also had unchecked refusal  | Keychain, Secret Service    |
|                             | paths.                      | and Windows string          |
|                             |                             | allocations before native   |
|                             |                             | calls. Native-store tests   |
|                             |                             | use disposable credentials. |
+-----------------------------+-----------------------------+-----------------------------+
| Git objects or reports      | Tree-map inserts, sort-time | Checked tree construction;  |
| could be incomplete under   | concatenation, parser       | a comparator with no        |
| memory pressure.            | appends and formatted       | allocation; checked         |
|                             | request buffers could fail  | capabilities and reference  |
|                             | silently.                   | lists; fixed bounded        |
|                             |                             | request framing. Fault      |
|                             |                             | tests require an error      |
|                             |                             | instead of a shorter        |
|                             |                             | successful tree or report.  |
|                             |                             | Packs reject invalid sizes  |
|                             |                             | and partial writes.         |
+-----------------------------+-----------------------------+-----------------------------+
| A child reply could look    | Missing booleans and        | Require typed ``ok``,       |
| successful despite          | mismatched target or        | complete plans, the session |
| malformed fields.           | operation identity were too | target, and the requested   |
|                             | permissive.                 | operation. Invalid or       |
|                             |                             | absent outcomes become      |
|                             |                             | ``Unknown`` when effects    |
|                             |                             | cannot be excluded. A       |
|                             |                             | native hostile-child        |
|                             |                             | fixture exercises malformed |
|                             |                             | replies and                 |
|                             |                             | write-then-stall behavior.  |
+-----------------------------+-----------------------------+-----------------------------+

A timeout does not prove that a remote write failed. The host reports ``Unknown`` after a
possible write and directs inspection or recovery. A local recording error after an accepted
push cannot safely be retried as an ordinary failure. The error text now preserves that
distinction and names a concrete next action.

Windows qualification also caught a closed-input pipe being mistaken for buffer pressure. A
nonblocking Windows write with a full buffer succeeds with a partial or zero-byte count; a
failed write is not a read-style empty-buffer retry. The adapter now reports a stopped
reader immediately. The existing real closed-pipe regression covers this on Windows, while
the full-pipe regression preserves the deadline behavior. See `Microsoft's pipe wait-mode
contract <https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-type-read-and-wait-modes>`__.

The subsequent Windows job stopped inside the concurrent fetch suite, with a native
segmentation fault in file-test cleanup. Streaming reads and isolating the HTTP test worker
did not remove that crash. A test-only native diagnostic then captured the fault inside
APR/Subversion and COM shell-extension initialization. Windows' standard recursive deletion
calls ``SHFileOperationW``, loading desktop shell extensions into the process. This is a
filesystem boundary problem, separate from pipe inheritance or an Odin allocation refusal.

The `full native
reproduction <https://github.com/insanai/guidedog/actions/runs/38354090468>`__ records
``SHFileOperationWithAdditionalFlags``, ``os._remove_all`` and the file-test cleanup below
the shell copy hook and APR/Subversion fault. Increasing the captured stack to 64 frames
established the call path before replacing it.

The core-only ``lib/fs`` adapter replaces that path throughout the engine, publishers,
examples and tests. Windows deletion uses directory iterators and direct file removal; it
has an explicit traversal stack, checks its owned allocations and closes handles on every
exit. The retained-target regression also showed that the old Linux recursive removal
followed a directory link into its target. All platforms now use the same traversal:
symbolic links and junctions are removed as leaves. Callers own changes to a tree during
cleanup; this is not a sandbox against another process replacing directories. Deep-tree and
retained link-target tests cover its contract. Qualification repeats the concurrent fetch
suite 30 times, with native fault traces and a watchdog so a lost test worker cannot turn a
crash into a silent job hang.

The pipe audit did establish a separate deterministic defect. Ordinary Windows process
creation inherited every inheritable handle, including stdin writers and other sessions'
pipe ends. Publisher startup now passes an explicit three-handle allowlist. A native fixture
requires an unrelated pipe to reach EOF while the child is still alive, then requires the
child to exit on its own input's EOF. Git oracle fixtures also mark their feeding writers
private. This follows `Microsoft's process-inheritance
contract <https://learn.microsoft.com/en-us/windows/win32/procthread/creating-processes>`__.

Both HTTP implementations refuse incomplete allocation results. WinHTTP checks
redirect-policy setup, status and header queries, closes body files on early failure, and
writes every byte even after a partial write. Libcurl checks option setup and header-list
appends before sending. It uses HTTP/HTTPS bit masks supported by older system libraries,
rather than ignoring failures from newer string options. The `curl redirect-option
contract <https://curl.se/libcurl/c/CURLOPT_REDIR_PROTOCOLS.html>`__ documents that
restriction. File and HTTP fault tests refuse each result allocation in turn and require
either a complete result or an error, with no leaked survivors.

GitHub Pages transaction
------------------------

The plan reads branch tip H. The publisher constructs candidate C with H as its parent,
writes H and C to its journal, and sends one receive-pack request to replace H with C. The
server accepts only if the branch still holds H. Another writer therefore produces a
conflict, not a forced overwrite. Recovery reads the branch and deployment evidence; it
never resets the branch or repeats the push blindly.

``Accepted`` is evidence that Git accepted C. ``Live`` requires Pages evidence for that same
commit. A previous successful deployment cannot establish that the new generation is live.
The first publication precedes ``setup``, because GitHub requires the configured source
branch to exist. The guide and release notes use that order.

The teaching folder publisher stages a complete copy and restores the old folder if
replacement fails. Its two renames have a brief missing-path window; it is not a
crash-journalled service. The chapter states this instead of presenting the example as an
equivalent local implementation of GitHub's atomic ref update.

Qualification and reproducibility
---------------------------------

Heavy checks run in an isolated checkout on the requested Linux machine. Commands:

.. code:: sh

   tools/release/suites.sh
   tools/release/suites.sh tests
   tools/release/credential-store.sh
   SANITIZE_FLAGS=-define:GRAPHDOG_STATIC=true tools/sanitize.sh
   tools/release/plugin-compatibility.sh
   sh tools/build-site.sh --guidedog build/guidedog --out build/site-review
   odin run tools/linkcheck -- build/site-review -v

All library and tool suites and all 174 integration tests passed on Linux with leak checks,
and again under AddressSanitizer. The isolated Secret Service round trip passed. The final
cleanup adapter, file/HTTP and pipe-boundary fixes passed Linux suites and AddressSanitizer
checks. All 174 integration tests passed again after the silent publisher fixture switched
to a hard-link alias of its already-built program. That avoids a freshly written executable
conflicting with concurrent forks; the real handshake timeout and outcome checks remain
intact.

The final seven-language site has 581 HTML pages and 193,675 checked internal links, with no
missing files or anchors. The source checker reports zero violations across 766 files. Test
programs live outside ``lib``, keeping them out of the public API pages.

The actual 0.6.0 host and a publisher compiled against the 0.6.0 SDK were checked with the
new counterparts in both directions: registration, dry run, first publication, unchanged
publication and a changed generation. ``plugin-compatibility.sh`` repeats the matrix in
release qualification with checksum-pinned previous release assets.

The manual has a ten-step tutorial in English, Simplified Chinese, Japanese, Korean, Hindi,
Spanish and German. Guidedog's native gettext and intl commands extract and merge the PO
catalogs. Translation statistics show no fuzzy or missing entries. Inline boundaries in the
new RST tutorial are checked without inserting visible spaces. The Markdown repository
overview uses Markdown delimiter rules, keeping Korean particles next to names without
printing escape characters. The overview and its translations include the current README
instructions. Complete downloadable listings are checked byte for byte against tested
source.

The `qualified 0.6.1
candidate <https://github.com/insanai/guidedog/actions/runs/38356475065>`__ passed every
gate on commit ``e87ac16d77abd83bab50734e386abef4f5ddbe7a``: both full Linux/macOS
qualification jobs, AddressSanitizer, the embedded-Typst builds, all five archive smoke
tests and Homebrew installation. Windows passed the filesystem fault tests, publisher handle
isolation, native credentials and 30 concurrent fetch rounds. The 0.6.1 tag names that exact
revision. Its `tag
workflow <https://github.com/insanai/guidedog/actions/runs/38358613741>`__ is the
authoritative publication record. Passing on Linux alone is not a substitute for Windows
pipe, credential and filesystem verification.

`Guidedog 0.6.1 <https://github.com/insanai/guidedog/releases/tag/v0.6.1>`__ was published
after every tag-workflow job passed. The downloaded macOS Apple silicon Guidedog and dscc
archives match ``SHA256SUMS``; GitHub build-provenance verification succeeds for both. All
four downloaded commands report 0.6.1. With an empty executable search path, Guidedog builds
the quickstart's HTML and PDF, dsc builds through the packaged Guidedog, and dscc builds
independently. The packaged Pages publisher registers and refuses an unbound target without
touching the user's credentials or machine registry.

The `Homebrew tap
update <https://github.com/insanai/homebrew-guidedog/actions/runs/38366631685>`__ installed
and tested the published formula before committing version 0.6.1. The `website
refresh <https://github.com/insanai/guidedog/actions/runs/38360041789>`__ also passed its
build, internal-link check and deployment.

Deliberate limits and decisions
-------------------------------

Plugins are trusted native programs with the invoking user's permissions. A checksum pin
controls upgrades; it is not a sandbox. The public catalog provides reviewed discovery
metadata and has tests, but it is not an installer. Automatic installation and third-party
admission need a separate policy.

There is no bundled GitHub device-login app. Users can import a fine-grained token or
provide a registered app's client ID. Registering a maintainer-owned app is a product and
permission decision, not an unfinished crash fix.

Native-store qualification exercises macOS Keychain, Windows Credential Manager and an
isolated Linux Secret Service. It does not promise every desktop's locked-session behavior
or a store on a headless machine. Unavailable stores fail without plaintext fallback. Native
library allocations and child RSS remain outside the engine's Odin allocation budget;
request deadlines are not an OS memory sandbox.

GDS 0008 and 0009 remain in discussion pending the maintainer's acceptance. Shipping 0.6.1
does not manufacture that decision. The beta remains scoped to trusted documentation
projects, with Windows supported by automated qualification and smoke checks but less field
use than Linux and macOS.
