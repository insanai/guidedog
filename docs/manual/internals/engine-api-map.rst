.. _porting-odin-gd-code-to-the-guidedoc-crate:

Porting Odin ``gd.`` code to the ``guidedoc`` crate
===================================================

Readers, renderers, passes, and hosts written against the Odin core
(``import gd "../../core"``) map onto this crate as below. Every Rust name is exported at
the crate root: ``use guidedoc::{...}`` or ``use guidedoc as gd;`` and write
``gd::NodeKind::Paragraph``.

The Odin code is the specification of behaviour, not a template: keep its output and
diagnostics, but use the Rust forms on the right (``Option`` for absent values, ``Result``
and ``?`` for failures, iterators for traversals, owned ``Vec``/``String`` buffers that we
``clear()`` and reuse instead of scratch).

The ten idioms
--------------

.. list-table::
   :header-rows: 1

   * - Odin
     - Rust
   * - ``p := gd.begin_node(b, .Paragraph, span)``... ``gd.end_node(b, p)``
     - ``let p = b.begin_node(NodeKind::Paragraph, span)?;`` ... ``b.end_node(p)?;`` or ``b.with_node(NodeKind::Paragraph, span, |b, p| { ...; Ok(()) })?``
   * - ``gd.add_leaf(b, .Text, span, gd.add_text(b, s))``
     - ``b.add_text_leaf(NodeKind::Text, span, s)?`` (two ``&mut`` calls cannot nest)
   * - ``start := gd.text_open(b); gd.text_push(b, x); ...; t := gd.text_close(b, start)``
     - same calls with ``?`` on pushes, or ``let t = b.build_text(|b| { b.text_push(x)?; Ok(()) })?;``
   * - ``if b.status != .Ok do return b.status`` / ``alive(p)``
     - ``?`` on every fallible builder call; ``b.is_ok()`` where we only test
   * - ``gd.report_message(ctx.reports, M, span, gd.arg_span(s), gd.arg_static("x"))``
     - ``ctx.report(M, span, arg_span(s), arg_static("x"))`` (``Arg::None`` for a missing argument)
   * - ``for child in gd.next_child(v, &it)`` after ``it := gd.children(v, id)``
     - ``for child in doc.children(id)``
   * - ``v.snap.nodes.rows[id]``, ``.used``
     - ``doc.node(id)`` / ``b.snap.nodes[id]`` (indexable by ``NodeId``), ``.len()``
   * - ``x := gd.first_child(v, id); if x != gd.INVALID_NODE``
     - ``if let Some(x) = doc.first_child(id)``
   * - ``l, _ := gd.link_info(v, id)``
     - ``doc.link_info(id).unwrap_or(LinkInfo::UNRESOLVED)`` (or match the ``Option``)
   * - ``mark := gd.scratch_mark(ctx.scratch)`` ... ``gd.scratch_take(ctx.scratch, T, n)``
     - a ``Vec<T>`` in our state struct, ``clear()``\ ed and refilled; ``Vec::with_capacity(n)`` when sizing

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

