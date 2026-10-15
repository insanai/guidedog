# A project theme

Follow the manual's [theme tutorial](https://insanai.github.io/guidedog/manual/tutorial/a-custom-theme.html).
Create a project with `guidedog quickstart`, rename its generated
`_templates/book.typ` to `_templates/base-book.typ`, then copy these files over it.
The base book comes from the same Guidedog version you are using; it is kept in
the project so the customized theme has no hidden dependency on a checkout.

The example extends the built-in HTML layout and wraps the generated PDF book.
It needs no plugin, compiler or additional JavaScript.
