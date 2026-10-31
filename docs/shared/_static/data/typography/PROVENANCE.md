# Local web typography

Typeset: Knuth-Plass boxes, glue, penalties and fitness classes.
https://github.com/bramstein/typeset/tree/48ebb5547de116db57e14454e19767e4ff9b2266
The linked list is unmodified; linebreak.js removes the post-break penalty width in computeSum() so lines after a hyphen are measured accurately. See TYPESET-LICENSE.txt.

Hypher: Frank Liang pattern-trie hyphenation.
https://github.com/bramstein/typeset/blob/48ebb5547de116db57e14454e19767e4ff9b2266/lib/hypher.js
See HYPHER-LICENSE.txt.

English: original TeX hyph-en-us.tex, Gerard D. C. Kuiken, version 2005-05-30.
https://github.com/hyphenation/tex-hyphen/tree/master/hyph-utf8/tex/generic/hyph-utf8/patterns/tex
The source and its permissive distribution notice are retained. english.js groups
patterns by their encoded length, replaces boundary dots with Hypher underscores,
and converts exception hyphens to U+2027. The reading edition uses three-character
left and right minima. No language service or CDN is used at runtime.

crates/gd-theme/assets/typography.js bundles linked-list.js, linebreak.js, hypher.js,
english.js, and Guidedog's paragraph layout pass into one self-contained theme script
copied into projects as _static/typography.js.