.. list-table::
   :header-rows: 1

   * - Odin ``gd.``
     - Rust
   * - ``Workspace``, ``init_workspace``, ``init_workspace_from_bytes``, ``init_workspace_with_plan``, ``Storage``, ``Snapshot_Storage``, ``Slab_Plan``, ``slab_plan``, ``plan_bytes``, ``workspace_size``, ``Table``
     - ``Workspace::new()`` / ``Workspace::with_limits(limits)``; storage grows on demand and is reused, bounded by ``Limits``. No sizing step.
   * - ``reset_workspace(&ws)``
     - ``ws.reset()``
   * - ``Request``, ``default_request(name, text, reader, renderer)``
     - ``Request<'a>`` (``Default``), ``default_request(name, text, reader, renderer)``
   * - ``Source{name, text}``
     - ``Source { name, text }`` (borrowed ``&str``\ s); ``request.source.text``
   * - ``Read_Options``, ``default_read_options()``
     - ``ReadOptions<'a>`` (``Default`` = ``default_read_options()``)
   * - ``Read_Extension``, ``read_extension(&s)``
     - ``ReadExtension<'a>``, ``read_extension(&s)`` (``T: 'static``)
   * - ``extension_as(e, T)``
     - ``e.downcast::<T>() -> Option<&T>``
   * - ``reader_settings(ctx, T) -> (^T, bool)``
     - ``ctx.reader_settings::<T>()? -> Option<&T>`` (``None``: defaults)
   * - ``no_reader_settings(ctx)``
     - ``ctx.no_reader_settings()?``
   * - ``Render_Options``
     - ``RenderOptions<'a>`` (``Default``: standalone)
   * - ``Math_Style``
     - ``MathStyle``
   * - ``Label``, ``Labels``, ``ENGLISH_LABELS``
     - ``Label`` (CamelCase variants: ``See_Also`` becomes ``SeeAlso``), ``Labels`` (``set``/``get``), ``ENGLISH_LABELS[l as usize]``
   * - ``label(o, l)``, ``admonition_label(o, k)``, ``split_label(s)``
     - ``label(&o, l)``, ``admonition_label(&o, k)``, ``split_label(s) -> (&str, &str)``
   * - ``Highlighter{procedure, user}``
     - ``trait Highlighter { fn highlight(&self, language, code, out: &mut Emitter) -> bool }``; ``options.highlighter: Option<&dyn Highlighter>``
   * - ``Policy``, ``Content_Policy``, ``default_policy()``, ``trusted_policy()``
     - ``Policy`` (``Default`` = ``default_policy()``), ``ContentPolicy``, same functions
   * - ``Limits``, ``default_limits()``
     - ``Limits`` (``Default``), adds ``max_text_bytes``, ``max_reports``, ``max_output_bytes``
   * - ``Resource_Provider{procedure, user}``
     - ``trait ResourceProvider { fn provide(&self, from: &str, name: &str) -> Result<Resource, ResourceStatus> }``; ``Resource { text: Arc<str>, canonical }``
   * - ``Resource_Status``
     - ``ResourceStatus::{Missing, Refused, Limit, NotUtf8}`` (no ``Ok``: the ``Result`` says it)
   * - ``Cancel_Probe``
     - ``request.cancel: Option<&dyn Fn() -> bool>``
   * - ``Pass{name, procedure, user}``, ``Pass_Outcome``
     - ``Pass { name, run: &closure }`` where the closure is ``Fn(Document, &mut Builder) -> PassOutcome``; ``PassOutcome``
   * - ``Map_Entry``, ``request.source_map`` slice
     - ``MapEntry``; ``request.source_map: bool``, read ``ws.source_map()``
   * - ``convert(&ws, registry, request, output) -> Result``
     - ``convert(&mut ws, &registry, &request) -> Result<&str, Fail>`` (output borrows ``ws``)
   * - ``convert_text(&ws, registry, reader, renderer, text)``
     - ``convert_text(&mut ws, &registry, reader, renderer, text) -> Result<&str, Fail>``
   * - ``read_document(&ws, reader, request) -> Read_Result``
     - ``read_document(&mut ws, &reader, &request) -> Result<Document<'_>, Fail>``
   * - ``transform_document(&ws, doc, passes, limits)``
     - ``transform_document(&mut ws, &passes, &limits) -> Result<Document<'_>, Fail>`` (works on the current document)
   * - ``check_render(&ws, doc, renderer, request)``
     - ``check_render(&mut ws, &renderer, &request) -> Result<usize, Fail>`` (exact size)
   * - ``render_document(&ws, doc, renderer, request, output)``
     - ``render_document(&mut ws, &renderer, &request) -> Result<&str, Fail>``
   * - ``Document``, ``document_valid``, ``view(doc) -> (View, bool)``, ``View``
     - ``Document<'w>`` *is* the view; get it from a stage's result or ``ws.document() -> Option<Document>``. Stale handles do not compile.
   * - ``Registry{readers, renderers}``
     - ``Registry { readers: Vec<Reader>, renderers: Vec<Renderer> }``
   * - ``find_reader(r, id)``, ``find_renderer(r, id)``
     - ``r.find_reader(id) -> Option<&Reader>``, ``r.find_renderer(id)``
   * - ``validate_registry(r)``
     - ``r.validate()`` / ``validate_registry(&r) -> Result<(), Fail>``
   * - ``Conformance``
     - ``Conformance``
   * - ``VERSION``, ``SCHEMA_VERSION``
     - ``VERSION`` (the workspace version), ``SCHEMA_VERSION``

