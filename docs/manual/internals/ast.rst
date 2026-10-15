Guidedoc AST contract
=====================

This is the normative mapping between source constructs and Guidedoc nodes (GDS 0003). Readers produce exactly these shapes; renderers may rely on them. ``crates/guidedoc/src/kinds.rs`` (``child_allowed``) enforces the parent-child rules during building and validation.

General rules
-------------

-  Nodes are emitted in preorder through the builder (``Builder::begin_node``, ``end_node``,
   ``add_leaf``, ``with_node``). A node's attributes are added right after ``begin_node``,
   before children.
-  All node text lives in the snapshot text pool (``Builder::add_text``,
   ``text_open``/``text_push``/``text_close``, ``build_text``). Text is final: escapes
   processed, entities decoded, markup removed.
-  Spans are byte ranges in the node's source (``SourceSpan::source`` is the source table
   index; included files have their own index).
-  ``Node.value`` is a small per-kind scalar documented below; otherwise zero.
-  ``Node.payload`` is ``NONE`` unless the kind uses a payload row (List uses lists, Cell
   uses cells, link-like kinds use links).
-  ``LinkInfo.resolved`` is meaningful only when ``kind == LinkKind::Internal``.
-  Adjacent Text nodes are allowed but readers should merge runs they produce together.

Shared block kinds
------------------

+---------------+------------------------------------+------------------------------------+
| Kind          | Children                           | Fields                             |
+===============+====================================+====================================+
| Document      | blocks                             | root; span covers the main source; |
|               |                                    | ``value`` = DocumentReading:       |
|               |                                    | Sphinx (1) when the Sphinx layer   |
|               |                                    | read it, whose transforms give ids |
|               |                                    | of their own (see Renderer         |
|               |                                    | obligations)                       |
+---------------+------------------------------------+------------------------------------+
| Title         | inlines                            | document title (RST promotion);    |
|               |                                    | also first child of Admonition     |
|               |                                    | (generic), Topic, Sidebar, Table   |
|               |                                    | (caption), Directive               |
+---------------+------------------------------------+------------------------------------+
| Subtitle      | inlines                            | document or sidebar subtitle       |
+---------------+------------------------------------+------------------------------------+
| Section       | Heading, then blocks               | ``value`` = level (1 = top);       |
|               |                                    | parent Document, Section, Only, or |
|               |                                    | DescContent                        |
+---------------+------------------------------------+------------------------------------+
| Heading       | inlines                            | ``value`` = level 1..6             |
|               |                                    | (CommonMark) or section level      |
|               |                                    | (RST)                              |
+---------------+------------------------------------+------------------------------------+
| Paragraph     | inlines                            |                                    |
+---------------+------------------------------------+------------------------------------+
| BlockQuote    | blocks, optional trailing          | class attribute ``epigraph``,      |
|               | Attribution                        | ``highlights``, ``pull-quote`` for |
|               |                                    | those RST directives               |
+---------------+------------------------------------+------------------------------------+
| Attribution   | inlines                            |                                    |
+---------------+------------------------------------+------------------------------------+
| List          | ListItem                           | payload ListInfo{style, delimiter, |
|               |                                    | start}; flag Tight                 |
+---------------+------------------------------------+------------------------------------+
| ListItem      | blocks                             |                                    |
+---------------+------------------------------------+------------------------------------+
| CodeBlock     | none; Caption in the Sphinx layer  | ``text`` = code, ending in "\\n"   |
|               |                                    | when non-empty; attribute          |
|               |                                    | Language; ``value`` > 0: line      |
|               |                                    | numbers start there                |
+---------------+------------------------------------+------------------------------------+
| Preformatted  | inlines                            | RST parsed-literal; renderers keep |
|               |                                    | white space                        |
+---------------+------------------------------------+------------------------------------+
| DoctestBlock  | none                               | ``text`` = content                 |
+---------------+------------------------------------+------------------------------------+
| MathBlock     | none                               | ``text`` = LaTeX source (one       |
|               |                                    | equation group)                    |
+---------------+------------------------------------+------------------------------------+
| RawBlock      | none                               | ``text`` = raw content; attribute  |
|               |                                    | Format (``html``, ``typst``,       |
|               |                                    | ``latex``, ...)                    |
+---------------+------------------------------------+------------------------------------+
| ThematicBreak | none                               | CommonMark thematic break, RST     |
|               |                                    | transition                         |
+---------------+------------------------------------+------------------------------------+
| Comment       | none                               | ``text`` = comment content;        |
|               |                                    | renderers emit nothing             |
+---------------+------------------------------------+------------------------------------+

CommonMark tight lists: paragraphs inside a Tight list's items are still Paragraph nodes;
renderers omit their ``<p>`` wrappers (reference profile) exactly as CommonMark says.

CommonMark mapping
------------------

