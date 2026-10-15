.. _porting-odin-gd-code-to-the-guidedoc-crate:

Porting Odin ``gd.`` code to the ``guidedoc`` crate
===================================================

Readers, renderers, passes, and hosts written against the Odin core
(``import gd "../../core"``) map onto this crate as below. Every Rust name is exported at
the crate root: ``use guidedoc::{...}`` or ``use guidedoc as gd;`` and write
``gd::NodeKind::Paragraph``.

The Odin code is the specification of behaviour, not a template: keep its output and
diagnostics, but use the Rust forms on the right (``Option`` for absent values, ``Result``
and ``?`` for failures, iterators for traversals, owned ``Vec``/``String`` buffers that you
``clear()`` and reuse instead of scratch).

The ten idioms
--------------

+--------------------------------------------+--------------------------------------------+
| Odin                                       | Rust                                       |
+============================================+============================================+
| `                                          | ``let p = b                                |
| `p := gd.begin_node(b, .Paragraph, span)`` | .begin_node(NodeKind::Paragraph, span)?;`` |
| … ``gd.end_node(b, p)``                    | … ``b.end_node(p)?;`` or                   |
|                                            | ``b.with_node(NodeKind::P                  |
|                                            | aragraph, span, |b, p| { ...; Ok(()) })?`` |
+--------------------------------------------+--------------------------------------------+
| ``gd.add                                   | ``b                                        |
| _leaf(b, .Text, span, gd.add_text(b, s))`` | .add_text_leaf(NodeKind::Text, span, s)?`` |
|                                            | (two ``&mut`` calls cannot nest)           |
+--------------------------------------------+--------------------------------------------+
| ``start := gd.text_open(b); gd.text_pus    | same calls with ``?`` on pushes, or        |
| h(b, x); …; t := gd.text_close(b, start)`` | ``let t = b.build                          |
|                                            | _text(|b| { b.text_push(x)?; Ok(()) })?;`` |
+--------------------------------------------+--------------------------------------------+
| ``if b.status != .Ok do return b.status``  | ``?`` on every fallible builder call;      |
| / ``alive(p)``                             | ``b.is_ok()`` where you only test          |
+--------------------------------------------+--------------------------------------------+
| ``gd.report_message(ctx.reports, M, s      | ``ctx.repor                                |
| pan, gd.arg_span(s), gd.arg_static("x"))`` | t(M, span, arg_span(s), arg_static("x"))`` |
|                                            | (``Arg::None`` for a missing argument)     |
+--------------------------------------------+--------------------------------------------+
| ``for child in gd.next_child(v, &it)``     | ``for child in doc.children(id)``          |
| after ``it := gd.children(v, id)``         |                                            |
+--------------------------------------------+--------------------------------------------+
| ``v.snap.nodes.rows[id]``, ``.used``       | ``doc.node(id)`` / ``b.snap.nodes[id]``    |
|                                            | (indexable by ``NodeId``), ``.len()``      |
+--------------------------------------------+--------------------------------------------+
| ``x := gd.fir                              | ``if let Some(x) = doc.first_child(id)``   |
| st_child(v, id); if x != gd.INVALID_NODE`` |                                            |
+--------------------------------------------+--------------------------------------------+
| ``l, _ := gd.link_info(v, id)``            | ``doc.link_                                |
|                                            | info(id).unwrap_or(LinkInfo::UNRESOLVED)`` |
|                                            | (or match the ``Option``)                  |
+--------------------------------------------+--------------------------------------------+
| ``mark := gd.scratch_mark(ctx.scratch)`` … | a ``Vec<T>`` in your state struct,         |
| ``gd.scratch_take(ctx.scratch, T, n)``     | ``clear()``\ ed and refilled;              |
|                                            | ``Vec::with_capacity(n)`` when sizing      |
+--------------------------------------------+--------------------------------------------+

A reader returns ``Result<(), Fail>``: return ``Ok(())`` when done (the engine closes the
root with ``Builder::finish``), and ``Err(fail)`` (usually by ``?``) on failure. A renderer
returns ``Result<(), Fail>`` likewise; errors it reports through ``ctx.report`` still fail
the render as in Odin. Messages stay
``const M: Message = Message { code, severity, status, title, message, hint }`` (every
message has a hint).

Name map
--------

Conversion: workspace, requests, running
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

