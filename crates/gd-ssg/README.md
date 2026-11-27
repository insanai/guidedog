# gd-ssg

Source-level publication commands linked into `guidedog` as `ssg`.
The ordinary build pipeline owns publication filtering, collections and rendering.

```sh
guidedog ssg new notes.rst --title "Release notes"
guidedog ssg schedule notes.rst --at 2026-10-20T09:00:00Z
guidedog ssg publish notes.rst
guidedog ssg draft notes.rst
guidedog ssg status
guidedog build html --preview
guidedog ssg init journal --preset journal --title "Our journal"
guidedog ssg build journal
```

`Publication` expresses a transition without sentinel strings.
`create_post`, `read_metadata` and `transition` return structured host problems.
One source update uses a directory lock and a synced atomic replacement; body bytes and permissions survive.
No command invokes version control.

`build_site` assembles project mounts, language editions and shared assets with one output transaction.
It builds the editions in `guidedog build` child processes, as many at once as fit in memory (GDS 0020), or one after another through its `SiteHost`.
The work folder's `memory.toml` keeps each edition's peak memory, which weighs it in the next build.
`create_site` supplies complete project, journal and research presets.
`install_theme` requires an archive pin and atomically selects a verified, content-addressed theme tree.
Builds use the installed lock offline.

The complete design is in `docs/gds/records/0013-static-site-generation.rst`; commands and configuration are in `docs/manual/howto/static-publishing.rst`.
