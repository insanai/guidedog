//! A build plugin: typed source and HTML hooks, plus a generated CSS asset. It
//! replaces `{{product}}` in source text, labels HTML pages, and supplies the label's
//! stylesheet. Build it, register it, and select it in `conf.toml`:
//!
//! ```text
//! cargo build --release -p gd-plugins --example build_plugin
//! guidedog plugin register product-badge "$PWD/target/release/examples/build_plugin" \
//!     --project=docs --config=badge.toml
//! ```
//!
//! `badge.toml` holds `product = "Orbit"`. An application can instead link the plugin
//! directly with `gd_plugins::Plugin::build(Badge::default())`.

use std::borrow::Cow;
use std::process::ExitCode;

use gd_plugins::{
    Asset, BuildPlugin, Hooks, HtmlInput, LoadContext, MAX_PLUGIN_TEXT, PluginInfo, Problem,
    SourceInput,
};

const MARKER: &str = "{{product}}";
const CSS: &str = ".product-badge { border-inline-start: 3px solid #216553; \
                   padding: .5rem 1rem; margin: 1rem; font-weight: 700; }\n";

/// The plugin's state: the product name `load` read. The unloaded prototype is empty.
#[derive(Debug, Clone, Default)]
struct Badge {
    product: String,
}

impl BuildPlugin for Badge {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("product-badge", "Product badge", "1.0.0", Hooks::ALL)
    }

    fn load(&mut self, ctx: &mut LoadContext<'_>) -> Result<(), Problem> {
        let text = ctx.read_config()?;
        let path = ctx.options().config.clone();
        let root = gd_toml::parse(&text).map_err(|e| {
            Problem::new("badge.toml", "Cannot read the badge settings", &e.message)
                .hint("Write product = \"Orbit\" in this TOML file.")
                .path(&format!("{path}:{}", e.line))
                .exit(2)
        })?;
        let product = root.get("product").and_then(|v| v.as_str()).unwrap_or_default();
        if product.is_empty() || product.len() > 256 || product.contains(['\r', '\n', '\0']) {
            return Err(Problem::new(
                "badge.product",
                "The product name is missing or invalid",
                "product must be a nonempty string on one line, at most 256 bytes long.",
            )
            .hint("Set product = \"Orbit\" in this file.")
            .path(&path)
            .exit(2));
        }
        self.product = product.to_string();
        Ok(())
    }

    fn process_source<'a>(&mut self, input: &SourceInput<'a>) -> Result<Cow<'a, str>, Problem> {
        if !input.text.contains(MARKER) {
            return Ok(Cow::Borrowed(input.text));
        }
        // Check the expansion before allocating it.
        let count = input.text.matches(MARKER).count();
        let growth = self.product.len().saturating_sub(MARKER.len());
        if input.text.len() + count * growth > MAX_PLUGIN_TEXT {
            return Err(Problem::new(
                "badge.source-size",
                "The expanded source would be too large",
                "Replacing every product marker would exceed the 8 MiB source limit.",
            )
            .hint("Use fewer markers, a shorter product name, or split the document.")
            .path(input.path)
            .exit(2));
        }
        Ok(Cow::Owned(input.text.replace(MARKER, &self.product)))
    }

    fn process_html<'a>(&mut self, input: &HtmlInput<'a>) -> Result<Cow<'a, str>, Problem> {
        let tags = (input.text.rfind("</head>"), input.text.rfind("</body>"));
        let (Some(head), Some(body)) = tags else {
            return Err(Problem::new(
                "badge.html",
                "The badge needs a complete HTML page",
                "The selected template has no closing head or body tag.",
            )
            .hint("Add </head> and </body> to the template, or disable product-badge.")
            .path(input.path)
            .exit(2));
        };
        let body = body.max(head);
        let css = format!("<link rel=\"stylesheet\" href=\"{}banner.css\">\n", input.assets_url);
        let badge = format!(
            "<aside class=\"product-badge\" aria-label=\"Product\">{}</aside>\n",
            escape_html(&self.product)
        );
        let mut out = String::with_capacity(input.text.len() + css.len() + badge.len());
        out.push_str(&input.text[..head]);
        out.push_str(&css);
        out.push_str(&input.text[head..body]);
        out.push_str(&badge);
        out.push_str(&input.text[body..]);
        Ok(Cow::Owned(out))
    }

    fn collect_assets(&mut self) -> Result<Vec<Asset>, Problem> {
        Ok(vec![Asset::new("banner.css", CSS)])
    }
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn main() -> ExitCode {
    gd_plugins::serve(Badge::default())
}
