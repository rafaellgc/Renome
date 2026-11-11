//! A forgiving reader of HTML and a writer of clean XHTML. Books carry
//! markup of every quality: old Kindle files have HTML 3 with `<font>` and
//! unclosed `<p>`, web pages have scripts and forms, EPUBs sometimes have
//! entities XML does not know. Everything is read into one small tree,
//! then written as XHTML an EPUB reader accepts: known elements only
//! (others give their content), safe attributes and styles only.

use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Elem(Elem),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Elem {
    /// Lower case, without a namespace prefix (`svg:image` is `image`),
    /// but for `epub:switch` and the like, which keep it.
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub kids: Vec<Node>,
}

impl Elem {
    pub fn new(name: &str) -> Self {
        Elem {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn set(&mut self, name: &str, value: impl Into<String>) {
        let value = value.into();
        match self.attrs.iter_mut().find(|(k, _)| k == name) {
            Some(a) => a.1 = value,
            None => self.attrs.push((name.into(), value)),
        }
    }

    pub fn with(mut self, name: &str, value: impl Into<String>) -> Self {
        self.set(name, value);
        self
    }

    pub fn push(&mut self, n: Node) {
        self.kids.push(n);
    }

    pub fn text(&mut self, t: impl Into<String>) {
        let t = t.into();
        if t.is_empty() {
            return;
        }
        if let Some(Node::Text(last)) = self.kids.last_mut() {
            last.push_str(&t);
        } else {
            self.kids.push(Node::Text(t));
        }
    }

    /// All the text inside, markup dropped.
    pub fn plain(&self) -> String {
        let mut s = String::new();
        plain_into(&self.kids, &mut s);
        s
    }
}

pub fn plain_into(nodes: &[Node], s: &mut String) {
    for n in nodes {
        match n {
            Node::Text(t) => s.push_str(t),
            Node::Elem(e) => {
                if e.name == "br" {
                    s.push('\n');
                }
                plain_into(&e.kids, s);
                // Blocks part words from the next ones.
                if is_block(&e.name)
                    || matches!(e.name.as_str(), "td" | "th" | "tr" | "img" | "aside")
                {
                    s.push('\n');
                }
            }
        }
    }
}

/// Elements with no content.
pub fn is_void(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
            | "basefont"
            | "frame"
            | "keygen"
            | "mbp:pagebreak"
    )
}

/// Elements that are blocks: they close an open `<p>`.
pub fn is_block(name: &str) -> bool {
    matches!(
        name,
        "p" | "div"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "ul"
            | "ol"
            | "dl"
            | "li"
            | "dt"
            | "dd"
            | "table"
            | "blockquote"
            | "pre"
            | "hr"
            | "section"
            | "article"
            | "aside"
            | "header"
            | "footer"
            | "nav"
            | "figure"
            | "figcaption"
            | "address"
            | "center"
            | "form"
            | "fieldset"
            | "main"
            | "details"
            | "summary"
            | "menu"
            | "dir"
            | "listing"
            | "xmp"
            | "plaintext"
            | "body"
            | "html"
    )
}

/// Elements whose content is raw text, not markup.
/// Reads HTML (or XHTML) into a tree; never fails. The tree is built the
/// way browsers build it (html5ever follows the HTML standard): a `<p>`
/// left open is closed by the next block, a table's stray text is moved
/// out of it, and so on. The result is the document's top nodes (one
/// `html`, with its `head` and `body`).
///
/// XHTML's `<a id="x"/>` means an empty element, but HTML would read it
/// as an opening tag holding the rest of the page; such tags are closed
/// first.
pub fn parse(src: &str) -> Vec<Node> {
    use html5ever::tendril::TendrilSink;
    let src = close_empty_tags(src);
    html5ever::parse_document(Arena::default(), Default::default())
        .one(html5ever::tendril::StrTendril::from_slice(&src))
}

/// Reads XML (FB2, Office documents) in the same forgiving way, but
/// without HTML's rules: no raw-text elements, no tags closed by others.
pub fn parse_xml(src: &str) -> Vec<Node> {
    let mut b = Builder::default();
    let s = src.as_bytes();
    let mut i = 0;
    let mut text_start = 0;
    while i < s.len() {
        if s[i] != b'<' {
            i += 1;
            continue;
        }
        // Text before.
        if i > text_start {
            b.text(&decode(&src[text_start..i]));
        }
        let rest = &src[i..];
        if rest.starts_with("<!--") {
            i = rest.find("-->").map(|e| i + e + 3).unwrap_or(s.len());
        } else if rest.starts_with("<![CDATA[") {
            let end = rest.find("]]>").map(|e| i + e).unwrap_or(s.len());
            b.text(&src[i + 9..end.max(i + 9)]);
            i = (end + 3).min(s.len());
        } else if rest.starts_with("<!") || rest.starts_with("<?") {
            i = rest.find('>').map(|e| i + e + 1).unwrap_or(s.len());
        } else if rest.starts_with("</") {
            let end = tag_end(s, i + 2);
            let name = tag_name(&src[i + 2..end]);
            if !name.is_empty() {
                b.close(&name);
            }
            i = (end + 1).min(s.len());
        } else if s.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic()) {
            let end = tag_end(s, i + 1);
            let inner = &src[i + 1..end];
            let self_closing = inner.trim_end().ends_with('/');
            let inner = inner.trim_end().trim_end_matches('/');
            let name = tag_name(inner);
            let attrs = attrs_of(
                &inner[inner
                    .find(|c: char| c.is_whitespace())
                    .unwrap_or(inner.len())..],
            );
            i = (end + 1).min(s.len());
            let mut e = Elem::new(&name);
            e.attrs = attrs;
            if self_closing {
                b.leaf(e);
            } else {
                b.stack.push(e);
            }
        } else {
            // A lone `<`.
            b.text("<");
            i += 1;
        }
        text_start = i;
    }
    if text_start < s.len() {
        b.text(&decode(&src[text_start..]));
    }
    b.finish()
}

