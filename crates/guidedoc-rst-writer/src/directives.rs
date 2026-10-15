//! Body elements written as Docutils directives: admonitions, topics, sidebars,
//! containers, compounds, tables of contents, images and figures, and the directives
//! the tree keeps as Directive nodes.

use std::fmt::Write as _;

use guidedoc::{
    AdmonitionKind, AttributeKey, LinkInfo, LinkKind, NodeFlag, NodeId, NodeKind, admonition_name,
    make_id,
};

use crate::blocks::Opt;
use crate::explicit::reference_to;
use crate::state::Writer;

/// Image attributes and the options that set them.
const IMAGE_OPTIONS: [(AttributeKey, &str); 5] = [
    (AttributeKey::Alt, "alt"),
    (AttributeKey::Height, "height"),
    (AttributeKey::Width, "width"),
    (AttributeKey::Scale, "scale"),
    (AttributeKey::Align, "align"),
];

impl Writer<'_, '_> {
    /// Tells whether the first child of `id` is a Title.
    fn has_title(&self, id: NodeId) -> bool {
        self.doc.first_child(id).is_some_and(|f| self.kind(f) == NodeKind::Title)
    }

    /// The Title child's inline content as a directive argument.
    fn title_argument(&mut self, id: NodeId) -> String {
        match self.doc.first_child(id).filter(|&f| self.kind(f) == NodeKind::Title) {
            Some(title) => self.inline_line(title),
            None => String::new(),
        }
    }

    /// `.. note::` and the other admonitions; `.. admonition:: Title` for the generic one.
    pub fn admonition(&mut self, id: NodeId) {
        let kind = AdmonitionKind::from_value(self.doc.node(id).value).unwrap_or_default();
        let title = self.title_argument(id);
        let mut implied = vec![];
        let auto_class;
        if kind == AdmonitionKind::Generic {
            let mut plain = String::new();
            if let Some(t) = self.doc.first_child(id) {
                self.doc.plain_text(t, &mut plain);
            }
            let mut slug = String::new();
            make_id(&mut slug, &plain);
            auto_class = format!("admonition-{slug}");
            implied.push(auto_class.as_str());
        }
        let mut options = Vec::new();
        self.common_options(id, &implied, &mut options);
        self.directive_head(admonition_name(kind), &title, &options);
        self.directive_body(id, usize::from(self.has_title(id)));
    }

    /// `.. topic:: Title`.
    pub fn topic(&mut self, id: NodeId) {
        let title = self.title_argument(id);
        let mut options = Vec::new();
        self.common_options(id, &[], &mut options);
        self.directive_head("topic", &title, &options);
        self.directive_body(id, usize::from(self.has_title(id)));
    }

    /// `.. sidebar:: Title` with its subtitle.
    pub fn sidebar(&mut self, id: NodeId) {
        let title = self.title_argument(id);
        let mut skip = usize::from(self.has_title(id));
        let mut options = Vec::new();
        if let Some(sub) = self.doc.children(id).nth(skip)
            && self.kind(sub) == NodeKind::Subtitle
        {
            options.push(("subtitle", self.inline_line(sub)));
            skip += 1;
        }
        self.common_options(id, &[], &mut options);
        self.directive_head("sidebar", &title, &options);
        self.directive_body(id, skip);
    }

    /// `.. container:: classes`.
    pub fn container(&mut self, id: NodeId) {
        let classes = self.classes(id, &[]);
        let mut options = Vec::new();
        if let Some(name) = self.attr(id, AttributeKey::Name) {
            options.push(("name", name.to_owned()));
        }
        self.directive_head("container", &classes, &options);
        self.directive_body(id, 0);
    }

    /// `.. compound::`.
    pub fn compound(&mut self, id: NodeId) {
        let mut options = Vec::new();
        self.common_options(id, &[], &mut options);
        self.directive_head("compound", "", &options);
        self.directive_body(id, 0);
    }

