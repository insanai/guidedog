#let gd-math-font = ()
#let gd-mono = ("mono",)
#let gd-math-sans-text(font, body) = math.sans(body)
#let text(font: (), weight: "regular", style: "normal", body) = {
  if weight == "bold" {
    math.bold(body)
  } else if style == "italic" {
    math.italic(body)
  } else if font != () {
    math.mono(body)
  } else {
    body
  }
}
#let box(body) = body