/// `<x .../>` written as `<x ...></x>` for every element that is not
/// void (and `<mbp:pagebreak>` always closed), outside comments, CDATA and
/// the raw text of scripts and styles.
fn close_empty_tags(src: &str) -> std::borrow::Cow<'_, str> {
    if !src.contains("/>") && !src.contains("mbp:") {
        return src.into();
    }
    let s = src.as_bytes();
    let mut out = String::with_capacity(src.len() + 64);
    let (mut i, mut copied) = (0, 0);
    while let Some(at) = src[i..].find('<').map(|p| p + i) {
        let rest = &src[at..];
        let skip_to = |end: &str, from: usize| src[from..].find(end).map(|e| from + e + end.len());
        if rest.starts_with("<!--") {
            i = skip_to("-->", at + 4).unwrap_or(s.len());
            continue;
        }
        if rest.starts_with("<![CDATA[") {
            i = skip_to("]]>", at + 9).unwrap_or(s.len());
            continue;
        }
        if !s.get(at + 1).is_some_and(|c| c.is_ascii_alphabetic()) {
            i = at + 1;
            continue;
        }
        let end = tag_end(s, at + 1);
        if end >= s.len() {
            break;
        }
        let inner = src[at + 1..end].trim_end();
        let name: String = inner
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != '/')
            .collect::<String>()
            .to_ascii_lowercase();
        i = end + 1;
        // Old Kindle books' page break is void too, but HTML does not
        // know it; it is closed with or without its `/`.
        let pagebreak = name == "mbp:pagebreak";
        if (inner.ends_with('/') && !is_void(&name)) || pagebreak {
            out.push_str(&src[copied..at + 1]);
            out.push_str(inner.trim_end_matches('/').trim_end());
            out.push_str("></");
            out.push_str(&src[at + 1..at + 1 + name.len()]);
            out.push('>');
            copied = i;
        } else if matches!(name.as_str(), "script" | "style" | "textarea" | "title") {
            // Up to its end tag, as is.
            let lower = src[i..].to_ascii_lowercase();
            i = lower
                .find(&format!("</{name}"))
                .map(|e| i + e)
                .unwrap_or(s.len());
        }
    }
    if copied == 0 {
        return src.into();
    }
    out.push_str(&src[copied..]);
    out.into()
}

/// The nodes html5ever builds, kept by number; made into [`Node`]s at the
/// end.
#[derive(Default)]
struct Arena {
    nodes: std::cell::RefCell<Vec<ANode>>,
}

struct ANode {
    /// `None` for the document, text, comments and template contents.
    name: Option<html5ever::QualName>,
    attrs: Vec<(String, String)>,
    text: String,
    kids: Vec<usize>,
    parent: Option<usize>,
    /// A template's contents (never shown).
    contents: Option<usize>,
}

impl ANode {
    fn new(name: Option<html5ever::QualName>) -> Self {
        ANode {
            name,
            attrs: Vec::new(),
            text: String::new(),
            kids: Vec::new(),
            parent: None,
            contents: None,
        }
    }
}

impl Arena {
    fn add(&self, n: ANode) -> usize {
        let mut nodes = self.nodes.borrow_mut();
        nodes.push(n);
        nodes.len() - 1
    }

    fn text_node(&self, t: &str) -> usize {
        let mut n = ANode::new(None);
        n.text = t.to_string();
        self.add(n)
    }

    fn is_text(nodes: &[ANode], i: usize) -> bool {
        nodes[i].name.is_none() && !nodes[i].text.is_empty()
    }

    /// Puts `child` in `parent` at `at` (the end when `None`); text joins
    /// the text before it.
    fn insert(
        &self,
        parent: usize,
        at: Option<usize>,
        child: html5ever::tree_builder::NodeOrText<usize>,
    ) {
        use html5ever::tree_builder::NodeOrText;
        let at = at.unwrap_or(self.nodes.borrow()[parent].kids.len());
        let id = match child {
            NodeOrText::AppendText(t) => {
                let mut nodes = self.nodes.borrow_mut();
                if at > 0 {
                    let prev = nodes[parent].kids[at - 1];
                    if Self::is_text(&nodes, prev) {
                        nodes[prev].text.push_str(&t);
                        return;
                    }
                }
                drop(nodes);
                self.text_node(&t)
            }
            NodeOrText::AppendNode(h) => h,
        };
        let mut nodes = self.nodes.borrow_mut();
        nodes[id].parent = Some(parent);
        nodes[parent].kids.insert(at, id);
    }

    fn detach(&self, id: usize) {
        let mut nodes = self.nodes.borrow_mut();
        if let Some(p) = nodes[id].parent.take() {
            nodes[p].kids.retain(|k| *k != id);
        }
    }

    fn build(nodes: &[ANode], id: usize, out: &mut Vec<Node>) {
        let n = &nodes[id];
        let Some(q) = &n.name else {
            if !n.text.is_empty() {
                match out.last_mut() {
                    Some(Node::Text(last)) => last.push_str(&n.text),
                    _ => out.push(Node::Text(n.text.clone())),
                }
            }
            return;
        };
        let mut e = Elem::new(&tag_name(&q.local));
        e.attrs = n.attrs.clone();
        for &k in &n.kids {
            Self::build(nodes, k, &mut e.kids);
        }
        // A stray `</p>` makes an empty paragraph (old HTML's paragraph
        // break); it adds nothing.
        if e.name == "p" && e.attrs.is_empty() && e.kids.is_empty() {
            return;
        }
        out.push(Node::Elem(e));
    }
}

fn attr_pairs(attrs: Vec<html5ever::Attribute>) -> Vec<(String, String)> {
    attrs
        .into_iter()
        .map(|a| {
            let local = a.name.local.to_ascii_lowercase().to_string();
            let name = match &a.name.prefix {
                Some(p) => format!("{p}:{local}"),
                None => local,
            };
            (name, a.value.to_string())
        })
        .collect()
}

impl html5ever::tree_builder::TreeSink for Arena {
    type Handle = usize;
    type Output = Vec<Node>;
    type ElemName<'a> = std::cell::Ref<'a, html5ever::QualName>;