    /// `.. contents:: Title` with its depth, scope, and backlinks.
    pub fn contents(&mut self, id: NodeId) {
        let node = self.doc.node(id);
        let title = self.attr(id, AttributeKey::Title).unwrap_or("").to_owned();
        let mut options: Vec<Opt> = Vec::new();
        if node.value > 0 {
            options.push(("depth", node.value.to_string()));
        }
        if node.flags.has(NodeFlag::Local) {
            options.push(("local", String::new()));
        }
        if let Some(b) = self.custom(id, "backlinks") {
            options.push(("backlinks", b.to_owned()));
        }
        self.common_options(id, &[], &mut options);
        self.directive_head("contents", &title, &options);
    }

    /// A directive the tree keeps as such: sectnum, header, footer, meta, title,
    /// target-notes, and the Sphinx author directives; others by their name, with
    /// their Custom attributes as options.
    pub fn directive(&mut self, id: NodeId) {
        let name = self.attr(id, AttributeKey::Name).unwrap_or("").to_owned();
        let mut options: Vec<Opt> = Vec::new();
        let mut argument = String::new();
        match name.as_str() {
            "sectnum" => self.custom_options(id, &[], &mut options),
            "meta" => return self.meta(id),
            "title" => argument = self.attr(id, AttributeKey::Title).unwrap_or("").to_owned(),
            "sectionauthor" | "moduleauthor" | "codeauthor" => {
                argument = self.custom(id, "author").unwrap_or("").to_owned();
            }
            "header" | "footer" | "target-notes" | "todolist" => {}
            _ => {
                self.lossy(id, "a directive the writer does not know", "its name and content");
                self.custom_options(id, &[], &mut options);
            }
        }
        if argument.is_empty() && self.has_title(id) {
            argument = self.title_argument(id);
        }
        let classes = self.classes(id, &[]);
        if !classes.is_empty() {
            options.push(("class", classes));
        }
        self.directive_head(&name, &argument, &options);
        self.directive_body(id, usize::from(self.has_title(id)));
    }

    /// `.. meta::` with one field per Custom attribute.
    fn meta(&mut self, id: NodeId) {
        self.directive_head("meta", "", &[]);
        self.push_frame(3, "");
        for a in self.doc.attributes(id) {
            if a.key == AttributeKey::Custom {
                let line = format!(":{}: {}", self.doc.text(a.name), self.doc.text(a.value));
                self.line(&line);
            }
        }
        self.pop_frame();
        self.block_end();
    }

    /// An image: the image directive at block level; inside a paragraph, a
    /// substitution reference whose definition follows the paragraph.
    pub fn image(&mut self, id: NodeId) {
        if !self.in_inline {
            self.block_image(id);
            return;
        }
        self.inline_image(id, None);
    }

    /// Writes an image inside text as a substitution reference whose definition
    /// follows the block; `wrapper` is a link around it.
    pub fn inline_image(&mut self, id: NodeId, wrapper: Option<NodeId>) {
        self.lossy(id, "an image inside text", "a substitution reference to an image");
        let mut alt = self.buffer();
        match self.attr(id, AttributeKey::Alt) {
            Some(text) => alt.push_str(text),
            None => self.doc.plain_text(id, &mut alt),
        }
        // The alt text's words, one space apart.
        let mut name = self.buffer();
        for (k, word) in alt.split_whitespace().enumerate() {
            if k > 0 {
                name.push(' ');
            }
            name.push_str(word);
        }
        self.recycle(alt);
        if name.is_empty() || name.contains(['|', '`', '*', '\\']) {
            name.clear();
            let _ = write!(name, "image-{}", self.pending_images.len() + 1);
        }
        while self.pending_images.iter().any(|(n, _, _)| *n == name) {
            name.push('-');
        }
        self.inline.markup_fmt(format_args!("|{name}|"));
        self.pending_images.push((name, id, wrapper));
    }

    /// `.. image:: URI` for an image at block level.
    pub fn block_image(&mut self, id: NodeId) {
        self.image_directive("image", id, None);
    }

