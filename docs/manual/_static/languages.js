/* The manual's language menu is also usable without JavaScript. */
(function () {
  "use strict";
  var menu = document.querySelector(".gd-language");
  if (!menu) return;
  var links = Array.from(menu.querySelectorAll("a[data-language]"));
  var current = menu.dataset.edition;
  var key = "guidedog-manual-language";
  function matching(code) {
    code = String(code || "").replace(/_/g, "-").toLowerCase();
    var base = code.split("-")[0];
    /* This project has Simplified Chinese, not a Traditional Chinese edition. */
    if (base === "zh" && /(?:^|-)(hant|tw|hk|mo)(?:-|$)/.test(code)) return null;
    return links.find(function (link) {
      return link.dataset.language.replace(/_/g, "-").toLowerCase() === code;
    }) || links.find(function (link) {
      return link.dataset.language.split("_")[0] === base;
    });
  }
  links.forEach(function (link) {
    var url = new URL(link.href, location.href);
    url.search = location.search;
    url.hash = location.hash;
    link.href = url.href;
    link.addEventListener("click", function () {
      try { localStorage.setItem(key, link.dataset.language); } catch (_) {}
    });
  });
  document.addEventListener("click", function (event) {
    if (!menu.contains(event.target)) menu.open = false;
  });
  document.addEventListener("keydown", function (event) {
    if (event.key === "Escape" && menu.open) {
      menu.open = false;
      menu.querySelector("summary").focus();
    }
  });
  /* A direct edition or chapter link is an explicit request. Detection applies
     only to the English entry pages; this also prevents redirect loops. */
  if (current !== "en" || !["index", "repository"].includes(menu.dataset.page)) return;
  var chosen = null;
  try { chosen = matching(localStorage.getItem(key)); } catch (_) {}
  if (!chosen) {
    var preferred = navigator.languages || [navigator.language];
    for (var i = 0; i < preferred.length && !chosen; i++) {
      chosen = matching(preferred[i]);
    }
  }
  if (chosen && chosen.dataset.language !== current) location.replace(chosen.href);
}());