+--------------------------------------------+--------------------------------------------+
| Odin ``gd.``                               | Rust                                       |
+============================================+============================================+
| ``Workspace``, ``init_workspace``,         | ``Workspace::new()`` /                     |
| ``init_workspace_from_bytes``,             | ``Workspace::with_limits(limits)``;        |
| ``init_workspace_with_plan``, ``Storage``, | storage grows on demand and is reused,     |
| ``Snapshot_Storage``, ``Slab_Plan``,       | bounded by ``Limits``. No sizing step.     |
| ``slab_plan``, ``plan_bytes``,             |                                            |
| ``workspace_size``, ``Table``              |                                            |
+--------------------------------------------+--------------------------------------------+
| ``reset_workspace(&ws)``                   | ``ws.reset()``                             |
+--------------------------------------------+--------------------------------------------+
| ``Request``,                               | ``Request<'a>`` (``Default``),             |
| ``defau                                    | ``defau                                    |
| lt_request(name, text, reader, renderer)`` | lt_request(name, text, reader, renderer)`` |
+--------------------------------------------+--------------------------------------------+
| ``Source{name, text}``                     | ``Source { name, text }`` (borrowed        |
|                                            | ``&str``\ s); ``request.source.text``      |
+--------------------------------------------+--------------------------------------------+
| ``Read_Options``,                          | ``ReadOptions<'a>`` (``Default`` =         |
| ``default_read_options()``                 | ``default_read_options()``)                |
+--------------------------------------------+--------------------------------------------+
| ``Read_Extension``, ``read_extension(&s)`` | ``ReadExtension<'a>``,                     |
|                                            | ``read_extension(&s)`` (``T: 'static``)    |
+--------------------------------------------+--------------------------------------------+
| ``extension_as(e, T)``                     | ``e.downcast::<T>() -> Option<&T>``        |
+--------------------------------------------+--------------------------------------------+
| ``reader_settings(ctx, T) -> (^T, bool)``  | ``c                                        |
|                                            | tx.reader_settings::<T>()? -> Option<&T>`` |
|                                            | (``None``: defaults)                       |
+--------------------------------------------+--------------------------------------------+
| ``no_reader_settings(ctx)``                | ``ctx.no_reader_settings()?``              |
+--------------------------------------------+--------------------------------------------+
| ``Render_Options``                         | ``RenderOptions<'a>`` (``Default``:        |
|                                            | standalone)                                |
+--------------------------------------------+--------------------------------------------+
| ``Math_Style``                             | ``MathStyle``                              |
+--------------------------------------------+--------------------------------------------+
| ``Label``, ``Labels``, ``ENGLISH_LABELS``  | ``Label`` (CamelCase variants:             |
|                                            | ``See_Also`` → ``SeeAlso``), ``Labels``    |
|                                            | (``set``/``get``),                         |
|                                            | ``ENGLISH_LABELS[l as usize]``             |
+--------------------------------------------+--------------------------------------------+
| ``label(o, l)``,                           | ``label(&o, l)``,                          |
| ``admonition_label(o, k)``,                | ``admonition_label(&o, k)``,               |
| ``split_label(s)``                         | ``split_label(s) -> (&str, &str)``         |
+--------------------------------------------+--------------------------------------------+
| ``Highlighter{procedure, user}``           | ``tra                                      |
|                                            | it Highlighter { fn highlight(&self, langu |
|                                            | age, code, out: &mut Emitter) -> bool }``; |
|                                            | ``optio                                    |
|                                            | ns.highlighter: Option<&dyn Highlighter>`` |
+--------------------------------------------+--------------------------------------------+
| ``Policy``, ``Content_Policy``,            | ``Policy`` (``Default`` =                  |
| ``default_policy()``, ``trusted_policy()`` | ``default_policy()``), ``ContentPolicy``,  |
|                                            | same functions                             |
+--------------------------------------------+--------------------------------------------+
| ``Limits``, ``default_limits()``           | ``Limits`` (``Default``), adds             |
|                                            | ``max_text_bytes``, ``max_reports``,       |
|                                            | ``max_output_bytes``                       |
+--------------------------------------------+--------------------------------------------+
| ``Resource_Provider{procedure, user}``     | ``trait ResourceProvider                   |
|                                            | { fn provide(&self, from: &str, name: &str |
|                                            | ) -> Result<Resource, ResourceStatus> }``; |
|                                            | ``Resource { text: Arc<str>, canonical }`` |
+--------------------------------------------+--------------------------------------------+
| ``Resource_Status``                        | ``R                                        |
|                                            | esourceStatus::{Missing, Refused, Limit}`` |
|                                            | (no ``Ok``: the ``Result`` says it)        |
+--------------------------------------------+--------------------------------------------+
| ``Cancel_Probe``                           | ``r                                        |
|                                            | equest.cancel: Option<&dyn Fn() -> bool>`` |
+--------------------------------------------+--------------------------------------------+
| ``Pass{name, procedure, user}``,           | ``Pass { name, run: &closure }`` where the |
| ``Pass_Outcome``                           | closure is                                 |
|                                            | ``Fn                                       |
|                                            | (Document, &mut Builder) -> PassOutcome``; |
|                                            | ``PassOutcome``                            |
+--------------------------------------------+--------------------------------------------+
| ``Map_Entry``, ``request.source_map``      | ``MapEntry``;                              |
| slice                                      | ``request.source_map: bool``, read         |
|                                            | ``ws.source_map()``                        |
+--------------------------------------------+--------------------------------------------+
| ``convert(&                                | ``convert(&mut ws, &r                      |
| ws, registry, request, output) -> Result`` | egistry, &request) -> Result<&str, Fail>`` |
|                                            | (output borrows ``ws``)                    |
+--------------------------------------------+--------------------------------------------+
| ``convert_tex                              | ``convert_text(&mut ws, &registry, reade   |
| t(&ws, registry, reader, renderer, text)`` | r, renderer, text) -> Result<&str, Fail>`` |
+--------------------------------------------+--------------------------------------------+
| ``read_docum                               | ``read_document(&mut ws, &reader,          |
| ent(&ws, reader, request) -> Read_Result`` |  &request) -> Result<Document<'_>, Fail>`` |
+--------------------------------------------+--------------------------------------------+
| ``tran                                     | ``transform_document(&mut ws, &passes      |
| sform_document(&ws, doc, passes, limits)`` | , &limits) -> Result<Document<'_>, Fail>`` |
|                                            | (works on the current document)            |
+--------------------------------------------+--------------------------------------------+
| ``c                                        | ``check_render(&mut ws, &re                |
| heck_render(&ws, doc, renderer, request)`` | nderer, &request) -> Result<usize, Fail>`` |
|                                            | (exact size)                               |
+--------------------------------------------+--------------------------------------------+
| ``render_docum                             | ``render_document(&mut ws, &r              |
| ent(&ws, doc, renderer, request, output)`` | enderer, &request) -> Result<&str, Fail>`` |
+--------------------------------------------+--------------------------------------------+
| ``Document``, ``document_valid``,          | ``Document<'w>`` *is* the view; get it     |
| ``view(doc) -> (View, bool)``, ``View``    | from a stage's result or                   |
|                                            | ``ws.document() -> Option<Document>``.     |
|                                            | Stale handles do not compile.              |
+--------------------------------------------+--------------------------------------------+
| ``Registry{readers, renderers}``           | ``Registry { readers:                      |
|                                            |  Vec<Reader>, renderers: Vec<Renderer> }`` |
+--------------------------------------------+--------------------------------------------+
| ``find_reader(r, id)``,                    | ``r.find_reader(id) -> Option<&Reader>``,  |
| ``find_renderer(r, id)``                   | ``r.find_renderer(id)``                    |
+--------------------------------------------+--------------------------------------------+
| ``validate_registry(r)``                   | ``r.validate()`` /                         |
|                                            | ``v                                        |
|                                            | alidate_registry(&r) -> Result<(), Fail>`` |
+--------------------------------------------+--------------------------------------------+
| ``Conformance``                            | ``Conformance``                            |
+--------------------------------------------+--------------------------------------------+
| ``VERSION``, ``SCHEMA_VERSION``            | ``VERSION`` (the workspace version),       |
|                                            | ``SCHEMA_VERSION``                         |
+--------------------------------------------+--------------------------------------------+

