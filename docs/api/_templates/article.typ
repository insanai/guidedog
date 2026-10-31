#import "theme.typ": *
// A continuous standalone article. Shared rules keep equations and figures consistent.
#let book(title: "", author: "", version: "", date: "", lang: "en", paper: "a4",
  numbering: false, authors: (), updated: "", category: "", tags: (), summary: "",
  body) = {
  let scripts = gd-script-fonts(lang)
  let names = if authors.len() > 0 { authors } else if author != "" { (author,) } else { () }
  set document(title: title, author: names)
  set page(paper: paper, margin: (x: 30mm, y: 25mm),
    header: context {
      if counter(page).get().first() > 1 {
        set text(font: gd-sans + scripts, size: 8pt, fill: gd-palette.quiet)
        title
        v(3pt)
        line(length: 100%, stroke: 0.4pt + gd-palette.rule)
      }
    },
    footer: context {
      if counter(page).get().first() > 1 {
        align(center, text(font: gd-sans + scripts, size: 8pt, counter(page).display()))
      }
    })
  configure-document(lang: lang, size: 10.5pt)[
    #set heading(numbering: if numbering { "1.1" } else { none })
    #if category != "" or tags.len() > 0 {
      block(below: 8pt, text(font: gd-sans + scripts, size: 9pt,
        fill: gd-palette.accent, ((category,) + tags).filter(s => s != "").join(" / ")))
    }
    #block(below: 12pt)[
      #set par(justify: false)
      #text(font: gd-serif + scripts, size: 24pt, weight: "bold", title)
    ]
    #if names.len() > 0 { block(below: 6pt, text(font: gd-sans + scripts, names.join(", "))) }
    #if date != "" {
      text(font: gd-sans + scripts, size: 9pt, fill: gd-palette.quiet,
        date + if updated != "" and updated != date { " (revised " + updated + ")" } else { "" })
    }
    #line(length: 100%, stroke: 0.5pt + gd-palette.rule)
    #if summary != "" {
      block(inset: 10pt, fill: gd-palette.soft, width: 100%)[
        #strong[Abstract]
        #par(emph(summary))
      ]
    }
    #body
  ]
}
