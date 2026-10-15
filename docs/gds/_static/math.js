// The SVG bundle embeds the TeX fonts. No external font request is needed.
window.MathJax = {
  svg: { fontCache: "local" },
  options: { enableMenu: false },
  tex: { packages: { "[-]": ["require", "autoload"] } }
};
