gd-gettext: design for large catalogs
=====================================

This note covers how gd-gettext handles large translation tables (hundreds of thousands of
messages, tens of MB): what other gettext implementations do, what this crate does and why,
and what it costs, measured.

What others do
--------------

**GNU gettext** (`MO
format <https://www.gnu.org/software/gettext/manual/html_node/MO-Files.html>`__, glibc
`loadmsgcat.c <https://github.com/bminor/glibc/blob/master/intl/loadmsgcat.c>`__,
`dcigettext.c <https://github.com/bminor/glibc/blob/master/intl/dcigettext.c>`__)

-  The MO file holds sorted original and translated tables of (length, offset) pairs,
   NUL-terminated strings, and an optional hash table. The manual says the table is sorted
   "to make binary search possible", and that the hash table resolves collisions "by double
   hashing". Each slot holds ``index + 1``; 0 means empty.
-  Lookup with a table: ``hash = hashpjw(msgid)``, ``idx = hash % size``,
   ``incr = 1 + hash % (size - 2)``. Probing continues until a slot is empty or the key
   matches. Without a table it binary-searches the sorted originals. The key is a C string,
   so ``strcmp`` stops at the NUL before msgid_plural: a plural entry is found by its
   singular msgid, and msgfmt hashes only that part.
-  Loading ``mmap``\ s the file (with a ``malloc``\ +\ ``read`` fallback) and serves
   pointers into it, so nothing is copied. It checks the magic number and the major
   revision. It does **not** check that table offsets lie inside the file. Revision 1 adds
   "system-dependent strings" (``<PRIu64>`` segments); glibc expands those into a second
   in-memory table at load.
-  Global state: ``textdomain``/``bindtextdomain``/``setlocale`` are process-wide.

**musl**
`__mo_lookup <https://git.musl-libc.org/cgit/musl/tree/src/locale/__mo_lookup.c>`__ runs
a binary search directly on the mapped file. It has no hash table and no allocations, but it
bounds-checks every probe (offset, length, NUL terminator) on every lookup, because nothing
was validated at load.

**Rust crates**

-  `gettext <https://github.com/justinas/gettext>`__ (pure Rust, justinas) parses an MO
   file into owned ``String``\ s in a ``HashMap`` keyed by ``ctx\x04id``. It ignores the
   hash table and decodes every string at load (via ``encoding``). Plurals come from the
   header. That makes one allocation per message and copies the whole file. Its offset
   arithmetic is unchecked on 32-bit targets.
