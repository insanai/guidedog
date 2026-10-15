#let ink = rgb("1f2a37")
#let slate = rgb("5b6b7d")
#let rule = rgb("d7dee8")
#let teal = rgb("087e8b")
#let pale = rgb("edf6f7")

// The record the pages belong to, for the running head the frame draws.
#let gds-current = state("gds-current", none)

#let placeholder = "XXXXX"

#let pdf-frame(body) = {
  set document(title: "Guidedog Discussions", author: "Guidedog contributors")
  set page(paper: "a4", margin: (x: 25mm, top: 24mm, bottom: 22mm),
    header: context {
      let meta = gds-current.get()
      if counter(page).get().first() > 1 and meta != none {
        set text(8pt, fill: slate)
        grid(columns: (1fr, auto), column-gutter: 1em,
          [#meta.number · #meta.title], smallcaps(meta.state))
        v(-0.5em)
        line(length: 100%, stroke: 0.4pt + rule)
      }
    },
    footer: context align(center, text(8pt, fill: slate)[
      #counter(page).display("1") / #counter(page).final().first()]),
  )
  set text(font: "Libertinus Serif", size: 10.5pt, fill: ink, lang: "en")
  set par(justify: true, leading: 0.66em, spacing: 1.05em)
  set heading(numbering: "1.1")
  show heading: set text(fill: teal)
  show heading.where(level: 1): it => {
    v(0.9em, weak: true)
    block(below: 0.75em, text(14pt, it))
  }
  show heading.where(level: 2): set text(12pt)
  show raw: set text(font: "DejaVu Sans Mono", size: 8pt)
  show raw.where(block: true): it => block(
    width: 100%, fill: rgb("f4f6f8"), stroke: 0.4pt + rule, inset: 9pt, radius: 3pt,
    breakable: false,
  )[#it]
  set table(inset: 6pt, stroke: 0.4pt + rule)
  set figure(gap: 7pt)
  show link: set text(fill: teal)
  show outline.entry.where(level: 1): set text(weight: "bold")
  body
}

#let note(title, body) = block(
  width: 100%, fill: pale, inset: 10pt, radius: 4pt, breakable: false,
)[*#title* #body]

#let card(title, body) = block(
  width: 100%, fill: pale, stroke: 0.5pt + teal, inset: 8pt, radius: 4pt,
)[#align(center)[*#title* #linebreak() #text(9pt)[#body]]]

#let down = align(center, text(16pt, fill: teal)[↓])

#let flow(items) = align(center, block(width: 85%)[
  #for (i, item) in items.enumerate() {
    if i > 0 { down }
    card(item.at(0), item.at(1))
  }
])

// Native Typst vectors: four actors, ordered messages, no package downloads.
#let sequence(actors, messages) = {
  let step = 43mm
  let row = 15mm
  let height = 13mm + messages.len() * row
  block(width: 100%, height: height + 5mm, breakable: false)[
    #for (i, actor) in actors.enumerate() {
      place(top + left, dx: i * step, dy: 0mm,
        box(width: 32mm, align(center, text(9pt, weight: "bold", actor))))
      place(top + left, dx: i * step + 16mm, dy: 8mm,
        line(start: (0pt, 0pt), end: (0pt, height - 8mm),
          stroke: (paint: rgb("a4b5c4"), thickness: 0.6pt, dash: "dashed")))
    }
    #for (i, msg) in messages.enumerate() {
      let (from, to, label) = msg
      let x = calc.min(from, to) * step + 16mm
      let width = calc.abs(to - from) * step
      let y = 18mm + i * row
      place(top + left, dx: x, dy: y,
        line(length: width, stroke: 0.8pt + teal))
      place(top + left, dx: x, dy: y - 6mm,
        box(width: width, align(center, text(8pt, label))))
      place(top + left, dx: if to > from { x + width - 2mm } else { x - 1mm },
        dy: y - 3.2mm, text(13pt, fill: teal, if to > from { [›] } else { [‹] }))
    }
  ]
}

// State colours, as in the discussion index.
#let state-fill(state) = (
  prediscussion: rgb("fef3c7"), discussion: rgb("dcfce7"), accepted: rgb("cffafe"),
  published: rgb("dbeafe"), committed: rgb("ede9fe"), abandoned: rgb("e5e7eb"),
).at(state, default: rgb("fef3c7"))

#let gds-name(number) = if number == placeholder [GDS draft] else [GDS #number]

// memo is the record's status, as an RFC states its own; it follows the state.
#let memo(number, state) = {
  let standing = if number == placeholder {
    [It is a provisional draft: a record numbered #raw(placeholder) has no place in the
      project record yet.]
  } else if state == "abandoned" {
    [It is abandoned: kept for its reasoning, it describes nothing Guidedog does.]
  } else if state in ("published", "committed") {
    [It is part of the project record and describes what Guidedog does.]
  } else {
    [It is part of the project record. What it proposes is not necessarily implemented;
      it says what is.]
  }
  [This document is a Guidedog discussion record, written in Typst and kept in git. Its
    RFC-style structure keeps scope, rationale, trade-offs, and operational constraints
    explicit; GDS 0001 defines the process. #standing]
}