    fn finish(self) -> Vec<Node> {
        let nodes = self.nodes.into_inner();
        let mut out = Vec::new();
        if let Some(doc) = nodes.first() {
            for &k in &doc.kids {
                Self::build(&nodes, k, &mut out);
            }
        }
        out
    }

    fn parse_error(&self, _msg: std::borrow::Cow<'static, str>) {}

    fn get_document(&self) -> usize {
        if self.nodes.borrow().is_empty() {
            self.add(ANode::new(None));
        }
        0
    }

    fn elem_name<'a>(&'a self, target: &'a usize) -> Self::ElemName<'a> {
        std::cell::Ref::map(self.nodes.borrow(), |n| {
            n[*target].name.as_ref().expect("an element")
        })
    }

    fn create_element(
        &self,
        name: html5ever::QualName,
        attrs: Vec<html5ever::Attribute>,
        flags: html5ever::tree_builder::ElementFlags,
    ) -> usize {
        let mut n = ANode::new(Some(name));
        n.attrs = attr_pairs(attrs);
        if flags.template {
            n.contents = Some(self.add(ANode::new(None)));
        }
        self.add(n)
    }

    fn create_comment(&self, _text: html5ever::tendril::StrTendril) -> usize {
        self.add(ANode::new(None))
    }

    fn create_pi(
        &self,
        _target: html5ever::tendril::StrTendril,
        _data: html5ever::tendril::StrTendril,
    ) -> usize {
        self.add(ANode::new(None))
    }

    fn append(&self, parent: &usize, child: html5ever::tree_builder::NodeOrText<usize>) {
        self.insert(*parent, None, child);
    }

    fn append_based_on_parent_node(
        &self,
        element: &usize,
        prev_element: &usize,
        child: html5ever::tree_builder::NodeOrText<usize>,
    ) {
        if self.nodes.borrow()[*element].parent.is_some() {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        _name: html5ever::tendril::StrTendril,
        _public_id: html5ever::tendril::StrTendril,
        _system_id: html5ever::tendril::StrTendril,
    ) {
    }

    fn get_template_contents(&self, target: &usize) -> usize {
        self.nodes.borrow()[*target].contents.unwrap_or(*target)
    }

    fn same_node(&self, x: &usize, y: &usize) -> bool {
        x == y
    }

    fn set_quirks_mode(&self, _mode: html5ever::tree_builder::QuirksMode) {}

    fn append_before_sibling(
        &self,
        sibling: &usize,
        new_node: html5ever::tree_builder::NodeOrText<usize>,
    ) {
        let (parent, at) = {
            let nodes = self.nodes.borrow();
            let Some(p) = nodes[*sibling].parent else {
                return;
            };
            let at = nodes[p].kids.iter().position(|k| k == sibling);
            (p, at)
        };
        if let html5ever::tree_builder::NodeOrText::AppendNode(h) = &new_node {
            self.detach(*h);
        }
        self.insert(parent, at, new_node);
    }

    fn add_attrs_if_missing(&self, target: &usize, attrs: Vec<html5ever::Attribute>) {
        let mut nodes = self.nodes.borrow_mut();
        let n = &mut nodes[*target];
        for (k, v) in attr_pairs(attrs) {
            if !n.attrs.iter().any(|(a, _)| *a == k) {
                n.attrs.push((k, v));
            }
        }
    }

    fn remove_from_parent(&self, target: &usize) {
        self.detach(*target);
    }

    fn reparent_children(&self, node: &usize, new_parent: &usize) {
        let mut nodes = self.nodes.borrow_mut();
        let kids = std::mem::take(&mut nodes[*node].kids);
        for &k in &kids {
            nodes[k].parent = Some(*new_parent);
        }
        nodes[*new_parent].kids.extend(kids);
    }
}

/// Where a tag ends (its `>`), quotes respected.
fn tag_end(s: &[u8], mut i: usize) -> usize {
    let mut quote = 0u8;
    while i < s.len() {
        let c = s[i];
        if quote != 0 {
            if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
        } else if c == b'>' {
            return i;
        }
        i += 1;
    }
    s.len()
}

fn tag_name(inner: &str) -> String {
    let n: String = inner
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != '/' && *c != '>')
        .collect::<String>()
        .to_ascii_lowercase();
    // `svg:image` → `image`; `epub:switch`, `mbp:pagebreak` keep theirs.
    match n.split_once(':') {
        Some((p, local)) if p != "epub" && p != "mbp" => local.to_string(),
        _ => n,
    }
}

fn attrs_of(s: &str) -> Vec<(String, String)> {
    let b = s.as_bytes();
    let mut out: Vec<(String, String)> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        let start = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'=' && b[i] != b'/' {
            i += 1;
        }
        if start == i {
            break;
        }
        let name = s[start..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                let vs = i + 1;
                i = vs;
                while i < b.len() && b[i] != q {
                    i += 1;
                }
                value = decode(&s[vs..i.min(b.len())]);
                i += 1;
            } else {
                let vs = i;
                while i < b.len() && !b[i].is_ascii_whitespace() {
                    i += 1;
                }
                value = decode(&s[vs..i]);
            }
        }
        if !out.iter().any(|(k, _)| *k == name) {
            out.push((name, value));
        }
    }
    out
}

#[derive(Default)]
struct Builder {
    /// The open elements.
    stack: Vec<Elem>,
    top: Vec<Node>,
}

impl Builder {
    fn cur(&mut self) -> &mut Vec<Node> {
        match self.stack.last_mut() {
            Some(e) => &mut e.kids,
            None => &mut self.top,
        }
    }

    fn text(&mut self, t: &str) {
        if t.is_empty() {
            return;
        }
        let kids = self.cur();
        if let Some(Node::Text(last)) = kids.last_mut() {
            last.push_str(t);
        } else {
            kids.push(Node::Text(t.to_string()));
        }
    }

    fn leaf(&mut self, e: Elem) {
        self.cur().push(Node::Elem(e));
    }

    fn close(&mut self, name: &str) {
        let Some(at) = self.stack.iter().rposition(|e| e.name == name) else {
            return;
        };
        while self.stack.len() > at {
            let e = self.stack.pop().unwrap();
            self.cur().push(Node::Elem(e));
        }
    }