-  ```gettext-rs``/``gettext-sys`` <https://docs.rs/gettext-rs/latest/gettextrs/>`__ bind
   libintl: fast (mmap plus hash) but global, LGPL, and C-only (its docs say it "CAN'T
   HANDLE NON-UTF-8 DATA").
-  `polib <https://docs.rs/polib>`__ and `rspolib <https://docs.rs/rspolib>`__ build
   mutable catalogs from PO and MO, whole-file and owned. They are good for editing but not
   built for serving lookups.
-  `tr <https://docs.rs/tr/latest/tr/>`__ and
   `i18n-embed <https://docs.rs/i18n-embed/latest/i18n_embed/>`__ focus on ergonomics: a
   ``tr!`` macro with plural syntax, a ``Translator`` trait with swappable backends, and
   language negotiation with fallback chains.
   `fluent-bundle <https://docs.rs/fluent-bundle/latest/fluent_bundle/>`__ gathers
   formatting errors into a list instead of failing.

**Takeaways:** serve strings from one buffer instead of copying them; use the file's own
hash table, which gives O(1) for free when present; validate once instead of on every probe
(musl) or never (glibc); keep the lookup behind a trait so editable and compiled catalogs
are interchangeable (``tr``, ``i18n-embed``); avoid global state.

Design
------

``MoCatalog``: one buffer, checked once, kept as a text pool
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

``MoCatalog::load(path, &LoadOptions)`` reads the file once into one ``Vec<u8>``;
``from_bytes`` takes a buffer the caller already has. Everything is checked **up front, in
linear time** (n log n for the overlap check), so every later lookup is infallible and needs
no ``unsafe``:

-  the header (magic in either byte order, major revision ≤ 1, a count that fits the file)
   and both tables lie in the file;
-  every string lies in the file, ends in NUL, and is UTF-8;
-  no two strings share bytes. The old damaged-file refusal is kept: a table pointing every
   entry at one large string would make a small file cost like a large one;
-  keys are distinct (as C strings), and sorted, or else an order is built (below);
-  the hash table, if present, must be **sound**: every key is reached from its own hash
   within ``MAX_HASH_PROBES`` (64) probes and before an empty slot. The most probes any key
   takes is recorded, and a lookup of a missing key stops there. That bounds every lookup
   even for a hostile table, and checking costs at most 64 probes per key. A table that
   fails is ignored (sysdep entries, foreign hash functions, corruption); it does not make
   the file an error.

The search is an enum, ``MoSearch``:

-  ``Hash { size, offset, probes }``: GNU's hashpjw with double hashing (O(1)), used for
   every msgfmt file.
-  ``Sorted``: binary search on the original table (O(log n)), used for
   ``msgfmt --no-hash``.
-  ``Permuted(order)``: binary search through an order built at load, for a file whose table
   is not sorted.

Contexts (``ctx \x04 id``) are compared without building the key: the stored bytes are
compared slice by slice against ``ctx``, ``\x04``, and ``id``, and the hash is computed the
same way. Plural forms stay NUL-joined in the pool; ``Forms`` splits them only when asked.
The plural rule is compiled once, at load.

**UTF-8, checked once, never again:** after the checks, the bytes the lookups need are
copied out of the buffer: a (start, length) span per string (8 bytes each) and the hash
table's slots. Then the buffer becomes the **text pool** (``mo_raw::into_pool``). In a file
msgfmt wrote, the strings fill the end of the file, NUL-separated, so the region from the
first string to the last NUL is UTF-8 as a whole. ``Vec::truncate`` and
``Vec::drain(..start)`` move that region to the front of the same allocation, and
``String::from_utf8`` validates it in one linear pass, with no second buffer. The raw header
and tables are gone, and the pool is a ``Box<str>`` (the shrink is a ``realloc`` that the
allocator does in place). If the region holds a gap that is not UTF-8, which no compiler
writes, the strings are copied one by one into a new pool instead (2× for a moment). Every
span lies on a character boundary: each string was valid UTF-8 on its own and is followed by
a NUL. So a lookup slices ``&pool[start..end]`` (through ``str::get``, which checks only the
range) with no UTF-8 check and no ``unsafe``.

The hash probe compares lengths first: a stored key matches only if it ends, or has a NUL,
exactly where the query ends. Most keys a probe meets are turned away without reading their
bytes.

**Why one read, not mmap:** std has no mmap. A catalog is read once, at startup; one
``read_to_end`` into a buffer reserved with ``try_reserve_exact`` to the file's size costs
about one memcpy of the file. glibc's mmap mostly saves that copy and shares pages between
processes, which matters for system-wide catalogs, not for one build. A host that wants mmap
can map the file itself (with a crate such as ``memmap2``) and pass the bytes to
``from_bytes`` after copying, or a future ``MoCatalog<B: AsRef<[u8]>>`` could borrow them.
Nothing in the lookup code depends on the buffer being owned.

**Limits:** ``LoadOptions::max_bytes`` (1 GiB by default) refuses a larger file before
reading it, and again while reading (a pipe or a growing file), with an Elm-style
``TooLarge`` error whose hint names the option. Buffers whose size comes from the input use
``try_reserve``, so an allocation the allocator refuses is a ``Memory`` error, not an abort.

Writing: msgfmt's bytes, streamed
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

``write_mo_to(&mut impl Write, ..)`` writes exactly what GNU msgfmt writes: the test
compiles a fixture and compares it byte for byte with msgfmt 0.23.1. That means sorted keys,
msgfmt's table size (the next odd prime ≥ 4n/3), and the singular-only hash for plural
entries. The old writer hashed the full key, so GNU's lookup of a plural message started at
the wrong slot and could miss it; that is now fixed. Other tools get O(1) lookups from our
files. The stored keys are gathered once into one buffer and sorted by slice comparison;
translations are written straight from the catalog without copying. ``write_mo`` reserves
the exact output size; ``save_mo`` writes through a ``BufWriter``. ``write_po_to`` streams
PO text one entry at a time through one reused ``String``.

PO side: ``PoReader``
~~~~~~~~~~~~~~~~~~~~~

``PoReader<R: BufRead>`` is an iterator of ``Result<Message, Error>``. It reads one line
into a reused buffer, checks it as UTF-8 (with line and column on failure), and yields each
entry when it ends. The header comes first and is checked (charset, Plural-Forms) as it
passes. It holds the current entry, the keys seen so far (for duplicate errors), and its
buffers. ``Stats::add`` counts a file without building a ``Catalog``.
``parse_po``/``load_po`` are now just "collect a ``PoReader`` into a ``Catalog``".

``merge`` still takes whole catalogs. The old translation must be random-access for exact
and fuzzy matching, and the fuzzy search's work budget is shared among all the template's
unmatched msgids, so the template is needed whole too.

One ``Lookup`` trait
~~~~~~~~~~~~~~~~~~~~

``Lookup`` provides gettext, ngettext, pgettext and npgettext (``text``, ``plural``,
``with_context``, ``plural_with_context``, ``translation``) from two required methods,
``find`` and ``plural_rule``. ``Catalog`` and ``MoCatalog`` both implement it.
``Translator`` binds ``Box<dyn Lookup + Send + Sync>``, so ``bind_text_domain`` serves
``.mo`` files zero-copy. A ``Catalog``'s lazily built index now sits in ``OnceLock``, so
catalogs are ``Sync`` and threads can share them.

Measured
--------

``cargo test --release -p gd-gettext --test large -- --ignored --nocapture``, run on this
machine. The test generates 200,000 messages (1 in 7 with a msgctxt, about 250-byte
translations): a 61.6 MB MO file and a 63.6 MB PO file. "Allocated" is every byte requested
from the allocator during the step; "peak" is the most held at once above the start, counted
by a counting global allocator in that test binary only. That allocator needs
``unsafe impl GlobalAlloc`` with ``SAFETY`` comments, and it is test-only.

The counting allocator forwards ``realloc``, so in-place grows and shrinks stay in place as
they would without it. That makes the first rows below lower than in the previous version of
this note.

=========================================================== =============== ========= ======
Step                                                        Time            Allocated Peak
=========================================================== =============== ========= ======
before: ``parse_mo`` into a ``Catalog`` (copies)            125–144 ms      188 MB    179 MB
before: its first lookup builds the index                   30–55 ms        60 MB     60 MB
after: ``MoCatalog::from_bytes`` (incl. a 61.6 MB clone)    66–67 ms        71 MB     66 MB
after: ``MoCatalog::load`` from a file                      77–82 ms        71 MB     66 MB
``write_mo`` (61.6 MB): iterator-compared sort → key buffer 372 → 94–102 ms 96 MB     87 MB
``parse_po`` into a ``Catalog``                             290 ms          176 MB    167 MB
``PoReader`` + ``Stats::add`` (no catalog)                  235 ms          102 MB    18 MB
=========================================================== =============== ========= ======

The 66 MB peak of ``MoCatalog::from_bytes`` is the 61.6 MB buffer (which becomes the pool),
3.2 MB of spans, and 1.1 MB of hash slots. Steady state is about 1× the file.

Lookups, 1M each, over 1000 present keys (hits) or 1000 absent ones (misses):

======================================================================= ========= =========
Catalog                                                                 Hits/s    Misses/s
======================================================================= ========= =========
``Catalog`` (owned strings, FNV index, ≤ ½ full)                        6.3–7.5 M 21–24 M
``MoCatalog``, raw buffer + ``from_utf8`` per lookup (previous version) 4.6 M     4.0 M
``MoCatalog``, text pool + length-first key check                       5.4–5.5 M 8.9–9.4 M
======================================================================= ========= =========

Loading a large compiled catalog went from 155–200 ms and about 240 MB held (copied strings
plus index) to 66–82 ms and about the file's size. Moving to the pool removed the per-lookup
UTF-8 check, but that check turned out to be small. The larger gain, on misses, came from
comparing key lengths before bytes. What remains of the gap to the FNV index is the GNU
table itself: msgfmt fills it to ¾ (so a miss probes about four slots, against about two at
½), and hashpjw is a little slower than FNV. Both are fixed by the file format; changing
them would lose GNU compatibility. An editable ``Catalog`` stays the faster choice when
memory is plentiful and lookups dominate; ``MoCatalog`` is the choice when catalogs are
large, many, or loaded often. Streaming PO statistics need a tenth of the peak memory of a
parsed catalog.