    /// Writes `.. head:: URI` with the image's options; `wrapper` is the link around
    /// it, which becomes `:target:`.
    pub fn image_directive(&mut self, head: &str, id: NodeId, wrapper: Option<NodeId>) {
        let link = self.doc.link_info(id).unwrap_or(LinkInfo::UNRESOLVED);
        let uri = self.doc.text(link.destination);
        let mut options = self.image_options(id);
        if let Some(w) = wrapper {
            options.push(("target", self.link_target(w)));
        }
        if !self.doc.text(link.title).is_empty() {
            self.lossy(id, "an image title", "the image without it");
        }
        self.directive_head(head, uri, &options);
    }

    fn image_options(&self, id: NodeId) -> Vec<Opt> {
        let mut options: Vec<Opt> = Vec::new();
        for (key, name) in IMAGE_OPTIONS {
            let value = match key {
                AttributeKey::Alt => self.image_alt(id),
                _ => self.attr(id, key).map(str::to_owned),
            };
            if let Some(v) = value {
                options.push((name, v));
            }
        }
        self.common_options(id, &[], &mut options);
        self.custom_options(id, &[], &mut options);
        options
    }

    /// What `:target:` holds for an image inside link `w`: a URI or `name_`.
    fn link_target(&self, w: NodeId) -> String {
        let link = self.doc.link_info(w).unwrap_or(LinkInfo::UNRESOLVED);
        let name = self.doc.text(link.name);
        if self.kind(w) == NodeKind::Reference && !name.is_empty() {
            return reference_to(name);
        }
        match link.kind {
            LinkKind::Email => format!("mailto:{}", self.doc.text(link.destination)),
            _ => self.doc.text(link.destination).to_owned(),
        }
    }

    /// `.. figure:: URI` with the image's and the figure's options, the caption, and
    /// the legend.
    pub fn figure(&mut self, id: NodeId) {
        let mut image = None;
        let mut wrapper = None;
        for child in self.doc.children(id) {
            if let Some(i) = self.image_in(child) {
                image = Some(i);
                wrapper = (i != child).then_some(child);
            }
        }
        let Some(image) = image else {
            self.lossy(id, "a figure without an image", "its caption and legend");
            self.blocks(id);
            return;
        };
        let link = self.doc.link_info(image).unwrap_or(LinkInfo::UNRESOLVED);
        let uri = self.doc.text(link.destination).to_owned();
        let mut options = self.image_options(image);
        if let Some(w) = wrapper {
            options.push(("target", self.link_target(w)));
        }
        self.figure_options(id, &mut options);
        self.directive_head("figure", &uri, &options);
        self.push_frame(3, "");
        for child in self.doc.children(id) {
            match self.kind(child) {
                NodeKind::Caption => {
                    self.class_prefix(child, &[]);
                    if let Some(name) = self.attr(child, AttributeKey::Name) {
                        let line = format!(".. _{name}:");
                        self.block_start();
                        self.line(&line);
                        self.block_end();
                    }
                    self.plain_block(child);
                }
                NodeKind::Legend => self.blocks(child),
                _ => {}
            }
        }
        self.pop_frame();
        self.block_end();
    }

    fn figure_options(&self, id: NodeId, options: &mut Vec<Opt>) {
        if let Some(w) = self.attr(id, AttributeKey::Width).or_else(|| self.custom(id, "figwidth"))
        {
            options.push(("figwidth", w.to_owned()));
        }
        if let Some(a) = self.attr(id, AttributeKey::Align) {
            options.retain(|(k, _)| *k != "align");
            options.push(("align", a.to_owned()));
        }
        let classes = self.classes(id, &[]);
        if !classes.is_empty() {
            options.push(("figclass", classes));
        }
        if let Some(name) = self.attr(id, AttributeKey::Name) {
            options.retain(|(k, _)| *k != "name");
            options.push(("name", name.to_owned()));
        }
    }
}