    fn finish(mut self) -> Vec<Node> {
        while let Some(e) = self.stack.pop() {
            self.cur().push(Node::Elem(e));
        }
        self.top
    }
}

/// Finds the first element named `name`, depth first.
pub fn find<'a>(nodes: &'a [Node], name: &str) -> Option<&'a Elem> {
    for n in nodes {
        if let Node::Elem(e) = n {
            if e.name == name {
                return Some(e);
            }
            if let Some(f) = find(&e.kids, name) {
                return Some(f);
            }
        }
    }
    None
}

/// Every element named `name`, in document order.
pub fn find_all<'a>(nodes: &'a [Node], name: &str, out: &mut Vec<&'a Elem>) {
    for n in nodes {
        if let Node::Elem(e) = n {
            if e.name == name {
                out.push(e);
            }
            find_all(&e.kids, name, out);
        }
    }
}

/// The body's content (the whole document when it has no `<body>`, its
/// `<head>` dropped).
pub fn body_of(nodes: Vec<Node>) -> (Vec<Node>, Vec<(String, String)>) {
    fn take_body(nodes: Vec<Node>) -> Result<Elem, Vec<Node>> {
        let mut rest = Vec::new();
        for n in nodes {
            match n {
                Node::Elem(e) if e.name == "body" => return Ok(e),
                Node::Elem(e) if e.name == "html" => match take_body(e.kids) {
                    Ok(b) => return Ok(b),
                    Err(k) => rest.extend(k),
                },
                Node::Elem(e) if e.name == "head" => {}
                n => rest.push(n),
            }
        }
        Err(rest)
    }
    match take_body(nodes) {
        Ok(b) => (b.kids, b.attrs),
        Err(rest) => (rest, Vec::new()),
    }
}

// ---------------------------------------------------------------- entities

