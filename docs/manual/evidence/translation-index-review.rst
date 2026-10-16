Translation and index review
============================

On 16 October 2026 we investigated `issue 2 <https://github.com/insanai/guidedog/issues/2>`_: translated glossary terms disappeared from the general index while their page anchors remained present.

Root cause
----------

A glossary term contains two kinds of children: visible inline text and a generated index record. The translation pass replaced all children with newly parsed inline content. This removed the index record along with the source text. The HTML collector therefore had no entry to put in ``genindex.html``. The PDF book uses the same collected index and was affected too.

The defect is independent of script. A translated Latin term disappears in the same way. An untranslated term keeps its original children, which explains why Latin terms left untranslated still appeared in the reported project. A natively written non-Latin glossary also keeps its index records.

Fix
---

The reader marks the automatic glossary index record as generated metadata. The translation pass preserves generated children when it replaces written inline content. The glossary entry takes its display text from the parsed translation, without markup, and keeps its original target and category. Existing ``:term:`` links continue to resolve to the same anchors.

The pass already preserved generated prefixes, such as parameter names inserted before a description. It now preserves generated children after the written content too. A translation that is refused keeps the complete original subtree, including its metadata.

Related defect
--------------

The review reproduced a second index defect. Every translated message is parsed in an isolated reader workspace. Each workspace starts its inline index serial at zero. Separate translated paragraphs containing ``:envvar:``, ``:index:``, ``:pep:`` or similar roles could therefore acquire the same ``index-0`` identity. Later entries lost their anchors.

Reference matching now carries the original inline index identities into the translated content. It matches unchanged entries first, then pairs remaining renamed entries in order. Reordered roles keep their identities, and renamed index text remains translatable. A translation that adds more index identities than its source provides is refused with the existing translation diagnostic.

Regression checks
-----------------

Both defects were reproduced with failing tests before the fixes. Reader-level tests cover reStructuredText and MyST, marked-up terms, category keys, refused translations and reordered or renamed inline indexes. The glossary matrix includes Japanese, Chinese, Korean, Hindi, Arabic and German.

End-to-end checks use the real gettext extraction, ``intl update`` and MO compilation paths. They verify translated entries, unchanged entries, native non-Latin terms, stable term links, cached rebuilds and catalog changes. A PDF build verifies that the book index contains the same translated entries and targets. Every release archive's smoke test also builds a translated glossary and separate translated paragraphs with inline index roles.

Wider review
------------

We reviewed the path from source discovery through reading, translation, collection, reference resolution, HTML and PDF composition, cache invalidation and output publication. We also checked the CLI and linked-plugin boundaries, resource confinement, preview paths, bounded HTTP reads and release qualification. The workspace checks exercise these layers together with their parser fixtures, malformed-input tests, transaction recovery tests and plugin scenarios.

The known limits in :doc:`../internals/release-readiness` remain recorded separately. This review fixes the two reproduced index defects; it does not establish that every possible document or platform behavior is free of defects.