#let chip(label, fill) = box(inset: (x: 0.6em, y: 0.3em), radius: 999pt, fill: fill,
  text(8.5pt, weight: "bold", fill: ink, label))

#let field(label, body) = stack(spacing: 0.5em,
  text(7.5pt, weight: "bold", tracking: 0.09em, fill: slate, upper(label)),
  text(10pt, body))

#let pdf-record(number, title, state, kind, authors, created, updated, discussion,
  labels) = {
  counter(heading).update(0)
  gds-current.update((number: gds-name(number), title: title, state: state))
  block(width: 100%, inset: (x: 1.25em, y: 1.15em), radius: 4pt, fill: luma(99%),
    stroke: 0.6pt + rule)[
    #set par(justify: false)
    #grid(columns: (1fr, auto), column-gutter: 1.4em,
      [#text(12pt, weight: "bold")[Guidedog Working Group] \
        #text(9pt, fill: slate)[Request for Discussion and Implementation Record]],
      align(right)[#text(13.5pt, weight: "bold")[#gds-name(number)] \
        #text(9pt, fill: slate)[#kind]],
    )
    #v(0.8em)
    #text(22pt, weight: "bold", hyphenate: false)[#title]
    #v(0.1em)
    #line(length: 100%, stroke: 0.6pt + rule)
    #v(0.5em)
    #grid(columns: (1fr, 1fr, 1fr), column-gutter: 1.6em, row-gutter: 1.1em,
      field("State", chip(state, state-fill(state))),
      field("Created", created),
      field("Last updated", updated),
      field("Authors", authors.join(", ")),
      field("Labels", if labels.len() > 0 { labels.join(", ") } else [—]),
      field("Type", kind),
    )
    #v(0.9em)
    #field("Discussion", discussion)
  ]
  v(0.8em)
  block(width: 100%, inset: 0.9em, radius: 4pt, fill: pale)[
    #set par(justify: true)
    #text(9.5pt)[*Status of this memo.* #memo(number, state)]
  ]
  v(0.8em)
  block(width: 100%)[
    #set text(9.5pt)
    #outline(title: text(11pt, fill: teal)[Contents], indent: 1.2em, depth: 2)
  ]
  v(1em)
}

// Semantic HTML stays selectable; diagrams retain their vector layout.
#let web-frame(body) = {
  set document(title: "Guidedog Discussions", author: "Guidedog contributors")
  set text(lang: "en")
  set heading(numbering: "1.1")
  show pagebreak: none
  show figure: it => html.elem("figure")[
    #html.frame(block(width: 164mm, inset: 2mm, it.body))
    #if it.caption != none { html.elem("figcaption", it.caption.body) }
  ]
  body
}

#let frame(body) = context {
  if target() == "html" { web-frame(body) } else { pdf-frame(body) }
}

// record opens a discussion record with its identity card and the status of the memo,
// and, in the PDF, its contents. The arguments are the record's `gds-*` lets.
#let record(number, title, state, kind, authors, created, updated, discussion,
  labels: ()) = context {
  counter(heading).update(0)
  if target() == "html" {
    let item(label, body) = html.elem("div")[
      #html.elem("dt", label)
      #html.elem("dd", body)
    ]
    html.elem("header", attrs: (class: "record-card"))[
      #html.elem("div", attrs: (class: "record-top"))[
        #html.elem("p", attrs: (class: "record-org"))[
          #html.elem("strong")[Guidedog Working Group]
          #html.elem("span")[Request for Discussion and Implementation Record]
        ]
        #html.elem("p", attrs: (class: "record-id"))[
          #html.elem("strong", gds-name(number))
          #html.elem("span", kind)
        ]
      ]
      #html.elem("h1", title)
      #html.elem("dl", attrs: (class: "record-fields"))[
        #item([State], html.elem("span", attrs: (class: "chip state-" + state), state))
        #item([Created], created)
        #item([Last updated], updated)
        #item([Authors], authors.join(", "))
        #item([Labels], if labels.len() > 0 { labels.join(", ") } else [—])
        #item([Type], kind)
        #html.elem("div", attrs: (class: "wide"))[
          #html.elem("dt")[Discussion]
          #html.elem("dd", discussion)
        ]
      ]
      #html.elem("aside", attrs: (class: "memo"))[
        #html.elem("strong")[Status of this memo.] #memo(number, state)
      ]
    ]
  } else {
    pdf-record(number, title, state, kind, authors, created, updated, discussion, labels)
  }
}