/// Text with character references replaced. Unknown references stay.
pub fn decode(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let semi = rest[1..]
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '#')
            .map(|e| e + 1)
            .unwrap_or(rest.len());
        let name = &rest[1..semi];
        let has_semi = rest[semi..].starts_with(';');
        let ch = if let Some(num) = name.strip_prefix('#') {
            let v = if let Some(h) = num.strip_prefix(['x', 'X']) {
                u32::from_str_radix(h, 16).ok()
            } else {
                num.parse::<u32>().ok()
            };
            v.map(|v| match v {
                // Windows-1252 numbers given as if they were Unicode.
                0x80..=0x9f => cp1252(v as u8),
                0 => '\u{fffd}',
                v => char::from_u32(v).unwrap_or('\u{fffd}'),
            })
        } else {
            ENTITIES.get(name).copied()
        };
        match ch {
            Some(c) if !name.is_empty() => {
                out.push(c);
                rest = &rest[semi + usize::from(has_semi)..];
            }
            _ => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn cp1252(b: u8) -> char {
    let bytes = [b];
    let (c, _, _) = encoding_rs::WINDOWS_1252.decode(&bytes);
    c.chars().next().unwrap_or('\u{fffd}')
}

/// HTML 4's named characters, plus `apos`.
static ENTITIES: LazyLock<HashMap<&'static str, char>> = LazyLock::new(|| {
    const LIST: &[(&str, u32)] = &[
        ("quot", 34),
        ("amp", 38),
        ("apos", 39),
        ("lt", 60),
        ("gt", 62),
        ("nbsp", 160),
        ("iexcl", 161),
        ("cent", 162),
        ("pound", 163),
        ("curren", 164),
        ("yen", 165),
        ("brvbar", 166),
        ("sect", 167),
        ("uml", 168),
        ("copy", 169),
        ("ordf", 170),
        ("laquo", 171),
        ("not", 172),
        ("shy", 173),
        ("reg", 174),
        ("macr", 175),
        ("deg", 176),
        ("plusmn", 177),
        ("sup2", 178),
        ("sup3", 179),
        ("acute", 180),
        ("micro", 181),
        ("para", 182),
        ("middot", 183),
        ("cedil", 184),
        ("sup1", 185),
        ("ordm", 186),
        ("raquo", 187),
        ("frac14", 188),
        ("frac12", 189),
        ("frac34", 190),
        ("iquest", 191),
        ("Agrave", 192),
        ("Aacute", 193),
        ("Acirc", 194),
        ("Atilde", 195),
        ("Auml", 196),
        ("Aring", 197),
        ("AElig", 198),
        ("Ccedil", 199),
        ("Egrave", 200),
        ("Eacute", 201),
        ("Ecirc", 202),
        ("Euml", 203),
        ("Igrave", 204),
        ("Iacute", 205),
        ("Icirc", 206),
        ("Iuml", 207),
        ("ETH", 208),
        ("Ntilde", 209),
        ("Ograve", 210),
        ("Oacute", 211),
        ("Ocirc", 212),
        ("Otilde", 213),
        ("Ouml", 214),
        ("times", 215),
        ("Oslash", 216),
        ("Ugrave", 217),
        ("Uacute", 218),
        ("Ucirc", 219),
        ("Uuml", 220),
        ("Yacute", 221),
        ("THORN", 222),
        ("szlig", 223),
        ("agrave", 224),
        ("aacute", 225),
        ("acirc", 226),
        ("atilde", 227),
        ("auml", 228),
        ("aring", 229),
        ("aelig", 230),
        ("ccedil", 231),
        ("egrave", 232),
        ("eacute", 233),
        ("ecirc", 234),
        ("euml", 235),
        ("igrave", 236),
        ("iacute", 237),
        ("icirc", 238),
        ("iuml", 239),
        ("eth", 240),
        ("ntilde", 241),
        ("ograve", 242),
        ("oacute", 243),
        ("ocirc", 244),
        ("otilde", 245),
        ("ouml", 246),
        ("divide", 247),
        ("oslash", 248),
        ("ugrave", 249),
        ("uacute", 250),
        ("ucirc", 251),
        ("uuml", 252),
        ("yacute", 253),
        ("thorn", 254),
        ("yuml", 255),
        ("OElig", 338),
        ("oelig", 339),
        ("Scaron", 352),
        ("scaron", 353),
        ("Yuml", 376),
        ("fnof", 402),
        ("circ", 710),
        ("tilde", 732),
        ("Alpha", 913),
        ("Beta", 914),
        ("Gamma", 915),
        ("Delta", 916),
        ("Epsilon", 917),
        ("Zeta", 918),
        ("Eta", 919),
        ("Theta", 920),
        ("Iota", 921),
        ("Kappa", 922),
        ("Lambda", 923),
        ("Mu", 924),
        ("Nu", 925),
        ("Xi", 926),
        ("Omicron", 927),
        ("Pi", 928),
        ("Rho", 929),
        ("Sigma", 931),
        ("Tau", 932),
        ("Upsilon", 933),
        ("Phi", 934),
        ("Chi", 935),
        ("Psi", 936),
        ("Omega", 937),
        ("alpha", 945),
        ("beta", 946),
        ("gamma", 947),
        ("delta", 948),
        ("epsilon", 949),
        ("zeta", 950),
        ("eta", 951),
        ("theta", 952),
        ("iota", 953),
        ("kappa", 954),
        ("lambda", 955),
        ("mu", 956),
        ("nu", 957),
        ("xi", 958),
        ("omicron", 959),
        ("pi", 960),
        ("rho", 961),
        ("sigmaf", 962),
        ("sigma", 963),
        ("tau", 964),
        ("upsilon", 965),
        ("phi", 966),
        ("chi", 967),
        ("psi", 968),
        ("omega", 969),
        ("thetasym", 977),
        ("upsih", 978),
        ("piv", 982),
        ("ensp", 8194),
        ("emsp", 8195),
        ("thinsp", 8201),
        ("zwnj", 8204),
        ("zwj", 8205),
        ("lrm", 8206),
        ("rlm", 8207),
        ("ndash", 8211),
        ("mdash", 8212),
        ("lsquo", 8216),
        ("rsquo", 8217),
        ("sbquo", 8218),
        ("ldquo", 8220),
        ("rdquo", 8221),
        ("bdquo", 8222),
        ("dagger", 8224),
        ("Dagger", 8225),
        ("bull", 8226),
        ("hellip", 8230),
        ("permil", 8240),
        ("prime", 8242),
        ("Prime", 8243),
        ("lsaquo", 8249),
        ("rsaquo", 8250),
        ("oline", 8254),
        ("frasl", 8260),
        ("euro", 8364),
        ("image", 8465),
        ("weierp", 8472),
        ("real", 8476),
        ("trade", 8482),
        ("alefsym", 8501),
        ("larr", 8592),
        ("uarr", 8593),
        ("rarr", 8594),
        ("darr", 8595),
        ("harr", 8596),
        ("crarr", 8629),
        ("lArr", 8656),
        ("uArr", 8657),
        ("rArr", 8658),
        ("dArr", 8659),
        ("hArr", 8660),
        ("forall", 8704),
        ("part", 8706),
        ("exist", 8707),
        ("empty", 8709),
        ("nabla", 8711),
        ("isin", 8712),
        ("notin", 8713),
        ("ni", 8715),
        ("prod", 8719),
        ("sum", 8721),
        ("minus", 8722),
        ("lowast", 8727),
        ("radic", 8730),
        ("prop", 8733),
        ("infin", 8734),
        ("ang", 8736),
        ("and", 8743),
        ("or", 8744),
        ("cap", 8745),
        ("cup", 8746),
        ("int", 8747),
        ("there4", 8756),
        ("sim", 8764),
        ("cong", 8773),
        ("asymp", 8776),
        ("ne", 8800),
        ("equiv", 8801),
        ("le", 8804),
        ("ge", 8805),
        ("sub", 8834),
        ("sup", 8835),
        ("nsub", 8836),
        ("sube", 8838),
        ("supe", 8839),
        ("oplus", 8853),
        ("otimes", 8855),
        ("perp", 8869),
        ("sdot", 8901),
        ("lceil", 8968),
        ("rceil", 8969),
        ("lfloor", 8970),
        ("rfloor", 8971),
        ("lang", 9001),
        ("rang", 9002),
        ("loz", 9674),
        ("spades", 9824),
        ("clubs", 9827),
        ("hearts", 9829),
        ("diams", 9830),
    ];
    LIST.iter()
        .filter_map(|(n, c)| Some((*n, char::from_u32(*c)?)))
        .collect()
});

// ---------------------------------------------------------------- writing

/// Text for XML content.
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            c if is_xml_char(c) => o.push(c),
            _ => {}
        }
    }
    o
}

/// Text for an XML attribute in double quotes.
pub fn esc_attr(s: &str) -> String {
    esc(s).replace('"', "&quot;")
}

/// Characters XML 1.0 allows.
pub fn is_xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..)
        && c != '\u{feff}'
}

/// Elements kept in a book's XHTML; everything else gives its content.
fn kept(name: &str) -> bool {
    matches!(
        name,
        "p" | "div"
            | "span"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "em"
            | "i"
            | "strong"
            | "b"
            | "u"
            | "s"
            | "strike"
            | "del"
            | "ins"
            | "sub"
            | "sup"
            | "small"
            | "big"
            | "code"
            | "pre"
            | "kbd"
            | "samp"
            | "var"
            | "tt"
            | "blockquote"
            | "q"
            | "cite"
            | "abbr"
            | "dfn"
            | "mark"
            | "ul"
            | "ol"
            | "li"
            | "dl"
            | "dt"
            | "dd"
            | "table"
            | "thead"
            | "tbody"
            | "tfoot"
            | "tr"
            | "td"
            | "th"
            | "caption"
            | "colgroup"
            | "col"
            | "img"
            | "br"
            | "hr"
            | "a"
            | "figure"
            | "figcaption"
            | "section"
            | "article"
            | "aside"
            | "header"
            | "footer"
            | "nav"
            | "address"
            | "ruby"
            | "rt"
            | "rp"
            | "bdi"
            | "bdo"
            | "wbr"
    )
}