Results and diagnostics
~~~~~~~~~~~~~~~~~~~~~~~

+--------------------------------------------+--------------------------------------------+
| Odin ``gd.``                               | Rust                                       |
+============================================+============================================+
| ``Status`` (``.Invalid_Input``)            | ``Status::InvalidInput``, etc.             |
+--------------------------------------------+--------------------------------------------+
| ``Result``, ``Read_Result`` (``status``,   | ``Result<_, Fail>``;                       |
| ``primary``, ``need``, ``written``,        | ``Fail { stat                              |
| ``reports``)                               | us, need, diagnostic: Box<Diagnostic> }``, |
|                                            | ``fail.code()``; reports via               |
|                                            | ``ws.reports()``                           |
+--------------------------------------------+--------------------------------------------+
| ``Region``, ``Capacity_Need``              | ``Region`` (no ``Second_Bank``),           |
|                                            | `                                          |
|                                            | `CapacityNeed { region, minimum, exact }`` |
+--------------------------------------------+--------------------------------------------+
| ``Message``                                | ``Message`` (same fields, ``const``)       |
+--------------------------------------------+--------------------------------------------+
| ``Diagnostic`` (``d.code``, ``d.args``)    | ``Diagnostic`` (same flat fields;          |
|                                            | ``args: [Arg; 2]``),                       |
|                                            | ``d.formatted_message()``,                 |
|                                            | ``ws.describe(&d)`` quotes spans           |
+--------------------------------------------+--------------------------------------------+
| ``Diagnostic_Arg``, ``Arg_Kind``           | ``Arg::{N                                  |
|                                            | one, Int, Span, Text(Cow<'static, str>)}`` |
+--------------------------------------------+--------------------------------------------+
| ``diagnostic(m, span, a, b)``              | ``diagnostic(m, span, a, b)`` (pass        |
|                                            | ``Arg::None`` for absent ones)             |
+--------------------------------------------+--------------------------------------------+
| ``arg_int``, ``arg_span``, ``arg_static``  | same; ``arg_text(&str)`` copies non-static |
|                                            | text (Odin passed borrowed strings to      |
|                                            | ``arg_static``)                            |
+--------------------------------------------+--------------------------------------------+
| ``Severity``                               | ``Severity``                               |
+--------------------------------------------+--------------------------------------------+
| ``Reports``, ``report(r, d)``,             | ``Reports``; ``r.report(d)``,              |
| ``report_message(r, m, span, a, b)``       | ``r.message(m, span, a, b)``; from         |
|                                            | adapters ``ctx.report(...)`` /             |
|                                            | ``ctx.reports()`` / ``b.report(...)``      |
+--------------------------------------------+--------------------------------------------+
| ``reports.muted = true`` … restore         | ``                                         |
|                                            | let was = ctx.reports().set_muted(true);`` |
|                                            | … ``ctx.reports().set_muted(was);``        |
+--------------------------------------------+--------------------------------------------+
| ``reports.errors``, ``.warnings``,         | ``error_count()``, ``warning_count()``,    |
| ``.primary``, ``.items[:used]``            | ``primary() -> Option<&Diagnostic>``,      |
|                                            | ``items()``                                |
+--------------------------------------------+--------------------------------------------+
| ``first_problem(r)``                       | ``r.first_problem()``                      |
+--------------------------------------------+--------------------------------------------+
| ``content_error(status)``,                 | ``content_error(status)``,                 |
| ``kept_message(policy, m)``                | ``kept_message(&policy, m)``               |
+--------------------------------------------+--------------------------------------------+
| ``CORE_CAPACITY``, ``CORE_WORK``,          | same constants (``CORE_UTF8`` too)         |
| ``CORE_DEPTH``, ``CORE_STACK``,            |                                            |
| ``CORE_NODES``, ``CORE_UNHANDLED_KIND``,   |                                            |
| ``INCLUDE_MISSING``                        |                                            |
+--------------------------------------------+--------------------------------------------+

.. _reading-a-document-view--document:

Reading a document (``View`` → ``Document``)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

+--------------------------------------------+--------------------------------------------+
| Odin ``gd.`` (``v`` is a View)             | Rust (``doc: Document``)                   |
+============================================+============================================+
| ``node_count(v)``, ``node(v, id)``,        | ``doc.node_count()``,                      |
| ``kind(v, id)``                            | ``doc.node(id) -> &Node``,                 |
|                                            | ``doc.kind(id)``                           |
+--------------------------------------------+--------------------------------------------+
| ``subtree_end(v, id)``                     | ``doc.subtree_end(id) -> NodeId``          |
+--------------------------------------------+--------------------------------------------+
| ``text(v, t)``, ``node_text(v, id)``       | ``doc.text(t)``, ``doc.node_text(id)``     |
+--------------------------------------------+--------------------------------------------+
| ``has_children``, ``first_child``,         | ``doc.has_children(id)``,                  |
| ``code_caption``                           | ``doc.first_child(id) -> Option<NodeId>``, |
|                                            | ``doc.code_caption(id) -> Option<NodeId>`` |
+--------------------------------------------+--------------------------------------------+
| ``children``/``next_child``, ``Children``, | ``doc.children(id)`` (iterator),           |
| ``child_count``                            | ``Children``, ``doc.child_count(id)``      |
+--------------------------------------------+--------------------------------------------+
| ``parent_of(v, id)``                       | ``doc.parent_of(id) -> Option<NodeId>``    |
+--------------------------------------------+--------------------------------------------+
| `                                          | ``doc.attribute(id, key) -> Option<&str>`` |
| `attribute(v, id, key) -> (string, bool)`` |                                            |
+--------------------------------------------+--------------------------------------------+
| ``custom_attribute(v, id, name)``          | ``doc.cu                                   |
|                                            | stom_attribute(id, name) -> Option<&str>`` |
+--------------------------------------------+--------------------------------------------+
| ``attributes``/``next_attribute``,         | ``doc.attributes(id) -> &[Attribute]``     |
| ``Attributes``                             |                                            |
+--------------------------------------------+--------------------------------------------+
| ``has_class(v, id, c)``                    | ``doc.has_class(id, c)``                   |
+--------------------------------------------+--------------------------------------------+
| ``list_info``, ``cell_info``               | ``doc.list_info(id)``,                     |
|                                            | ``doc.cell_info(id)`` (defaults when       |
|                                            | absent)                                    |
+--------------------------------------------+--------------------------------------------+
| ``link_info(v, id) -> (Link_Info, bool)``  | ``doc.link_info(id) -> Option<LinkInfo>``  |
+--------------------------------------------+--------------------------------------------+
| ``source_entry(v, id)``                    | ``doc.s                                    |
|                                            | ource_entry(id) -> Option<&SourceEntry>``; |
|                                            | ``doc.source_str(id)`` for its text        |
+--------------------------------------------+--------------------------------------------+
| ``source_text(v, span)``                   | ``doc.source_text(span)``                  |
+--------------------------------------------+--------------------------------------------+
| ``plain_text(v, id, &buf)``                | ``doc.plain_text(id, &mut sink)``          |
|                                            | (``String``, ``Vec<u8>``, ``Builder``,     |
|                                            | ``Emitter``)                               |
+--------------------------------------------+--------------------------------------------+
| ``gd.View{snap = b.snap}`` (a view of a    | ``b.document()`` / ``ctx.document()``      |
| builder)                                   |                                            |
+--------------------------------------------+--------------------------------------------+
| ``dump_tree(v, &e)``                       | ``dump_tree(doc, &mut sink)``,             |
|                                            | ``dump_to_string(doc)``                    |
+--------------------------------------------+--------------------------------------------+
| ``kind_name``, ``admonition_name``,        | same functions; ``style.name()``;          |
| ``content_model``, ``LIST_STYLE_NAMES[s]`` | ``child_allowed`` is public too            |
+--------------------------------------------+--------------------------------------------+
| ``media_kind``, ``Media_Kind``             | ``media_kind``, ``MediaKind``              |
+--------------------------------------------+--------------------------------------------+

The tree
~~~~~~~~

+--------------------------------------------+--------------------------------------------+
| Odin ``gd.``                               | Rust                                       |
+============================================+============================================+
| ``Node_Id``, ``Source_Id``                 | ``NodeId(u32)`` (``.index()``, ``+ u32``,  |
|                                            | ``- u32``, ``NodeId::from_index``),        |
|                                            | ``SourceId(u16)`` (``SourceId::MAIN``)     |
+--------------------------------------------+--------------------------------------------+
| ``NONE``, ``INVALID_NODE``, ``ROOT``       | same constants (the sentinels remain only  |
|                                            | inside tables: ``Node::payload``,          |
|                                            | ``LinkInfo::resolved``)                    |
+--------------------------------------------+--------------------------------------------+
| ``Text{start, length}``                    | ``Text { start, len }``                    |
+--------------------------------------------+--------------------------------------------+
| ``Source_Span{source, start, end}``        | ``SourceSpan { source, start, end }``,     |
|                                            | ``SourceSpan::new``                        |
+--------------------------------------------+--------------------------------------------+
| ``Attribute_Range``                        | ``AttributeRange`` (``.range()``)          |
+--------------------------------------------+--------------------------------------------+
| ``Node``                                   | ``Node`` (same fields)                     |
+--------------------------------------------+--------------------------------------------+
| ``Node_Kind`` (``.Block_Quote``)           | ``NodeKind::BlockQuote`` (all variants     |
|                                            | CamelCase; ``NodeKind::ALL``)              |
+--------------------------------------------+--------------------------------------------+
| ``Node_Flag``, ``Node_Flags``,             | ``NodeFlag``, ``NodeFlags``;               |
| ``{.Tight}``, ``.Tight in f``,             | ``NodeFlags::TIGHT``,                      |
| ``f += {.Auto}``                           | ``f.has(NodeFlag::Tight)``,                |
|                                            | ``f.insert(NodeFlags::AUTO)`` or ``|``     |
+--------------------------------------------+--------------------------------------------+
| ``Document_Reading``, ``Admonition_Kind``, | ``DocumentReading``, ``AdmonitionKind``    |
| ``List_Style``, ``List_Info``,             | (``from_value``), ``ListStyle``,           |
| ``Link_Kind``, ``Link_Info``, ``Align``,   | ``ListInfo``, ``LinkKind``, ``LinkInfo``   |
| ``Cell_Info``, ``Attribute_Key``,          | (``UNRESOLVED``), ``Align``, ``CellInfo``  |
| ``Attribute``, ``Content``                 | (``Default`` 1×1), ``AttributeKey``,       |
|                                            | ``Attribute``, ``Content``                 |
+--------------------------------------------+--------------------------------------------+
| ``Line_View{text, offset}``                | ``LineView<'s> { text, offset }``          |
+--------------------------------------------+--------------------------------------------+
| ``offset_in(outer, inner) -> (int, bool)`` | ``                                         |
|                                            | offset_in(outer, inner) -> Option<usize>`` |
+--------------------------------------------+--------------------------------------------+

Adapters
~~~~~~~~

+--------------------------------------------+--------------------------------------------+
| Odin ``gd.``                               | Rust                                       |
+============================================+============================================+
| ``Reader{id, name,                         | ``Reader {                                 |
| version, conformance, read, read_inline}`` |  id, name, version, conformance, read: Rea |
|                                            | dFn, read_inline: Option<ReadInlineFn> }`` |
|                                            | (a ``const``)                              |
+--------------------------------------------+--------------------------------------------+
| ``Renderer{..., media_type, render}``      | ``Render                                   |
|                                            | er { ..., media_type, render: RenderFn }`` |
+--------------------------------------------+--------------------------------------------+
| ``Read_Proc``, ``Read_Inline_Proc``,       | ``ReadFn = f                               |
| ``Render_Proc``                            | n(&mut ReadContext) -> Result<(), Fail>``, |
|                                            | ``ReadInlineFn``, ``RenderFn``             |
+--------------------------------------------+--------------------------------------------+
| ``Read_Context`` (``ctx.b``,               | ``ReadContext<'a>`` (``ctx.b``,            |
| ``ctx.source``, ``ctx.options``,           | ``ctx.source``, ``ctx.options``,           |
| ``ctx.policy``, ``ctx.limits``,            | ``ctx.policy``, ``ctx.limits``,            |
| ``ctx.reports``, ``ctx.ws.sources[id]``)   | ``ctx.reports()``,                         |
|                                            | ``ctx.source_entry(id)``,                  |
|                                            | ``ctx.source_count()``)                    |
+--------------------------------------------+--------------------------------------------+
| ``source_of(ctx, id)``                     | ``ctx.source_of(id) -> SourceText<'a>``    |
|                                            | (derefs to ``&str``; keep it while         |
|                                            | building)                                  |
+--------------------------------------------+--------------------------------------------+
| ``include_source(ctx, fro                  | ``ctx.include_source(from, name, span      |
| m, name, span, use) -> (Source_Id, bool)`` | , SourceUse::Parsed) -> Option<SourceId>`` |
+--------------------------------------------+--------------------------------------------+
| ``add_source(ctx, name, text)``            | ``ctx.ad                                   |
|                                            | d_source(name, text) -> Option<SourceId>`` |
+--------------------------------------------+--------------------------------------------+
| ``Source_Entry``, ``Source_Use``           | ``SourceEnt                                |
|                                            | ry { name, parent, included_at, usage }``, |
|                                            | ``SourceUse``                              |
+--------------------------------------------+--------------------------------------------+
| ``Render_Context`` (``ctx.view``,          | ``RenderContext<'a>`` (``ctx.doc``,        |
| ``ctx.doc``, ``ctx.out``, ``ctx.options``, | ``ctx.out``, ``ctx.options``,              |
| ``ctx.policy``, ``ctx.limits``,            | ``ctx.policy``, ``ctx.limits``,            |
| ``ctx.reports``)                           | ``ctx.reports()``); ``RenderContext::new`` |
|                                            | for tests                                  |
+--------------------------------------------+--------------------------------------------+
| ``Emitter``, ``Emit_Mode`` (``.Measure``,  | ``Emitter<'a>``,                           |
| ``.Write``)                                | ``EmitMode::{Check, Write}``; renderers    |
|                                            | run once (no measure pass)                 |
+--------------------------------------------+--------------------------------------------+
| ``emit(e, s)``, ``emit_byte``,             | ``e.emit(s)``, ``e.emit_byte(c)``,         |
| ``emit_rune``, ``emit_int``, ``emit_mark`` | ``e.emit_char(c)``, ``e.emit_int(v)``,     |
|                                            | ``e.emit_mark(span)``; ``e.written()``,    |
|                                            | ``e.is_check()``                           |
+--------------------------------------------+--------------------------------------------+
| a local                                    | ``Emitter::new(&mut vec)``; for numbers    |
| ``Emitter{mode = .Write, out = buf[:]}``   | just ``write!(string, "{n}")``             |
+--------------------------------------------+--------------------------------------------+