+-----------------------------+-----------------------------------------------------------+
| Construct                   | Node                                                      |
+=============================+===========================================================+
| ATX / setext heading        | Heading{value = level}                                    |
+-----------------------------+-----------------------------------------------------------+
| indented / fenced code      | CodeBlock; Language = first word of the info string       |
|                             | (escapes and entities processed); attribute Custom "info" |
|                             | holds the full info string when it has more than one word |
+-----------------------------+-----------------------------------------------------------+
| HTML block                  | RawBlock{Format "html"}; ``text`` = the lines verbatim,   |
|                             | each ending "\\n"                                         |
+-----------------------------+-----------------------------------------------------------+
| block quote                 | BlockQuote                                                |
+-----------------------------+-----------------------------------------------------------+
| bullet list                 | List{Bullet, delimiter = '-', '+', or '\*'}               |
+-----------------------------+-----------------------------------------------------------+
| ordered list                | List{Arabic, delimiter = '.' or ')', start}               |
+-----------------------------+-----------------------------------------------------------+
| link reference definition   | nothing (consumed)                                        |
+-----------------------------+-----------------------------------------------------------+
| text                        | Text                                                      |
+-----------------------------+-----------------------------------------------------------+
| soft / hard line break      | SoftBreak / HardBreak                                     |
+-----------------------------+-----------------------------------------------------------+
| code span                   | Code{text}                                                |
+-----------------------------+-----------------------------------------------------------+
| emphasis / strong           | Emphasis / Strong                                         |
+-----------------------------+-----------------------------------------------------------+
| inline link, reference link | Link; links row {destination, title, kind External};      |
|                             | children = link text                                      |
+-----------------------------+-----------------------------------------------------------+
| image                       | Image; links row {destination, title}; children =         |
|                             | description inlines                                       |
+-----------------------------+-----------------------------------------------------------+
| URI autolink                | Link{kind External, destination = URI as written}; child  |
|                             | Text                                                      |
+-----------------------------+-----------------------------------------------------------+
| email autolink              | Link{kind Email, destination = address}; child Text       |
+-----------------------------+-----------------------------------------------------------+
| raw HTML                    | RawInline{Format "html"}                                  |
+-----------------------------+-----------------------------------------------------------+

Destinations are stored after backslash-escape and entity processing, *not* percent-encoded;
the HTML renderer normalizes them as CommonMark's reference does.

The table above is the tree of ``commonmark::Settings::strict`` (pure CommonMark 0.31.2).

Markdown (MyST) mapping
-----------------------

By default the ``commonmark`` reader reads the Markdown that Sphinx projects write with
myst-parser 0.16.1 (:doc:`commonmark-ledger`). Its tree is reStructuredText's
for the equivalent construct, so renderers and the project host need nothing new:

+--------------------------------------------+--------------------------------------------+
| Construct                                  | Node                                       |
+============================================+============================================+
| heading at the top level                   | Section{value = depth} > Heading{value =   |
|                                            | depth}, as an RST section; a skipped level |
|                                            | nests one deeper; attribute Custom         |
|                                            | ``slug`` with ``heading_anchors``          |
+--------------------------------------------+--------------------------------------------+
| heading in a quote, list, or directive     | Rubric (warning ``myst.header``)           |
+--------------------------------------------+--------------------------------------------+
| ``(name)=``                                | Target{text = normalized name}; internal   |
|                                            | link row, resolved to the next element     |
+--------------------------------------------+--------------------------------------------+
| ``% comment`` lines                        | Comment{text} (consecutive lines join)     |
+--------------------------------------------+--------------------------------------------+
| ``+++ text``                               | Comment{text; Class ``block_break``}       |
+--------------------------------------------+--------------------------------------------+
| front matter                               | FieldList of its keys (bibliographic keys' |
|                                            | values are Markdown), which resolution     |
|                                            | turns into bibliographic fields;           |
|                                            | ``html_meta`` becomes Directive{Name       |
|                                            | "meta"} as the meta directive's;           |
|                                            | ``substitutions`` configure the document   |
+--------------------------------------------+--------------------------------------------+
| :literal:`\```{name} arg` , ``:::{name}``  | whatever the directive's handler builds    |
|                                            | (Sphinx layer); nothing, with a warning,   |
|                                            | without handlers                           |
+--------------------------------------------+--------------------------------------------+
| ``{name}`` role                            | whatever the role's handler builds;        |
|                                            | Role{text = name} > Text without handlers  |
+--------------------------------------------+--------------------------------------------+
| ``[^1]``, ``[^label]``                     | FootnoteReference as ``[1]_`` and          |
|                                            | ``[#label]_``: text "1" or "#label", flag  |
|                                            | Auto for labels                            |
+--------------------------------------------+--------------------------------------------+
| footnote definitions                       | Footnote nodes at the end of the document  |
|                                            | (inside the last open section), in order   |
|                                            | of first reference, after                  |
|                                            | ThematicBreak{Class ``footnotes``};        |
|                                            | unreferenced ones are dropped              |
+--------------------------------------------+--------------------------------------------+
| GFM table                                  | Table{value = columns; Class               |
|                                            | ``colwidths-auto``} > TableHead >          |
|                                            | Row{Header} > Cell{Header; Class           |
|                                            | ``text-left``/``-center``/``-right``} >    |
|                                            | Paragraph; TableBody likewise              |
+--------------------------------------------+--------------------------------------------+
| fenced code                                | CodeBlock{Language = first word;           |
|                                            | ``default`` without one, ``none`` for      |
|                                            | indented code, when a directive extension  |
|                                            | is present}                                |
+--------------------------------------------+--------------------------------------------+
| ``$x$``, ``$$x$$ (label)``                 | Math; MathBlock through the math directive |
|                                            | (Id ``equation-<label>``), else            |
|                                            | MathBlock{Custom label}                    |
+--------------------------------------------+--------------------------------------------+
| ``\begin{env}`` (amsmath)                  | MathBlock{Class ``amsmath``; Custom        |
|                                            | nowrap}, labelled ``amsmath-N`` when       |
|                                            | numbered                                   |
+--------------------------------------------+--------------------------------------------+
| ``[text](url)`` of an allowed scheme,      | Link{External}                             |
| autolinks                                  |                                            |
+--------------------------------------------+--------------------------------------------+
| ``[text](path.md)``, ``path.rst#x``        | Xref{reftype doc; name = path without      |
|                                            | suffix; Custom ``anchor`` = x}             |
+--------------------------------------------+--------------------------------------------+
| ``[text](target)`` otherwise               | Xref{reftype any; name = target}           |
+--------------------------------------------+--------------------------------------------+
| ``[text](#slug)`` of a heading anchor      | Link{Internal, resolved = the Section};    |
|                                            | without text, the heading's text           |
+--------------------------------------------+--------------------------------------------+
| term / ``: definition``                    | DefinitionList{Class ``simple myst``} >    |
|                                            | DefinitionItem > Term, Definition+         |
+--------------------------------------------+--------------------------------------------+
| ``:name: body``                            | FieldList{Class ``myst``} > Field >        |
|                                            | FieldName (inlines), FieldBody             |
+--------------------------------------------+--------------------------------------------+
| ``- [ ]``, ``- [x]``                       | List{Class ``contains-task-list``} >       |
|                                            | ListItem{Class ``task-list-item``} >       |
|                                            | Paragraph starting with RawInline{html}    |
|                                            | checkbox                                   |
+--------------------------------------------+--------------------------------------------+
| ``<div class="admonition kind">``          | Admonition{kind}, or generic with a title  |
|                                            | element                                    |
+--------------------------------------------+--------------------------------------------+
| ``<img ...>``                              | Image{Alt, Width, Height, Class, Name}     |
+--------------------------------------------+--------------------------------------------+
| ``{{ expression }}``                       | the Markdown its value gives, inline or,   |
|                                            | filling a paragraph, as blocks             |
+--------------------------------------------+--------------------------------------------+

Directive and role hosts: the RST reader's directives and roles serve a host markup through the ``guidedoc_rst::Host`` trait (``crates/guidedoc-rst/src/host.rs``): the host runs a directive with ``Parser::run_hosted``, a role with ``guidedoc_rst_inline::apply_host_role``, and the handler asks for nested content with ``Parser::parse_body`` and ``Parser::inline``, which call back into the host for a hosted directive. The Markdown reader defines its own interface (``guidedoc_commonmark::Extension``, ``crates/guidedoc-commonmark/src/extension.rs``); ``MystExtension`` (Sphinx layer, ``crates/guidedoc-sphinx/src/markdown.rs``) connects the two, so one set of handlers serves reStructuredText and Markdown.

reStructuredText mapping
------------------------

Blocks:

+------------------------------+----------------------------------------------------------+
| Construct                    | Node                                                     |
+==============================+==========================================================+
| section                      | Section{value = level} > Heading{value = level}          |
|                              | (inlines), then body                                     |
+------------------------------+----------------------------------------------------------+
| transition                   | ThematicBreak                                            |
+------------------------------+----------------------------------------------------------+
| bullet list                  | List{Bullet, delimiter = '\*', '-', '+', '•', '‣', '⁃'}; |
|                              | Tight when every item holds at most one paragraph and    |
|                              | nothing else                                             |
+------------------------------+----------------------------------------------------------+
| enumerated list              | List{Arabic / LowerAlpha / UpperAlpha / LowerRoman /     |
|                              | UpperRoman, delimiter '.', ')' or '(' (both              |
|                              | parentheses), start}                                     |
+------------------------------+----------------------------------------------------------+
| definition list              | DefinitionList > DefinitionItem > Term, Classifier\*,    |
|                              | Definition                                               |
+------------------------------+----------------------------------------------------------+
| field list                   | FieldList > Field > FieldName, FieldBody                 |
+------------------------------+----------------------------------------------------------+
| option list                  | OptionList > OptionItem > OptionGroup > Option{text =    |
|                              | "-a" / "--all" / "/V"} > OptionArgument{text = argument; |
|                              | ``value`` = the delimiter byte, ' ' or '='}; then        |
|                              | OptionDescription                                        |
+------------------------------+----------------------------------------------------------+
| literal block (:: or quoted) | CodeBlock (no Language)                                  |
+------------------------------+----------------------------------------------------------+
| line block                   | LineBlock > Line (inlines) and nested LineBlock for      |
|                              | deeper indentation                                       |
+------------------------------+----------------------------------------------------------+
| block quote                  | BlockQuote (+ Attribution)                               |
+------------------------------+----------------------------------------------------------+
| doctest block                | DoctestBlock                                             |
+------------------------------+----------------------------------------------------------+
| grid / simple table          | Table{value = columns; attribute Widths "a b c"          |
|                              | relative} > [Title] TableHead? > Row{Header} > Cell;     |
|                              | TableBody > Row > Cell; Cell payload {row_span,          |
|                              | col_span}; cells hold blocks                             |
+------------------------------+----------------------------------------------------------+
| footnote                     | Footnote{text = label as written: "1", "#", "#name",     |
|                              | "\*"; flags Auto / Symbol; ``value`` = number            |
|                              | (resolution), or symbol index for Symbol} > blocks       |
+------------------------------+----------------------------------------------------------+
| citation                     | Citation{text = label} > blocks                          |
+------------------------------+----------------------------------------------------------+
| hyperlink target             | Target{text = normalized name, "" for anonymous; flag    |
|                              | Anonymous}; links row: External {destination}, indirect  |
|                              | {name = referenced refname}, internal {} (resolution     |
|                              | sets Internal + resolved = next element)                 |
+------------------------------+----------------------------------------------------------+
| substitution definition      | SubstitutionDefinition{text = normalized name} > inlines |
+------------------------------+----------------------------------------------------------+
| comment                      | Comment{text}                                            |
+------------------------------+----------------------------------------------------------+

Directives:

+--------------------------------------------+--------------------------------------------+
| Directive                                  | Node                                       |
+============================================+============================================+
| attention, caution, danger, error, hint,   | Admonition{value = AdmonitionKind} >       |
| important, note, tip, warning, seealso     | blocks                                     |
+--------------------------------------------+--------------------------------------------+
| admonition                                 | Admonition{Generic} > Title, blocks        |
+--------------------------------------------+--------------------------------------------+
| image                                      | Image (block level, no children); links    |
|                                            | row {destination = URI}; attributes Alt,   |
|                                            | Width, Height, Scale, Align; ``:target:``  |
|                                            | wraps it in Link (URI) or Reference        |
|                                            | (name\_)                                   |
+--------------------------------------------+--------------------------------------------+
| figure                                     | Figure > (Image or Link/Reference >        |
|                                            | Image), Caption?, Legend?                  |
+--------------------------------------------+--------------------------------------------+
| topic                                      | Topic > Title, blocks                      |
+--------------------------------------------+--------------------------------------------+
| sidebar                                    | Sidebar > Title, Subtitle?, blocks         |
+--------------------------------------------+--------------------------------------------+
| rubric                                     | Rubric (inlines)                           |
+--------------------------------------------+--------------------------------------------+
| epigraph, highlights, pull-quote           | BlockQuote with class                      |
+--------------------------------------------+--------------------------------------------+
| compound                                   | Compound > blocks                          |
+--------------------------------------------+--------------------------------------------+
| container                                  | Container{Class} > blocks                  |
+--------------------------------------------+--------------------------------------------+
| parsed-literal                             | Preformatted (inlines)                     |
+--------------------------------------------+--------------------------------------------+
| code, code-block, sourcecode               | CodeBlock{Language}; ``:number-lines:``    |
|                                            | sets ``value``                             |
+--------------------------------------------+--------------------------------------------+
| math                                       | MathBlock per blank-line-separated group   |
+--------------------------------------------+--------------------------------------------+
| raw                                        | RawBlock{Format}                           |
+--------------------------------------------+--------------------------------------------+
| table                                      | Table with Title first                     |
+--------------------------------------------+--------------------------------------------+
| csv-table, list-table                      | Table                                      |
+--------------------------------------------+--------------------------------------------+
| contents                                   | Contents{``value`` = depth, 0 = unlimited; |
|                                            | flag Local; attribute Title; Custom        |
|                                            | ``backlinks``}                             |
+--------------------------------------------+--------------------------------------------+
| sectnum, section-numbering                 | Directive{Name "sectnum"; attributes       |
|                                            | Custom "depth", "prefix", "suffix",        |
|                                            | "start"}                                   |
+--------------------------------------------+--------------------------------------------+
| header, footer                             | Directive{Name "header" / "footer"} >      |
|                                            | blocks                                     |
+--------------------------------------------+--------------------------------------------+
| meta                                       | Directive{Name "meta"}; attributes Custom  |
|                                            | name = content                             |
+--------------------------------------------+--------------------------------------------+
| title                                      | Directive{Name "title"; attribute Title}   |
+--------------------------------------------+--------------------------------------------+
| target-notes                               | Directive{Name "target-notes"}; renderers  |
|                                            | write the footnotes Docutils makes of it:  |
|                                            | one per external URI a reference leads to  |
|                                            | by name (or an anonymous reference), and a |
|                                            | footnote reference after each such         |
|                                            | reference                                  |
+--------------------------------------------+--------------------------------------------+
| include                                    | the included source's nodes, spans in its  |
|                                            | own source; ``:literal:`` / ``:code:``     |
|                                            | give CodeBlock                             |
+--------------------------------------------+--------------------------------------------+
| class                                      | Class attribute on the next element        |
+--------------------------------------------+--------------------------------------------+
| role, default-role                         | reader state only                          |
+--------------------------------------------+--------------------------------------------+
| replace, unicode, date, image (in          | inlines inside SubstitutionDefinition      |
| substitutions)                             |                                            |
+--------------------------------------------+--------------------------------------------+

Every directive option from the Docutils specification is parsed; options that affect
rendering become attributes (``:class:`` gives Class, ``:name:`` gives Name and a target).

Inline markup:

+--------------------------------------------+--------------------------------------------+
| Construct                                  | Node                                       |
+============================================+============================================+
| ``*emphasis*``, ``**strong**``             | Emphasis, Strong                           |
+--------------------------------------------+--------------------------------------------+
| :literal:`\`\`literal\`\``                 | Code                                       |
+--------------------------------------------+--------------------------------------------+
| interpreted text                           | per role: emphasis, strong and literal     |
|                                            | give Code; code gives Code{Language}; math |
|                                            | gives Math; sub/subscript,                 |
|                                            | sup/superscript; title-reference/title/t   |
|                                            | give TitleReference; pep-reference/PEP and |
|                                            | rfc-reference/RFC give Link{External};     |
|                                            | abbreviation/ab and acronym/ac give        |
|                                            | Abbreviation; raw-derived roles give       |
|                                            | RawInline{Format}; other custom roles give |
|                                            | Role{text = role name; Class}              |
+--------------------------------------------+--------------------------------------------+
| :literal:`\`text <uri>\`_`                 | Link{External, destination = uri, name =   |
|                                            | normalized text (defines a target)};       |
|                                            | ``__`` gives no name                       |
+--------------------------------------------+--------------------------------------------+
| :literal:`\`text <name_>\`_`               | Reference{name = refname}; attribute       |
|                                            | Custom "target-name" = normalized text,    |
|                                            | which also names an indirect target        |
+--------------------------------------------+--------------------------------------------+
| ``name_``, :literal:`\`phrase\`_`,         | Reference{links name; flag Anonymous for   |
| anonymous ``__``                           | ``__``}; resolution sets kind + resolved   |
|                                            | or destination                             |
+--------------------------------------------+--------------------------------------------+
| :literal:`_\`name\``                       | InlineTarget{text = normalized name} >     |
|                                            | inlines                                    |
+--------------------------------------------+--------------------------------------------+
| ``[1]_``, ``[#]_``, ``[#l]_``, ``[*]_``    | FootnoteReference{text = label as written; |
|                                            | flags Auto / Symbol; ``value`` = number    |
|                                            | after resolution}                          |
+--------------------------------------------+--------------------------------------------+
| ``[CIT]_``                                 | CitationReference{text = label}            |
+--------------------------------------------+--------------------------------------------+
| ``|name|``, ``|name|_``                    | SubstitutionReference{text = normalized    |
|                                            | name; resolved = definition}; with ``_``   |
|                                            | wrapped in Reference                       |
+--------------------------------------------+--------------------------------------------+
| standalone URI / email                     | Link{External or Email} > Text             |
+--------------------------------------------+--------------------------------------------+

Resolution (after parsing, before validation) fills link rows in place: a Reference to an
external or indirect target becomes kind External with the final destination; to an internal
target, kind Internal with resolved = the referent node (the element after the target, the
section of an implicit title target, or the InlineTarget). Unresolvable references are
errors (``rst.reference.unresolved``). Title promotion uses ``unwrap_node``.

Renderer obligations
--------------------

-  Link kind Project (set by a host pass when composing a multi-document book): the
   destination is a project anchor key. HTML treats it like External (an href); Typst emits
   ``#link(label("key"))[...]``.

-  ``RenderOptions::anchor_prefix`` is applied by ``build_anchors``; renderers need no code.

-  Words a renderer writes itself (admonition titles, "Added in version %s", permalink
   titles, "Contents") come from ``guidedoc::label(&options, Label::...)``, so a host can
   pass them translated in ``RenderOptions::labels``.

-  Typst with ``standalone = false`` emits only the body (no preamble, no document or page
   set rules), so a host can compose a book from chapters and one preamble.

-  Anchors come from ``build_anchors`` so HTML ids, Typst labels, and the labels a project
   records agree; renderers write ids only from that table. The ids are those Docutils gives
   a document read as Sphinx reads it (auto_id_prefix "id"), numbered in the order Docutils'
   parser and transforms give them (``src/anchors_order.rs``), so a reference, a footnote
   reference, a table-of-contents entry, or a problematic reference has the "idN" it has in
   Sphinx and Docutils. A document the Sphinx layer read also takes the ids of Sphinx's
   transforms: AutoNumbering gives a captioned Figure, a Table with a Title, and a CodeBlock
   with ``caption`` that have no id yet the next "idN" (before Docutils' transforms give
   theirs), and SortIds moves a section's first id after the others when it starts with
   "id". A renderer writes:

   -  a node's ids (``anchor_ids``) on its element: a section's on the section, a promoted
      document title's (the section it was) on the title, a label's on the element it names,
      and those of a Target that nothing after it can take where the Target stands; a
      definition item's ids go on its first term;
   -  an unresolved reference or substitution reference, kept as problematic text, with its
      id; the system messages Docutils would write are not written (their ids are given, so
      later ids are numbered as Docutils numbers them);
   -  the ids of generated elements from ``anchor_extras``: a table of contents' entries
      (owner the Contents, item the Section), and the footnotes of target-notes (owner the
      directive) and the footnote references after the references they note (owner the
      reference).
   -  a section title a table of contents lists, holding no reference, links back to it
      (HTML: ``<a class="toc-backref" role="doc-backlink">`` around the title and its
      number): to its entry, or with ``backlinks`` "top" to the table; the last table
      listing it wins.
   -  a node a host marks with Custom ``pinned-ids`` carries all its ids as Id attributes,
      in order, and gets none of its own: a project pins the ids a section was read with, so
      content a later ``only`` drops does not renumber them. A label naming it gives its id
      where the section's Id attributes place it.

-  SubstitutionReference renders its definition's children in place.

-  Unresolved links, raw content refused by policy, unsafe URLs, and untranslatable math are
   reported with the renderer's own ``Message`` constants, never silently dropped.

-  Under ``Policy::keep_documents_with_errors`` those that are content errors (status
   InvalidInput, Unsupported, or Policy; see ``content_error``) are reported as warnings
   (``kept_message``) and the page is written: an unresolved link becomes problematic text
   (HTML ``<span class="problematic">``, plain text in Typst and text), an undefined
   substitution becomes a problematic ``|name|`` in HTML, and refused content is left out.

Sphinx layer
------------

The ``sphinx`` reader (reStructuredText with the Sphinx directives, roles, and domains)
produces these shapes. Attributes are Custom unless a key is named. Values written by the
project host's link and numbering passes are marked *host*; everything else is set by the
reader.

+-----------------------------+-----------------------------+-----------------------------+
| Kind                        | Children                    | Fields                      |
+=============================+=============================+=============================+
| Toctree                     | ToctreeEntry\*              | ``value`` = maxdepth (0:    |
|                             |                             | unlimited); ``caption``,    |
|                             |                             | ``name``, ``hidden``,       |
|                             |                             | ``includehidden``,          |
|                             |                             | ``titlesonly``, ``glob``,   |
|                             |                             | ``reversed`` = "1" when     |
|                             |                             | set, ``numbered`` = depth   |
|                             |                             | ("999" for bare             |
|                             |                             | ``:numbered:``)             |
+-----------------------------+-----------------------------+-----------------------------+
| ToctreeEntry                | none                        | ``text`` = entry as written |
|                             |                             | (docname, glob pattern,     |
|                             |                             | URL, or ``self``);          |
|                             |                             | attribute Title = explicit  |
|                             |                             | title                       |
+-----------------------------+-----------------------------+-----------------------------+
| Only                        | blocks; Sections when the   | ``expression`` (for         |
|                             | directive stands where      | ``only``); ``ifconfig``     |
|                             | sections may (their         | holds the expression of     |
|                             | ``value`` counts from the   | ``ifconfig`` instead.       |
|                             | document, as their nesting  | Titles inside begin         |
|                             | does)                       | sections in a title context |
|                             |                             | of their own; when the      |
|                             |                             | first one's adornment is    |
|                             |                             | already in use, the Only    |
|                             |                             | stands where a title of     |
|                             |                             | that level would, after     |
|                             |                             | deeper sections close       |
|                             |                             | (Sphinx's Only directive)   |
+-----------------------------+-----------------------------+-----------------------------+
| Index                       | none (block or inline)      | ``entries`` = one entry per |
|                             |                             | line, as written            |
|                             |                             | (``single: a; b``,          |
|                             |                             | ``pair: x; y``,             |
|                             |                             | ``see: a; b``, ``!main``    |
|                             |                             | marks); attribute Id =      |
|                             |                             | ``index-N``                 |
+-----------------------------+-----------------------------+-----------------------------+
| Glossary                    | DefinitionList              | ``sorted`` = "1"; each Term |
|                             |                             | has attribute Id            |
|                             |                             | ``term-<id>`` and Name =    |
|                             |                             | the term                    |
+-----------------------------+-----------------------------+-----------------------------+
| VersionModified             | blocks                      | ``type`` = versionadded /   |
|                             |                             | versionchanged / deprecated |
|                             |                             | / versionremoved;           |
|                             |                             | ``version``; children are   |
|                             |                             | the explanation, possibly   |
|                             |                             | empty                       |
+-----------------------------+-----------------------------+-----------------------------+
| Desc                        | DescSignature+, Index\*,    | ``domain`` (py, c, cpp, js, |
|                             | DescContent                 | rst, std, odin, rust),      |
|                             |                             | ``objtype`` (function,      |
|                             |                             | class, ...), ``noindex`` =  |
|                             |                             | "1", ``no-contents-entry``  |
|                             |                             | = "1"                       |
+-----------------------------+-----------------------------+-----------------------------+
| DescSignature               | inlines: DescAnnotation,    | attribute Id per object id  |
|                             | DescAddname, DescName,      | (Sphinx's, e.g.             |
|                             | DescParameterlist,          | ``mod.func``),              |
|                             | DescReturns, Text           | ``fullname``, ``toc`` (the  |
|                             |                             | text of its table of        |
|                             |                             | contents entry, with        |
|                             |                             | toc_object_entries),        |
|                             |                             | ``params`` (a C function's  |
|                             |                             | parameter names, one per    |
|                             |                             | line); C++: ``cpp:path``    |
|                             |                             | (the symbol path from the   |
|                             |                             | root, one element per line: |
|                             |                             | identifier, template        |
|                             |                             | arguments, template         |
|                             |                             | parameters, display, and    |
|                             |                             | the element's id in         |
|                             |                             | versions 1 to 4, apart by   |
|                             |                             | U+001F), ``cpp:objtype``,   |
|                             |                             | ``cpp:id`` (the newest id), |
|                             |                             | ``cpp:display``,            |
|                             |                             | ``cpp:tparams``,            |
|                             |                             | ``cpp:fparams``; Odin:      |
|                             |                             | ``module`` = the package's  |
|                             |                             | import path, ``fullname`` = |
|                             |                             | the name within it          |
|                             |                             | ("Type.member"); an Odin    |
|                             |                             | procedure's named results   |
|                             |                             | are a second                |
|                             |                             | DescParameterlist after the |
|                             |                             | text " -> "; *host*:        |
|                             |                             | ``nopermalink`` on a        |
|                             |                             | redeclared C++ symbol (no   |
|                             |                             | permalink)                  |
+-----------------------------+-----------------------------+-----------------------------+
| DescContent                 | blocks; Sections where the  |                             |
|                             | directive stands where      |                             |
|                             | sections may: titles in an  |                             |
|                             | object's content, in a      |                             |
|                             | title context of their own  |                             |
|                             | (Sphinx's                   |                             |
|                             | parse_content_to_nodes with |                             |
|                             | section headings); they     |                             |
|                             | have no table of contents   |                             |
|                             | entry                       |                             |
+-----------------------------+-----------------------------+-----------------------------+
| Hlist                       | List                        | ``value`` = columns         |
+-----------------------------+-----------------------------+-----------------------------+
| ProductionList              | Production\*                | ``group``                   |
+-----------------------------+-----------------------------+-----------------------------+
| Production                  | inlines (the definition,    | ``text`` = token name;      |
|                             | with Xref to tokens)        | attribute Id                |
|                             |                             | ``grammar-token-...``       |
+-----------------------------+-----------------------------+-----------------------------+
| Xref                        | inlines: the display        | links row {name = target as |
|                             | content                     | written, kind Unresolved};  |
|                             |                             | ``reftype`` (doc, ref,      |
|                             |                             | numref, any, term, keyword, |
|                             |                             | option, envvar, token,      |
|                             |                             | download, eq, or            |
|                             |                             | ``domain:role``),           |
|                             |                             | ``refexplicit`` = "1" with  |
|                             |                             | an explicit title,          |
|                             |                             | ``refspecific`` = "1" for a |
|                             |                             | leading ``.``,              |
|                             |                             | ``py:module`` and           |
|                             |                             | ``py:class`` = context (C:  |
|                             |                             | ``c:parent``; Odin:         |
|                             |                             | ``odin:package`` and        |
|                             |                             | ``odin:type``; Rust:        |
|                             |                             | ``rust:module`` and         |
|                             |                             | ``rust:type``; C++:         |
|                             |                             | ``cpp:parent``, the path of |
|                             |                             | the symbol a name is looked |
|                             |                             | up from; reftype            |
|                             |                             | ``cpp:identifier`` for the  |
|                             |                             | names in a C++ signature);  |
|                             |                             | code-like roles wrap the    |
|                             |                             | display in Code;            |
|                             |                             | ``unstyled`` = "1" when an  |
|                             |                             | info field made it (its     |
|                             |                             | display has no role         |
|                             |                             | classes)                    |
+-----------------------------+-----------------------------+-----------------------------+
| DescName, DescAddname,      | inlines                     | parts of a signature        |
| DescAnnotation,             |                             |                             |
| DescReturns, DescParameter, |                             |                             |
| DescOptional                |                             |                             |
+-----------------------------+-----------------------------+-----------------------------+
| DescParameterlist           | DescParameter /             |                             |
|                             | DescOptional                |                             |
+-----------------------------+-----------------------------+-----------------------------+

Other Sphinx constructs use existing kinds:

-  ``code-block``, ``sourcecode``, ``code``, ``literalinclude``: CodeBlock with Language,
   ``value`` = first line number when numbered, ``linenos`` = "1", ``emphasize-lines`` =
   "1,3-5" (after ``dedent`` and line selection), ``caption`` = the caption as written
   (literalinclude's file name for an empty one), whose inline markup a Caption child holds
   parsed, as Sphinx's container_wrapper parses it (in Markdown, as Markdown); ``:name:`` gives
   Custom ``name``. The graphviz directives give their DOT CodeBlock{``graphviz`` = "1"} the
   same ``caption`` and Caption. Renderers show the Caption's content, or the attribute's
   text when there is no Caption.
-  ``seealso``: Admonition{Seealso}; ``todo``: Admonition{Todo}; ``todolist``,
   ``sectionauthor``, ``moduleauthor``, ``codeauthor``: Directive{Name} with ``author``;
   ``centered``: Paragraph with class ``centered``; ``tabularcolumns``, ``highlight``,
   ``default-domain``, ``currentmodule``: reader state only.
-  ``math`` with ``:label:``: MathBlock with Id ``equation-<label>``; ``:nowrap:`` gives
   ``nowrap``.
-  Info fields: a FieldList directly in a DescContent is rebuilt as Sphinx's
   DocFieldTransformer does for the object's domain and type. It keeps its attributes; a
   field of a known type is Field{Custom ``docfield`` = Sphinx's type name, such as
   ``parameter``, ``exceptions``, ``returntype``} > FieldName (the translated group name,
   "Parameters") and FieldBody, holding List{Bullet} > ListItem > Paragraph per use, or the
   Paragraph alone for a use that collapses or a plain type. A use's paragraph holds the
   argument as Strong{Custom ``literal`` = "1"} (Sphinx's literal_strong), the type in Text
   " (" and ")" as Xref and Emphasis{``literal``} (literal_emphasis) parts, Text " -- " (an
   en dash once smart quotes apply), and the field's inline content; content of several
   blocks follows the paragraph. Other fields are kept, their name capitalized; Python's
   ``:meta:`` fields are dropped. Smart quotes leave ``literal`` text as written. The words
   the transform adds (group names, arguments, types, separators) carry flag Generated, so a
   use's message for translation is its content alone.
-  ``doctest``, ``testcode``, ``testoutput`` (sphinx.ext.doctest): CodeBlock with Language
   ``pycon``, ``python``, ``none``, and Custom ``testnodetype`` (the directive) and
   ``groups``; ``testsetup``, ``testcleanup``, and ``:hide:`` give Comment{text} instead.
   Shown sessions have ``# doctest:`` flags and ``<BLANKLINE>`` markers trimmed
   (``trim_doctest_flags``).
-  Roles ``kbd``, ``guilabel``, ``menuselection``, ``command``, ``dfn``, ``file``, ``samp``,
   ``program``, ``mimetype``, ``newsgroup``, ``mailheader``, ``makevar``, ``manpage``,
   ``regexp``: Role{text = role name} with the processed content (``{var}`` in ``samp`` and
   ``file`` as Emphasis; the ``&`` accelerator of ``guilabel``/``menuselection`` as
   Role{accelerator}; ``-->`` of ``menuselection`` as "‣"). ``abbr``: Abbreviation with
   Title. ``index``: Index then the text.
-  *host*: Section ``secnumber`` ("1.2"); Figure, Table, CodeBlock, MathBlock ``number`` and
   ``number-label`` ("Fig. 1.2"); Directive{todolist} is replaced by the collected todos.

The host's link pass replaces Toctree with Container{class ``toctree-wrapper compound``}
holding the caption (Paragraph with class ``caption``) and nested Lists of Links, and
replaces each Xref with a Link (External, Project, or Internal) or, when unresolved, with
Role{text = "xref"} and a warning. Only nodes are kept or dropped by their expression.
Renderers therefore rarely meet Toctree, Xref, or Only; when they do (single-file
conversion), they render Xref and Only content as plain inline and block content and Toctree
as nothing.

Renderer obligations for this layer:

-  Emit every id from ``anchor_ids``: the primary on the element, aliases as empty
   ``<span id>`` in HTML and ``#metadata(none)<id>`` labels in Typst.
-  ``secnumber`` prefixes headings; ``number-label`` prefixes captions (HTML:
   ``<span class="caption-number">Fig. 1.2 </span>``); equations show ``(number)``.
-  HTML permalinks (``RenderOptions::permalinks``): a ``¶`` link after headings, captions,
   signatures, and glossary terms (not a term with classifiers, as Sphinx's writer).
-  HTML writes a field list directly in a DescContent as Sphinx's writer does:
   ``<dl class="field-list simple">`` (simple as Docutils' SimpleListChecker decides, a
   field with ``docfield`` always),
   ``<dt class="field-odd">Name<span class="colon">:</span></dt>``,
   ``<dd class="field-odd">``, rows alternating odd and even, and a list in a body as
   ``<ul class="simple">`` with ``<li><p>...</p></li>``. An Xref with ``unstyled`` shows
   only its display, as an info field's pending_xref has no role classes.
-  Code: ``RenderOptions::highlighter``, when set, writes highlighted HTML for a block's
   language; otherwise the code is escaped text. ``emphasize-lines``, ``linenos``, and
   ``caption`` apply in both targets.