/// Elements dropped with their content.
fn dropped(name: &str) -> bool {
    matches!(
        name,
        "script"
            | "style"
            | "head"
            | "title"
            | "meta"
            | "link"
            | "noscript"
            | "iframe"
            | "object"
            | "embed"
            | "form"
            | "input"
            | "button"
            | "select"
            | "textarea"
            | "canvas"
            | "video"
            | "audio"
            | "map"
            | "area"
            | "template"
            | "math"
    )
}

/// CSS properties kept in `style` attributes: those that carry the
/// author's intent (alignment, emphasis, indents) rather than a page
/// layout. Sizes in `px` and `pt` go: the reader chooses.
fn kept_style(prop: &str, value: &str) -> bool {
    let fixed = value.contains("px")
        || value.contains("pt")
        || value.contains("cm")
        || value.contains("mm")
        || value.contains("in");
    match prop {
        "text-align" | "font-style" | "font-weight" | "text-decoration" | "font-variant"
        | "vertical-align" | "text-transform" | "white-space" | "direction" | "unicode-bidi"
        | "page-break-before" | "page-break-after" | "break-before" | "break-after"
        | "page-break-inside" | "list-style-type" | "float" | "clear" => true,
        "text-indent" | "margin-left" | "margin-right" | "padding-left" | "margin-top"
        | "margin-bottom" => !fixed,
        "width" => value.trim_end().ends_with('%'),
        _ => false,
    }
}