Building
~~~~~~~~

+--------------------------------------------+--------------------------------------------+
| Odin ``gd.``                               | Rust (``b: &mut Builder``)                 |
+============================================+============================================+
| ``Builder``,                               | ``Builder::new(&mut snap, &mu              |
| ``builder_init(&                           | t reports, &mut open_vec, limits, root)``; |
| b, snap, scratch, reports, limits, root)`` | in tests prefer                            |
|                                            | ``ws.begin_read(&request)?`` which gives a |
|                                            | ``ReadContext``                            |
+--------------------------------------------+--------------------------------------------+
| ``builder_finish(b)``                      | ``b.finish()?`` (the engine calls it after |
|                                            | ``read``)                                  |
+--------------------------------------------+--------------------------------------------+
| ``builder_fail(b, d)``                     | ``return Err(b.fail(d));``                 |
+--------------------------------------------+--------------------------------------------+
| ``builder_capacity(b, region, n)``         | ``return Err(b.capacity(region, n));``     |
+--------------------------------------------+--------------------------------------------+
| ``spend(b, n)``                            | ``b.spend(n)?``                            |
+--------------------------------------------+--------------------------------------------+
| ``current(b)``, ``current_kind(b)``        | ``b.current() -> Option<NodeId>``,         |
|                                            | ``b.current_kind()``; ``b.open_nodes()``   |
|                                            | for the stack                              |
+--------------------------------------------+--------------------------------------------+
| ``BUILDER_OPEN``                           | ``BUILDER_OPEN``                           |
+--------------------------------------------+--------------------------------------------+
| ``begin_node``, ``end_node``,              | ``b.begin_node(k, span)?``,                |
| ``add_leaf``, ``add_text``                 | ``b.end_node(id)?``,                       |
|                                            | ``b.add_leaf(k, span, text)?``,            |
|                                            | ``b.add_text(s)?``;                        |
|                                            | ``b.add_text_leaf(k, span, s)?``           |
+--------------------------------------------+--------------------------------------------+
| ``text_open``, ``text_push``,              | ``b.text_open()``, ``b.text_push(s)?``,    |
| ``text_push_byte``, ``text_push_rune``,    | ``b.text_push_byte(c)?``,                  |
| ``text_close``, ``text_rewind``            | ``b.text_push_char(c)?``,                  |
|                                            | ``b.text_close(start)``,                   |
|                                            | ``b.text_rewind(start)``;                  |
|                                            | ``b.text_push_bytes(&[u8])?``              |
+--------------------------------------------+--------------------------------------------+
| writing into                               | ``normalize_label(b, s)`` /                |
| ``b.snap.text[b.snap.text_used:]`` through | ``make_id(b, s)`` (a ``Builder`` is a      |
| a ``Buffer``                               | ``TextSink``) between ``text_open`` and    |
|                                            | ``text_close``                             |
+--------------------------------------------+--------------------------------------------+
| ``pool_string(b, t)``                      | ``b.pool_string(t)``                       |
+--------------------------------------------+--------------------------------------------+
| ``b.snap.text_used``                       | ``b.text_used()``                          |
+--------------------------------------------+--------------------------------------------+
| ``set_text``, ``set_value``,               | ``b.set_text(id, t)``,                     |
| ``add_flags``, ``set_span_end``,           | ``b.set_value(id, v)``,                    |
| ``set_kind``                               | ``b.add_flags(id, NodeFlags::X)``,         |
|                                            | ``b.set_span_end(id, end)``,               |
|                                            | ``b.set_kind(id, k)``                      |
+--------------------------------------------+--------------------------------------------+
| ``add_attribute(b, id, key, value)`` /     | ``b.add_attribute(id, key, value)?`` /     |
| ``(…, value, name)``                       | ``b.ad                                     |
|                                            | d_attribute_named(id, key, value, name)?`` |
+--------------------------------------------+--------------------------------------------+
| ``add_attribute_string(b, id, key, s)`` /  | ``b.add_attribute_string(id, key, s)?`` /  |
| ``(…, .Custom, value, name)``              | ``                                         |
|                                            | b.add_custom_attribute(id, name, value)?`` |
+--------------------------------------------+--------------------------------------------+
| ``set_list``, ``set_link -> row``,         | ``b.set_list(id, info)?``,                 |
| ``set_cell``                               | ``b.set_link(id, info)? -> u32``,          |
|                                            | ``b.set_cell(id, info)?``                  |
+--------------------------------------------+--------------------------------------------+
| ``                                         | `                                          |
| resolve_link(b, row, target, kind, dest)`` | `b.resolve_link(row, target, kind, dest)`` |
|                                            | (``Text::default()`` keeps the             |
|                                            | destination)                               |
+--------------------------------------------+--------------------------------------------+
| ``unwrap_node(b, id)``                     | ``b.unwrap_node(id)?``                     |
+--------------------------------------------+--------------------------------------------+
| ``copy_subtree(b, v, id)``,                | ``b.copy_subtree(doc, id)?``,              |
| ``copy_payload(b, v, from, to)``           | ``b.copy_payload(doc, from, to)?``         |
+--------------------------------------------+--------------------------------------------+
| ``b.snap.nodes.rows[i]``,                  | ``b.snap.nodes[i]``, ``b.snap.links[r]``,  |
| ``b.snap.links.rows[r]``,                  | ``b.snap.nodes.len()`` (direct edits are   |
| ``b.snap.nodes.used``                      | allowed; validation checks them)           |
+--------------------------------------------+--------------------------------------------+
| ``b.limits``, ``b.status``                 | ``b.limits``, ``b.status()`` /             |
|                                            | ``b.is_ok()``                              |
+--------------------------------------------+--------------------------------------------+
| ``Snapshot``                               | ``Snapshot`` (pub ``Vec`` tables;          |
|                                            | ``text: Vec<u8>``)                         |
+--------------------------------------------+--------------------------------------------+
| ``                                         | ``validate(&snap, &sour                    |
| validate(snap, sources, scratch, limits)`` | ce_lengths, &limits) -> Result<(), Fail>`` |
+--------------------------------------------+--------------------------------------------+

