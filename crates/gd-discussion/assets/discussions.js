(() => {
  "use strict";
  const root = document.documentElement;
  const theme = document.querySelector(".theme-toggle");
  let mode = "auto";
  try { mode = localStorage.getItem("guidedog-theme") || "auto"; } catch (_) {}
  if (!["auto", "light", "dark"].includes(mode)) mode = "auto";
  function setTheme(value) {
    mode = value;
    root.dataset.theme = value;
    theme.textContent = "Theme: " + value;
    theme.setAttribute("aria-label", "Color theme: " + value + ". Activate to change.");
    try { localStorage.setItem("guidedog-theme", value); } catch (_) {}
  }
  setTheme(mode);
  theme.addEventListener("click", () => {
    const modes = ["auto", "light", "dark"];
    setTheme(modes[(modes.indexOf(mode) + 1) % modes.length]);
  });
  const menu = document.querySelector(".sidebar details");
  const small = matchMedia("(max-width: 760px)");
  function adaptNavigation() { menu.open = !small.matches; }
  adaptNavigation();
  small.addEventListener("change", adaptNavigation);
  const links = [...document.querySelectorAll(".sidebar nav a")];
  document.querySelector("#section-filter").addEventListener("input", (event) => {
    const query = event.target.value.trim().toLowerCase();
    links.forEach(link => { link.hidden = !link.textContent.toLowerCase().includes(query); });
  });
  links.forEach(link => link.addEventListener("click", () => {
    if (small.matches) menu.open = false;
  }));
  if ("IntersectionObserver" in window) {
    const observer = new IntersectionObserver(entries => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        links.forEach(link => {
          if (link.hash === "#" + entry.target.id) link.setAttribute("aria-current", "location");
          else link.removeAttribute("aria-current");
        });
      }
    }, { rootMargin: "-12% 0px -70% 0px" });
    document.querySelectorAll("main h1, main h2").forEach(heading => observer.observe(heading));
  }
  document.querySelectorAll("pre").forEach(pre => {
    const button = document.createElement("button");
    button.className = "copy-button";
    button.type = "button";
    button.textContent = "Copy";
    button.setAttribute("aria-label", "Copy code block");
    const source = pre.textContent;
    button.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText(source);
        button.textContent = "Copied";
      } catch (_) { button.textContent = "Select to copy"; }
      setTimeout(() => { button.textContent = "Copy"; }, 1800);
    });
    pre.appendChild(button);
  });
})();