pub fn clean_style(style: &str) -> String {
    style
        .split(';')
        .filter_map(|d| {
            let (p, v) = d.split_once(':')?;
            let p = p.trim().to_ascii_lowercase();
            let v = v.trim();
            (kept_style(&p, &v.to_ascii_lowercase()) && !v.contains(['<', '>', '"', '{', '}']))
                .then(|| format!("{p}: {v}"))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// How links and pictures are rewritten while writing.
pub struct Out<'a> {
    /// A link's or picture's address as it is in the source → as it
    /// goes in the book (`None`: the attribute goes).
    pub href: &'a dyn Fn(&str, bool) -> Option<String>,
    /// Keep `class` attributes (their style sheet comes along).
    pub classes: bool,
}

/// Writes nodes as clean XHTML.
pub fn write(nodes: &[Node], out: &Out, s: &mut String) {
    for n in nodes {
        match n {
            Node::Text(t) => s.push_str(&esc(t)),
            Node::Elem(e) => write_elem(e, out, s),
        }
    }
}

fn write_elem(e: &Elem, out: &Out, s: &mut String) {
    let name = e.name.as_str();
    if dropped(name) {
        return;
    }
    // What the book hides (landmarks, page lists) stays hidden.
    if e.attr("hidden").is_some()
        || (name == "nav"
            && e.attr("epub:type")
                .is_some_and(|t| t.contains("landmarks") || t.contains("page-list")))
    {
        return;
    }
    // Old markup → its present form.
    let (name, extra_style): (&str, Option<String>) = match name {
        "center" => ("div", Some("text-align: center".into())),
        "font" => ("span", None),
        "strike" => ("s", None),
        "tt" => ("code", None),
        "big" => ("span", Some("font-size: larger".into())),
        "listing" | "xmp" | "plaintext" => ("pre", None),
        "mbp:pagebreak" => ("div", Some("page-break-before: always".into())),
        "image" => ("img", None),
        "svg" => {
            // A picture wrapped in SVG (common for covers): the picture.
            let mut imgs = Vec::new();
            find_all(&e.kids, "image", &mut imgs);
            for img in imgs {
                let src = img
                    .attr("href")
                    .or_else(|| img.attr("xlink:href"))
                    .unwrap_or_default();
                if let Some(src) = (out.href)(src, true) {
                    s.push_str(&format!("<img src=\"{}\" alt=\"\"/>", esc_attr(&src)));
                }
            }
            return;
        }
        n if n.starts_with("epub:") || n.starts_with("mbp:") => {
            write(&e.kids, out, s);
            return;
        }
        n => (n, None),
    };
    if !kept(name) && name != "img" {
        write(&e.kids, out, s);
        return;
    }
    let mut attrs: Vec<(String, String)> = Vec::new();
    let mut style = String::new();
    for (k, v) in &e.attrs {
        match k.as_str() {
            "id" if valid_id(v) => attrs.push(("id".into(), v.clone())),
            "class" if out.classes && !v.trim().is_empty() => {
                attrs.push(("class".into(), v.trim().into()))
            }
            "lang" | "xml:lang" if !v.is_empty() => {
                if !attrs.iter().any(|(k, _)| k == "xml:lang") {
                    attrs.push(("xml:lang".into(), v.clone()));
                }
            }
            "dir" if v == "rtl" || v == "ltr" => attrs.push(("dir".into(), v.clone())),
            "epub:type" if !v.trim().is_empty() => {
                attrs.push(("epub:type".into(), v.trim().into()))
            }
            "title" if !v.is_empty() => attrs.push(("title".into(), v.clone())),
            "colspan" | "rowspan" if matches!(name, "td" | "th") && v.parse::<u32>().is_ok() => {
                attrs.push((k.clone(), v.clone()))
            }
            "start" if name == "ol" && v.parse::<i64>().is_ok() => {
                attrs.push((k.clone(), v.clone()))
            }
            "value" if name == "li" && v.parse::<i64>().is_ok() => {
                attrs.push((k.clone(), v.clone()))
            }
            "type" if name == "ol" && matches!(v.as_str(), "1" | "a" | "A" | "i" | "I") => {
                attrs.push((k.clone(), v.clone()))
            }
            "href" | "filepos" if name == "a" => {
                if let Some(h) = (out.href)(v, false) {
                    attrs.retain(|(k, _)| k != "href");
                    attrs.push(("href".into(), h));
                }
            }
            "src" | "recindex" | "href" | "xlink:href" if name == "img" => {
                if !attrs.iter().any(|(k, _)| k == "src")
                    && let Some(h) = (out.href)(v, true)
                {
                    attrs.push(("src".into(), h));
                }
            }
            "alt" if name == "img" => attrs.push(("alt".into(), v.clone())),
            "align" => match v.to_ascii_lowercase().as_str() {
                a @ ("center" | "left" | "right" | "justify") if name != "img" => {
                    style.push_str(&format!("text-align: {a}; "))
                }
                _ => {}
            },
            "style" => {
                let c = clean_style(v);
                if !c.is_empty() {
                    style.push_str(&c);
                    style.push_str("; ");
                }
            }
            // Old Kindle books: a paragraph's first-line indent and the
            // space above it.
            "width" if name == "p" => {
                let v = v.trim();
                if v.ends_with("em") || v.ends_with('%') {
                    if !v.starts_with('-') {
                        style.push_str(&format!("text-indent: {v}; "));
                    }
                } else if v.parse::<f32>().is_ok_and(|n| n > 0.0) {
                    style.push_str("text-indent: 1.5em; ");
                } else if v.parse::<f32>().is_ok_and(|n| n <= 0.0) {
                    style.push_str("text-indent: 0; ");
                }
            }
            "height" if name == "p" => {
                let v = v.trim();
                if (v.ends_with("em") || v.ends_with('%')) && !v.starts_with('-') {
                    style.push_str(&format!("margin-top: {v}; "));
                } else if v.parse::<f32>().is_ok_and(|n| n > 0.0) {
                    style.push_str("margin-top: 1em; ");
                }
            }
            "width" if name == "img" || name == "hr" => {
                if let Some(p) = v.strip_suffix('%') {
                    style.push_str(&format!("width: {p}%; "));
                }
            }
            _ => {}
        }
    }
    if e.name == "font"
        && let Some(sz) = e.attr("size")
    {
        let em = match sz.trim() {
            "1" | "-2" => Some("0.7em"),
            "2" | "-1" => Some("0.85em"),
            "4" | "+1" => Some("1.15em"),
            "5" | "+2" => Some("1.4em"),
            "6" | "7" | "+3" | "+4" => Some("1.8em"),
            _ => None,
        };
        if let Some(em) = em {
            style.push_str(&format!("font-size: {em}; "));
        }
    }
    if let Some(x) = extra_style {
        style.push_str(&x);
        style.push_str("; ");
    }
    if name == "img" {
        if !attrs.iter().any(|(k, _)| k == "src") {
            return;
        }
        if !attrs.iter().any(|(k, _)| k == "alt") {
            attrs.push(("alt".into(), String::new()));
        }
    }
    let style = style.trim().trim_end_matches(';').trim().to_string();
    if !style.is_empty() {
        attrs.push(("style".into(), style));
    }
    // A `<span>` or `<font>` with nothing to say is just its content.
    if (name == "span") && attrs.is_empty() {
        write(&e.kids, out, s);
        return;
    }
    s.push('<');
    s.push_str(name);
    for (k, v) in &attrs {
        s.push_str(&format!(" {k}=\"{}\"", esc_attr(v)));
    }
    if is_void(name) {
        s.push_str("/>");
        return;
    }
    s.push('>');
    // Blocks inside a `<p>` are not allowed: a `<p>` holding one becomes
    // a `<div>`.
    if name == "pre" {
        // Raw text, its first newline kept.
        write(&e.kids, out, s);
    } else {
        write(&e.kids, out, s);
    }
    s.push_str("</");
    s.push_str(name);
    s.push('>');
}

/// An id a reader accepts: not empty, no spaces or markup characters
/// (HTML allows ids starting with a digit, and books use them).
pub fn valid_id(v: &str) -> bool {
    !v.is_empty()
        && !v.contains(|c: char| {
            c.is_whitespace() || matches!(c, '"' | '<' | '>' | '&' | '\'' | '#')
        })
}

/// Makes `nodes` valid where XHTML is strict: a `<p>` (or heading) with a
/// block inside is split around it; text right inside a list or table is
/// wrapped or dropped. Done in place, before writing.
pub fn tidy(nodes: &mut Vec<Node>) {
    let old = std::mem::take(nodes);
    for n in old {
        match n {
            Node::Elem(mut e) => {
                tidy(&mut e.kids);
                let inline_only = matches!(
                    e.name.as_str(),
                    "p" | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                        | "span"
                        | "a"
                        | "em"
                        | "i"
                        | "b"
                        | "strong"
                        | "u"
                        | "small"
                        | "sub"
                        | "sup"
                        | "code"
                        | "q"
                        | "cite"
                        | "font"
                        | "s"
                        | "strike"
                        | "big"
                        | "tt"
                );
                if inline_only && e.kids.iter().any(is_block_node) {
                    // Split: runs of inline content keep the element.
                    let mut run: Vec<Node> = Vec::new();
                    let flush = |run: &mut Vec<Node>, nodes: &mut Vec<Node>, e: &Elem| {
                        if run.iter().any(|n| !is_blank(n)) {
                            nodes.push(Node::Elem(Elem {
                                name: e.name.clone(),
                                attrs: e.attrs.clone(),
                                kids: std::mem::take(run),
                            }));
                        } else {
                            run.clear();
                        }
                    };
                    for k in std::mem::take(&mut e.kids) {
                        if is_block_node(&k) {
                            flush(&mut run, nodes, &e);
                            nodes.push(k);
                        } else {
                            run.push(k);
                        }
                    }
                    flush(&mut run, nodes, &e);
                    continue;
                }
                match e.name.as_str() {
                    "ul" | "ol" => {
                        e.kids = wrap_loose(std::mem::take(&mut e.kids), "li", &["li"]);
                    }
                    "dl" => e
                        .kids
                        .retain(|k| matches!(k, Node::Elem(x) if x.name == "dt" || x.name == "dd")),
                    "table" => {
                        // Rows straight in the table go in a `<tbody>`.
                        let mut kids = Vec::new();
                        let mut rows: Vec<Node> = Vec::new();
                        for k in std::mem::take(&mut e.kids) {
                            match &k {
                                Node::Elem(x) if x.name == "tr" => rows.push(k),
                                Node::Elem(x)
                                    if matches!(
                                        x.name.as_str(),
                                        "thead"
                                            | "tbody"
                                            | "tfoot"
                                            | "caption"
                                            | "colgroup"
                                            | "col"
                                    ) =>
                                {
                                    if !rows.is_empty() {
                                        kids.push(Node::Elem(Elem {
                                            name: "tbody".into(),
                                            attrs: vec![],
                                            kids: std::mem::take(&mut rows),
                                        }));
                                    }
                                    kids.push(k)
                                }
                                _ => {}
                            }
                        }
                        if !rows.is_empty() {
                            kids.push(Node::Elem(Elem {
                                name: "tbody".into(),
                                attrs: vec![],
                                kids: rows,
                            }));
                        }
                        e.kids = kids;
                    }
                    "thead" | "tbody" | "tfoot" => e
                        .kids
                        .retain(|k| matches!(k, Node::Elem(x) if x.name == "tr")),
                    "tr" => e
                        .kids
                        .retain(|k| matches!(k, Node::Elem(x) if x.name == "td" || x.name == "th")),
                    _ => {}
                }
                nodes.push(Node::Elem(e));
            }
            t => nodes.push(t),
        }
    }
}

fn wrap_loose(kids: Vec<Node>, wrap: &str, ok: &[&str]) -> Vec<Node> {
    let mut out = Vec::new();
    for k in kids {
        match &k {
            Node::Elem(x) if ok.contains(&x.name.as_str()) => out.push(k),
            n if is_blank(n) => {}
            _ => {
                // Loose content joins the item before, or makes one.
                if let Some(Node::Elem(last)) = out.last_mut()
                    && last.name == wrap
                {
                    last.kids.push(k);
                } else {
                    out.push(Node::Elem(Elem {
                        name: wrap.into(),
                        attrs: vec![],
                        kids: vec![k],
                    }));
                }
            }
        }
    }
    out
}

pub fn is_blank(n: &Node) -> bool {
    matches!(n, Node::Text(t) if t.trim().is_empty())
}

fn is_block_node(n: &Node) -> bool {
    matches!(n, Node::Elem(e) if is_block(&e.name) && e.name != "li" && e.name != "dt" && e.name != "dd"
        || matches!(n, Node::Elem(e) if e.name == "svg" || e.name == "mbp:pagebreak"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(html: &str) -> String {
        let (body, _) = body_of(parse(html));
        let mut body = crate::bookconv::text_in::wrap_loose(body);
        tidy(&mut body);
        let mut s = String::new();
        let keep = |h: &str, _pic: bool| Some(h.to_string());
        write(
            &body,
            &Out {
                href: &keep,
                classes: false,
            },
            &mut s,
        );
        s
    }

    #[test]
    fn old_markup_is_mended() {
        assert_eq!(
            clean("<html><body><p>One<p>Two &amp; three&nbsp;four<br>five</body>"),
            "<p>One</p><p>Two &amp; three\u{a0}four<br/>five</p>"
        );
        assert_eq!(
            clean("<p align=center><font size=+2><b>Title</b></font></p>"),
            "<p style=\"text-align: center\"><span style=\"font-size: 1.4em\"><b>Title</b></span></p>"
        );
        assert_eq!(
            clean("<ul><li>a<li>b</ul>"),
            "<ul><li>a</li><li>b</li></ul>"
        );
        assert_eq!(
            clean("<p>before<div>block</div>after</p>"),
            "<p>before</p><div>block</div><p>after</p>"
        );
        assert_eq!(
            clean("<table><tr><td>1<td>2</table>"),
            "<table><tbody><tr><td>1</td><td>2</td></tr></tbody></table>"
        );
    }

    #[test]
    fn scripts_and_unknown_tags_go() {
        assert_eq!(
            clean(
                "<script>var a = '<p>';</script><custom>kept <i>text</i></custom><style>p{}</style>"
            ),
            "<p>kept <i>text</i></p>"
        );
        assert_eq!(
            clean("<p style=\"font-size: 12pt; font-style: italic; color: red\">x</p>"),
            "<p style=\"font-style: italic\">x</p>"
        );
    }

    #[test]
    fn entities_and_numbers() {
        assert_eq!(
            decode("&eacute;&#233;&#xE9;&#150;&bogus;&amp"),
            "ééé–&bogus;&"
        );
        assert_eq!(esc("a<b & c\u{1}"), "a&lt;b &amp; c");
    }

    #[test]
    fn xhtml_empty_tags_stay_empty() {
        assert_eq!(
            clean("<p><a id=\"n1\"/>One <span class=\"x\"/>two</p><p>Three</p>"),
            "<p><a id=\"n1\"></a>One two</p><p>Three</p>"
        );
        // A script written as an empty tag does not swallow the page.
        assert_eq!(
            clean("<head><script src=\"a.js\"/></head><body><p>Text</p></body>"),
            "<p>Text</p>"
        );
        assert_eq!(
            close_empty_tags("<!-- <x/> --><br/><div/>"),
            "<!-- <x/> --><br/><div></div>"
        );
        assert_eq!(
            close_empty_tags("<p>a</p><mbp:pagebreak/><p>b</p><mbp:pagebreak>"),
            "<p>a</p><mbp:pagebreak></mbp:pagebreak><p>b</p><mbp:pagebreak></mbp:pagebreak>"
        );
    }

    #[test]
    fn misnested_markup_is_mended() {
        assert_eq!(
            clean("<p><b>bold <i>both</b> italic</i></p>"),
            "<p><b>bold <i>both</i></b><i> italic</i></p>"
        );
        assert_eq!(
            clean("<table><tr><td>1</td></tr>stray</table>"),
            "<p>stray</p><table><tbody><tr><td>1</td></tr></tbody></table>"
        );
    }

    #[test]
    fn svg_cover_becomes_a_picture() {
        assert_eq!(
            clean("<div><svg xmlns:xlink=\"x\"><image xlink:href=\"cover.jpg\"/></svg></div>"),
            "<div><img src=\"cover.jpg\" alt=\"\"/></div>"
        );
    }
}