Memory and limits
~~~~~~~~~~~~~~~~~

+--------------------------------------------+--------------------------------------------+
| Odin ``gd.``                               | Rust                                       |
+============================================+============================================+
| ``Scratch``, ``Scratch_Mark``,             | reusable ``Vec``/``String`` fields in your |
| ``scratch_mark``, ``scratch_release``,     | reader or renderer state (``clear()``      |
| ``scratch_take``, ``scratch_buffer``       | keeps capacity)                            |
+--------------------------------------------+--------------------------------------------+
| ``Stack(T)``, ``push``, ``pop``, ``top``,  | ``Vec<T>``: ``push``, ``pop() -> Option``, |
| ``stack_slice``                            | ``last_mut()``, ``as_slice()`` (bound      |
|                                            | depth with ``Limits::max_depth``)          |
+--------------------------------------------+--------------------------------------------+
| ``Buffer``, ``buffer_write``,              | ``String``: ``push_str``, ``push``,        |
| ``buffer_write_byte``, ``buffer_string``,  | ``as_str()``; no overflow (bound with      |
| ``.overflow``                              | limits where input drives growth)          |
+--------------------------------------------+--------------------------------------------+
| ``Stack_Guard``, ``stack_guard(limits)``,  | ``StackGuard``, ``stack_guard(&limits)``,  |
| ``stack_exhausted(g)``,                    | ``g.is_exhausted()``,                      |
| ``stack_diagnostic(g, span)``              | ``g.diagnostic(span)`` (free functions     |
|                                            | kept too)                                  |
+--------------------------------------------+--------------------------------------------+
| ``DEFAULT_STACK_BYTES``, ``STACK_RESERVE`` | same                                       |
+--------------------------------------------+--------------------------------------------+
| ``Symbol_Table``,                          | ``SymbolTable::with_capacity(n)``          |
| ``symbol_table_init(&t, scratch, n)``      |                                            |
+--------------------------------------------+--------------------------------------------+
| ``sym                                      | ``t.                                       |
| bol_put(&t, k, v) -> (existing, status)``, | insert(k, v) -> Result<(), SymbolClash>``; |
| ``Symbol_Status``                          | ``                                         |
|                                            | SymbolClash::{Duplicate(existing), Full}`` |
+--------------------------------------------+--------------------------------------------+
| ``symbol_get(&t, k) -> (v, bool)``,        | ``t.get(k) -> Option<u32>``,               |
| ``symbol_set``                             | ``t.set(k, v) -> bool``                    |
+--------------------------------------------+--------------------------------------------+