Results and diagnostics
~~~~~~~~~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1

   * - Odin ``gd.``
     - Rust
   * - ``Status`` (``.Invalid_Input``)
     - ``Status::InvalidInput``, etc.
   * - ``Result``, ``Read_Result`` (``status``, ``primary``, ``need``, ``written``, ``reports``)
     - ``Result<_, Fail>``; ``Fail { status, need, diagnostic: Box<Diagnostic> }``, ``fail.code()``; reports via ``ws.reports()``
   * - ``Region``, ``Capacity_Need``
     - ``Region`` (no ``Second_Bank``), ``CapacityNeed { region, minimum, exact }``
   * - ``Message``
     - ``Message`` (same fields, ``const``)
   * - ``Diagnostic`` (``d.code``, ``d.args``)
     - ``Diagnostic`` (same flat fields; ``args: [Arg; 2]``), ``d.formatted_message()``, ``ws.describe(&d)`` quotes spans
   * - ``Diagnostic_Arg``, ``Arg_Kind``
     - ``Arg::{None, Int, Span, Text(Cow<'static, str>)}``
   * - ``diagnostic(m, span, a, b)``
     - ``diagnostic(m, span, a, b)`` (pass ``Arg::None`` for absent ones)
   * - ``arg_int``, ``arg_span``, ``arg_static``
     - same; ``arg_text(&str)`` copies non-static text (Odin passed borrowed strings to ``arg_static``)
   * - ``Severity``
     - ``Severity``
   * - ``Reports``, ``report(r, d)``, ``report_message(r, m, span, a, b)``
     - ``Reports``; ``r.report(d)``, ``r.message(m, span, a, b)``; from adapters ``ctx.report(...)`` / ``ctx.reports()`` / ``b.report(...)``
   * - ``reports.muted = true`` ... restore
     - ``let was = ctx.reports().set_muted(true);`` ... ``ctx.reports().set_muted(was);``
   * - ``reports.errors``, ``.warnings``, ``.primary``, ``.items[:used]``
     - ``error_count()``, ``warning_count()``, ``primary() -> Option<&Diagnostic>``, ``items()``
   * - ``first_problem(r)``
     - ``r.first_problem()``
   * - ``content_error(status)``, ``kept_message(policy, m)``
     - ``content_error(status)``, ``kept_message(&policy, m)``
   * - ``CORE_CAPACITY``, ``CORE_WORK``, ``CORE_DEPTH``, ``CORE_STACK``, ``CORE_NODES``, ``CORE_UNHANDLED_KIND``, ``INCLUDE_MISSING``
     - same constants (``CORE_UTF8`` too)

.. _reading-a-document-view--document:

Reading a document (``View`` to ``Document``)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1

   * - Odin ``gd.`` (``v`` is a View)
     - Rust (``doc: Document``)
   * - ``node_count(v)``, ``node(v, id)``, ``kind(v, id)``
     - ``doc.node_count()``, ``doc.node(id) -> &Node``, ``doc.kind(id)``
   * - ``subtree_end(v, id)``
     - ``doc.subtree_end(id) -> NodeId``
   * - ``text(v, t)``, ``node_text(v, id)``
     - ``doc.text(t)``, ``doc.node_text(id)``
   * - ``has_children``, ``first_child``, ``code_caption``
     - ``doc.has_children(id)``, ``doc.first_child(id) -> Option<NodeId>``, ``doc.code_caption(id) -> Option<NodeId>``
   * - ``children``/``next_child``, ``Children``, ``child_count``
     - ``doc.children(id)`` (iterator), ``Children``, ``doc.child_count(id)``
   * - ``parent_of(v, id)``
     - ``doc.parent_of(id) -> Option<NodeId>``
   * - ``attribute(v, id, key) -> (string, bool)``
     - ``doc.attribute(id, key) -> Option<&str>``
   * - ``custom_attribute(v, id, name)``
     - ``doc.custom_attribute(id, name) -> Option<&str>``
   * - ``attributes``/``next_attribute``, ``Attributes``
     - ``doc.attributes(id) -> &[Attribute]``
   * - ``has_class(v, id, c)``
     - ``doc.has_class(id, c)``
   * - ``list_info``, ``cell_info``
     - ``doc.list_info(id)``, ``doc.cell_info(id)`` (defaults when absent)
   * - ``link_info(v, id) -> (Link_Info, bool)``
     - ``doc.link_info(id) -> Option<LinkInfo>``
   * - ``source_entry(v, id)``
     - ``doc.source_entry(id) -> Option<&SourceEntry>``; ``doc.source_str(id)`` for its text
   * - ``source_text(v, span)``
     - ``doc.source_text(span)``
   * - ``plain_text(v, id, &buf)``
     - ``doc.plain_text(id, &mut sink)`` (``String``, ``Vec<u8>``, ``Builder``, ``Emitter``)
   * - ``gd.View{snap = b.snap}`` (a view of a builder)
     - ``b.document()`` / ``ctx.document()``
   * - ``dump_tree(v, &e)``
     - ``dump_tree(doc, &mut sink)``, ``dump_to_string(doc)``
   * - ``kind_name``, ``admonition_name``, ``content_model``, ``LIST_STYLE_NAMES[s]``
     - same functions; ``style.name()``; ``child_allowed`` is public too
   * - ``media_kind``, ``Media_Kind``
     - ``media_kind``, ``MediaKind``

The tree
~~~~~~~~

.. list-table::
   :header-rows: 1

   * - Odin ``gd.``
     - Rust
   * - ``Node_Id``, ``Source_Id``
     - ``NodeId(u32)`` (``.index()``, ``+ u32``, ``- u32``, ``NodeId::from_index``), ``SourceId(u16)`` (``SourceId::MAIN``)
   * - ``NONE``, ``INVALID_NODE``, ``ROOT``
     - same constants (the sentinels remain only inside tables: ``Node::payload``, ``LinkInfo::resolved``)
   * - ``Text{start, length}``
     - ``Text { start, len }``
   * - ``Source_Span{source, start, end}``
     - ``SourceSpan { source, start, end }``, ``SourceSpan::new``
   * - ``Attribute_Range``
     - ``AttributeRange`` (``.range()``)
   * - ``Node``
     - ``Node`` (same fields)
   * - ``Node_Kind`` (``.Block_Quote``)
     - ``NodeKind::BlockQuote`` (all variants CamelCase; ``NodeKind::ALL``)
   * - ``Node_Flag``, ``Node_Flags``, ``{.Tight}``, ``.Tight in f``, ``f += {.Auto}``
     - ``NodeFlag``, ``NodeFlags``; ``NodeFlags::TIGHT``, ``f.has(NodeFlag::Tight)``, ``f.insert(NodeFlags::AUTO)`` or ``|``
   * - ``Document_Reading``, ``Admonition_Kind``, ``List_Style``, ``List_Info``, ``Link_Kind``, ``Link_Info``, ``Align``, ``Cell_Info``, ``Attribute_Key``, ``Attribute``, ``Content``
     - ``DocumentReading``, ``AdmonitionKind`` (``from_value``), ``ListStyle``, ``ListInfo``, ``LinkKind``, ``LinkInfo`` (``UNRESOLVED``), ``Align``, ``CellInfo`` (``Default`` 1x1), ``AttributeKey``, ``Attribute``, ``Content``
   * - ``Line_View{text, offset}``
     - ``LineView<'s> { text, offset }``
   * - ``offset_in(outer, inner) -> (int, bool)``
     - ``offset_in(outer, inner) -> Option<usize>``

Adapters
~~~~~~~~

.. list-table::
   :header-rows: 1

   * - Odin ``gd.``
     - Rust
   * - ``Reader{id, name, version, conformance, read, read_inline}``
     - ``Reader { id, name, version, conformance, read: ReadFn, read_inline: Option<ReadInlineFn> }`` (a ``const``)
   * - ``Renderer{..., media_type, render}``
     - ``Renderer { ..., media_type, render: RenderFn }``
   * - ``Read_Proc``, ``Read_Inline_Proc``, ``Render_Proc``
     - ``ReadFn = fn(&mut ReadContext) -> Result<(), Fail>``, ``ReadInlineFn``, ``RenderFn``
   * - ``Read_Context`` (``ctx.b``, ``ctx.source``, ``ctx.options``, ``ctx.policy``, ``ctx.limits``, ``ctx.reports``, ``ctx.ws.sources[id]``)
     - ``ReadContext<'a>`` (``ctx.b``, ``ctx.source``, ``ctx.options``, ``ctx.policy``, ``ctx.limits``, ``ctx.reports()``, ``ctx.source_entry(id)``, ``ctx.source_count()``)
   * - ``source_of(ctx, id)``
     - ``ctx.source_of(id) -> SourceText<'a>`` (derefs to ``&str``; keep it while building)
   * - ``include_source(ctx, from, name, span, use) -> (Source_Id, bool)``
     - ``ctx.include_source(from, name, span, SourceUse::Parsed) -> Option<SourceId>``
   * - ``add_source(ctx, name, text)``
     - ``ctx.add_source(name, text) -> Option<SourceId>``
   * - ``Source_Entry``, ``Source_Use``
     - ``SourceEntry { name, parent, included_at, usage }``, ``SourceUse``
   * - ``Render_Context`` (``ctx.view``, ``ctx.doc``, ``ctx.out``, ``ctx.options``, ``ctx.policy``, ``ctx.limits``, ``ctx.reports``)
     - ``RenderContext<'a>`` (``ctx.doc``, ``ctx.out``, ``ctx.options``, ``ctx.policy``, ``ctx.limits``, ``ctx.reports()``); ``RenderContext::new`` for tests
   * - ``Emitter``, ``Emit_Mode`` (``.Measure``, ``.Write``)
     - ``Emitter<'a>``, ``EmitMode::{Check, Write}``; renderers run once (no measure pass)
   * - ``emit(e, s)``, ``emit_byte``, ``emit_rune``, ``emit_int``, ``emit_mark``
     - ``e.emit(s)``, ``e.emit_byte(c)``, ``e.emit_char(c)``, ``e.emit_int(v)``, ``e.emit_mark(span)``; ``e.written()``, ``e.is_check()``
   * - a local ``Emitter{mode = .Write, out = buf[:]}``
     - ``Emitter::new(&mut vec)``; for numbers just ``write!(string, "{n}")``

Building
~~~~~~~~

.. list-table::
   :header-rows: 1

   * - Odin ``gd.``
     - Rust (``b: &mut Builder``)
   * - ``Builder``, ``builder_init(&b, snap, scratch, reports, limits, root)``
     - ``Builder::new(&mut snap, &mut reports, &mut open_vec, limits, root)``; in tests prefer ``ws.begin_read(&request)?`` which gives a ``ReadContext``
   * - ``builder_finish(b)``
     - ``b.finish()?`` (the engine calls it after ``read``)
   * - ``builder_fail(b, d)``
     - ``return Err(b.fail(d));``
   * - ``builder_capacity(b, region, n)``
     - ``return Err(b.capacity(region, n));``
   * - ``spend(b, n)``
     - ``b.spend(n)?``
   * - ``current(b)``, ``current_kind(b)``
     - ``b.current() -> Option<NodeId>``, ``b.current_kind()``; ``b.open_nodes()`` for the stack
   * - ``BUILDER_OPEN``
     - ``BUILDER_OPEN``
   * - ``begin_node``, ``end_node``, ``add_leaf``, ``add_text``
     - ``b.begin_node(k, span)?``, ``b.end_node(id)?``, ``b.add_leaf(k, span, text)?``, ``b.add_text(s)?``; ``b.add_text_leaf(k, span, s)?``
   * - ``text_open``, ``text_push``, ``text_push_byte``, ``text_push_rune``, ``text_close``, ``text_rewind``
     - ``b.text_open()``, ``b.text_push(s)?``, ``b.text_push_byte(c)?``, ``b.text_push_char(c)?``, ``b.text_close(start)``, ``b.text_rewind(start)``; ``b.text_push_bytes(&[u8])?``
   * - writing into ``b.snap.text[b.snap.text_used:]`` through a ``Buffer``
     - ``normalize_label(b, s)`` / ``make_id(b, s)`` (a ``Builder`` is a ``TextSink``) between ``text_open`` and ``text_close``
   * - ``pool_string(b, t)``
     - ``b.pool_string(t)``
   * - ``b.snap.text_used``
     - ``b.text_used()``
   * - ``set_text``, ``set_value``, ``add_flags``, ``set_span_end``, ``set_kind``
     - ``b.set_text(id, t)``, ``b.set_value(id, v)``, ``b.add_flags(id, NodeFlags::X)``, ``b.set_span_end(id, end)``, ``b.set_kind(id, k)``
   * - ``add_attribute(b, id, key, value)`` / ``(..., value, name)``
     - ``b.add_attribute(id, key, value)?`` / ``b.add_attribute_named(id, key, value, name)?``
   * - ``add_attribute_string(b, id, key, s)`` / ``(..., .Custom, value, name)``
     - ``b.add_attribute_string(id, key, s)?`` / ``b.add_custom_attribute(id, name, value)?``
   * - ``set_list``, ``set_link -> row``, ``set_cell``
     - ``b.set_list(id, info)?``, ``b.set_link(id, info)? -> u32``, ``b.set_cell(id, info)?``
   * - ``resolve_link(b, row, target, kind, dest)``
     - ``b.resolve_link(row, target, kind, dest)`` (``Text::default()`` keeps the destination)
   * - ``unwrap_node(b, id)``
     - ``b.unwrap_node(id)?``
   * - ``copy_subtree(b, v, id)``, ``copy_payload(b, v, from, to)``
     - ``b.copy_subtree(doc, id)?``, ``b.copy_payload(doc, from, to)?``
   * - ``b.snap.nodes.rows[i]``, ``b.snap.links.rows[r]``, ``b.snap.nodes.used``
     - ``b.snap.nodes[i]``, ``b.snap.links[r]``, ``b.snap.nodes.len()`` (direct edits are allowed; validation checks them)
   * - ``b.limits``, ``b.status``
     - ``b.limits``, ``b.status()`` / ``b.is_ok()``
   * - ``Snapshot``
     - ``Snapshot`` (pub ``Vec`` tables; ``text: Vec<u8>``)
   * - ``validate(snap, sources, scratch, limits)``
     - ``validate(&snap, &source_lengths, &limits) -> Result<(), Fail>``

Memory and limits
~~~~~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1

   * - Odin ``gd.``
     - Rust
   * - ``Scratch``, ``Scratch_Mark``, ``scratch_mark``, ``scratch_release``, ``scratch_take``, ``scratch_buffer``
     - reusable ``Vec``/``String`` fields in our reader or renderer state (``clear()`` keeps capacity)
   * - ``Stack(T)``, ``push``, ``pop``, ``top``, ``stack_slice``
     - ``Vec<T>``: ``push``, ``pop() -> Option``, ``last_mut()``, ``as_slice()`` (bound depth with ``Limits::max_depth``)
   * - ``Buffer``, ``buffer_write``, ``buffer_write_byte``, ``buffer_string``, ``.overflow``
     - ``String``: ``push_str``, ``push``, ``as_str()``; no overflow (bound with limits where input drives growth)
   * - ``Stack_Guard``, ``stack_guard(limits)``, ``stack_exhausted(g)``, ``stack_diagnostic(g, span)``
     - ``StackGuard``, ``stack_guard(&limits)``, ``g.is_exhausted()``, ``g.diagnostic(span)`` (free functions kept too)
   * - ``DEFAULT_STACK_BYTES``, ``STACK_RESERVE``
     - same
   * - ``Symbol_Table``, ``symbol_table_init(&t, scratch, n)``
     - ``SymbolTable::with_capacity(n)``
   * - ``symbol_put(&t, k, v) -> (existing, status)``, ``Symbol_Status``
     - ``t.insert(k, v) -> Result<(), SymbolClash>``; ``SymbolClash::{Duplicate(existing), Full}``
   * - ``symbol_get(&t, k) -> (v, bool)``, ``symbol_set``
     - ``t.get(k) -> Option<u32>``, ``t.set(k, v) -> bool``

Text
~~~~

.. list-table::
   :header-rows: 1

   * - Odin ``gd.``
     - Rust
   * - ``encode_rune(r)``
     - ``c.encode_utf8(&mut [0; 4])``
   * - ``decode_rune(s, i)``, ``decode_last_rune(s, end)``
     - same, ``-> (char, usize)`` (U+FFFD, 1 off a character boundary)
   * - ``is_ascii_space``, ``is_ascii_punct``, ``is_ascii_alnum``, ``is_ascii_digit``
     - same (``u8`` arguments)
   * - ``is_unicode_whitespace``, ``is_unicode_punctuation``
     - same (``char``); plus ``is_letter``, ``is_decimal_digit``, ``to_lower``, ``fold_rune`` (Odin's ``core:unicode`` calls)
   * - ``normalize_label(&buf, s)``
     - ``normalize_label(&mut sink, s)``
   * - ``line_column(text, off)``, ``line_bounds(text, off)``
     - same, ``-> (usize, usize)``
   * - ``valid_utf8`` (core-internal)
     - ``check_utf8(source, bytes) -> Result<&str, Fail>`` for hosts; ``&str`` is always UTF-8
   * - ``normalize_nfd_into(dst, s) -> (n, ok)``, ``nfd_first``
     - ``normalize_nfd_into(&mut String, s)`` (appends), ``normalize_nfd(s) -> Cow<str>``, ``nfd_first(s) -> char``

Anchors
~~~~~~~

.. list-table::
   :header-rows: 1

   * - Odin ``gd.``
     - Rust
   * - ``Anchors``, ``build_anchors(ctx, &a, headings) -> Status``
     - ``let a = build_anchors(ctx.doc, ctx.options, headings);`` (infallible)
   * - ``anchor_of(&a, id) -> (string, bool)``
     - ``a.anchor_of(id) -> Option<&str>``
   * - ``anchor_ids``/``next_anchor_id``, ``Anchor_Ids``
     - ``for id in a.anchor_ids(node)``, ``AnchorIds``
   * - ``anchor_extras(&a, owner)``, ``extra_anchor(&a, owner, item)``, ``extra_text(&a, e)``, ``Anchor_Extra``
     - ``a.anchor_extras(owner) -> &[AnchorExtra]``, ``a.extra_anchor(owner, item) -> Option<&str>``, ``a.extra_text(&e)``, ``AnchorExtra``
   * - ``contents_scope(v, id) -> (first, end, base)``
     - ``contents_scope(doc, id) -> (NodeId, NodeId, usize)``
   * - ``make_id(&buf, s)``, ``write_id(&buf, s)``, ``id_fold(r)``
     - ``make_id(&mut sink, s)``, ``write_id(&mut sink, s)``, ``id_fold(c) -> &'static str``

Host services
~~~~~~~~~~~~~

.. list-table::
   :header-rows: 1

   * - Odin ``gd.``
     - Rust
   * - ``write_object(v, &e)`` (measure, then write)
     - ``write_object(doc, &mut Vec<u8>)`` (format version 2; Odin caches are not readable)
   * - ``load_object(&ws, bytes, limits) -> Read_Result``
     - ``load_object(&mut ws, &bytes, &limits) -> Result<Document, Fail>`` (bytes are copied)
   * - ``line_column``, ``line_bounds``
     - as above; ``ws.source_text(span)``, ``ws.describe(&d)``
