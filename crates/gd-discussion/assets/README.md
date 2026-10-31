# @NAME@

Proposals and decisions for this project, written in reStructuredText. The prefix of
this catalog is `@PREFIX@`.

```sh
guidedog dsc new "A proposal"
guidedog dsc list
guidedog dsc check
guidedog dsc build
```

Read [workflow.rst](workflow.rst) for the commands, and edit the process draft in
`records/` to describe how this project decides. `guidedog dsc build` writes the website
to `_build/html` and a PDF per record to `_build/pdf`.

Each PDF uses an RFC-style discussion layout, with a metadata panel, state badges, a status note and compact contents. The HTML and PDF templates and static scripts (`_templates/layout.html`, `_templates/discussion.typ`, `_templates/theme.typ`, `_static/guidedog.css`, `_static/guidedog.js`, `_static/typography.js`) are copied here by `guidedog dsc init` so we can customize them directly; ordinary books use `_templates/book.typ`.
