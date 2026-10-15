Windows runtime notices

The pinned official Graphviz 16.1.0 Windows distribution supplies the DLLs.
We include its core SVG renderer and dot and neato layout plugins, together
with their runtime dependencies. We omit the optional image and font renderers.

Graphviz: Eclipse Public License 2.0, COPYING-graphviz.
  Source: https://gitlab.com/graphviz/graphviz/-/tree/16.1.0
Expat: MIT, COPYING-expat. Source: https://github.com/libexpat/libexpat
zlib: zlib license, COPYING-zlib. Source: https://github.com/madler/zlib
PCRE2: BSD license, COPYING-pcre2. Source: https://github.com/PCRE2Project/pcre2
GLib: LGPL 2.1 or later, COPYING-LGPL-2.1.
  Source: https://gitlab.gnome.org/GNOME/glib
GTS: LGPL 2 or later, COPYING-LGPL-2.1.
  Source: https://gts.sourceforge.net/
GNU libiconv and gettext (libintl): LGPL 2.1 or later, COPYING-LGPL-2.1.
  Source: https://www.gnu.org/software/libiconv/
  Source: https://www.gnu.org/software/gettext/
Microsoft Visual C++ runtime: redistributed from the official Graphviz package.
  License: https://visualstudio.microsoft.com/license-terms/vs2022-cruntime/

These libraries remain separate DLLs and can be replaced with compatible builds.
SOURCE.txt records the upstream Graphviz archive and its SHA-256.