Text
~~~~

+--------------------------------------------+--------------------------------------------+
| Odin ``gd.``                               | Rust                                       |
+============================================+============================================+
| ``encode_rune(r)``                         | ``c.encode_utf8(&mut [0; 4])``             |
+--------------------------------------------+--------------------------------------------+
| ``decode_rune(s, i)``,                     | same, ``-> (char, usize)`` (U+FFFD, 1 off  |
| ``decode_last_rune(s, end)``               | a character boundary)                      |
+--------------------------------------------+--------------------------------------------+
| ``is_ascii_space``, ``is_ascii_punct``,    | same (``u8`` arguments)                    |
| ``is_ascii_alnum``, ``is_ascii_digit``     |                                            |
+--------------------------------------------+--------------------------------------------+
| ``is_unicode_whitespace``,                 | same (``char``); plus ``is_letter``,       |
| ``is_unicode_punctuation``                 | ``is_decimal_digit``, ``to_lower``,        |
|                                            | ``fold_rune`` (Odin's ``core:unicode``     |
|                                            | calls)                                     |
+--------------------------------------------+--------------------------------------------+
| ``normalize_label(&buf, s)``               | ``normalize_label(&mut sink, s)``          |
+--------------------------------------------+--------------------------------------------+
| ``line_column(text, off)``,                | same, ``-> (usize, usize)``                |
| ``line_bounds(text, off)``                 |                                            |
+--------------------------------------------+--------------------------------------------+
| ``valid_utf8`` (core-internal)             | ``check_u                                  |
|                                            | tf8(source, bytes) -> Result<&str, Fail>`` |
|                                            | for hosts; ``&str`` is always UTF-8        |
+--------------------------------------------+--------------------------------------------+
| ``normalize_nfd_into(dst, s) -> (n, ok)``, | ``normalize_nfd_into(&mut String, s)``     |
| ``nfd_first``                              | (appends),                                 |
|                                            | ``normalize_nfd(s) -> Cow<str>``,          |
|                                            | ``nfd_first(s) -> char``                   |
+--------------------------------------------+--------------------------------------------+

