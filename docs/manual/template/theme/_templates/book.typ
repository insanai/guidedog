#import "base-book.typ": book as base-book
#import "palette.typ": palette

// Keep the host's parameters explicit. The final positional argument is content.
#let book(
  title: "", author: "", version: "", date: "", lang: "en", paper: "a4",
  numbering: false, logo: none, copyright: "", version-label: "Version", body,
) = base-book(
  title: title, author: author, version: version, date: date, lang: lang,
  paper: paper, numbering: numbering, logo: logo, copyright: copyright,
  version-label: version-label, accent: palette.accent, size: palette.size,
  serif: palette.serif, sans: palette.sans, page-ref: n => [p. #n], body,
)
