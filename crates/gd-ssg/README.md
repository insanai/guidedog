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
```

`Publication` expresses a transition without sentinel strings.
`create_post`, `read_metadata` and `transition` return structured host problems.
One source update uses a directory lock and a synced atomic replacement; body bytes and permissions survive.
No command invokes version control.

The design and remaining website orchestration work are in `docs/gds/records/0013-static-site-generation.rst`.