Anchors
~~~~~~~

+--------------------------------------------+--------------------------------------------+
| Odin ``gd.``                               | Rust                                       |
+============================================+============================================+
| ``Anchors``,                               | ``let a = build_                           |
| ``bu                                       | anchors(ctx.doc, ctx.options, headings);`` |
| ild_anchors(ctx, &a, headings) -> Status`` | (infallible)                               |
+--------------------------------------------+--------------------------------------------+
| ``anchor_of(&a, id) -> (string, bool)``    | ``a.anchor_of(id) -> Option<&str>``        |
+--------------------------------------------+--------------------------------------------+
| ``anchor_ids``/``next_anchor_id``,         | ``for id in a.anchor_ids(node)``,          |
| ``Anchor_Ids``                             | ``AnchorIds``                              |
+--------------------------------------------+--------------------------------------------+
| ``anchor_extras(&a, owner)``,              | ``a                                        |
| ``extra_anchor(&a, owner, item)``,         | .anchor_extras(owner) -> &[AnchorExtra]``, |
| ``extra_text(&a, e)``, ``Anchor_Extra``    | ``a.ex                                     |
|                                            | tra_anchor(owner, item) -> Option<&str>``, |
|                                            | ``a.extra_text(&e)``, ``AnchorExtra``      |
+--------------------------------------------+--------------------------------------------+
| ``con                                      | ``contents_s                               |
| tents_scope(v, id) -> (first, end, base)`` | cope(doc, id) -> (NodeId, NodeId, usize)`` |
+--------------------------------------------+--------------------------------------------+
| ``make_id(&buf, s)``,                      | ``make_id(&mut sink, s)``,                 |
| ``write_id(&buf, s)``, ``id_fold(r)``      | ``write_id(&mut sink, s)``,                |
|                                            | ``id_fold(c) -> &'static str``             |
+--------------------------------------------+--------------------------------------------+

Host services
~~~~~~~~~~~~~

+--------------------------------------------+--------------------------------------------+
| Odin ``gd.``                               | Rust                                       |
+============================================+============================================+
| ``write_object(v, &e)`` (measure, then     | ``write_object(doc, &mut Vec<u8>)``        |
| write)                                     | (format version 2; Odin caches are not     |
|                                            | readable)                                  |
+--------------------------------------------+--------------------------------------------+
| ``load_o                                   | ``load_object(&mut ws, &b                  |
| bject(&ws, bytes, limits) -> Read_Result`` | ytes, &limits) -> Result<Document, Fail>`` |
|                                            | (bytes are copied)                         |
+--------------------------------------------+--------------------------------------------+
| ``line_column``, ``line_bounds``           | as above; ``ws.source_text(span)``,        |
|                                            | ``ws.describe(&d)``                        |
+--------------------------------------------+--------------------------------------------+
