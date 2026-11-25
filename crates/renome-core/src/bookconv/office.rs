//! Word (DOCX) and OpenDocument (ODT) texts read into a [`Book`].
//!
//! What carries the author's intent is kept: headings (by style or outline
//! level) become chapters and the table of contents; italics, bold,
//! underline, small capitals, superscripts; lists with their numbering
//! kind; tables with merged cells; footnotes and endnotes (linked both
//! ways); pictures; centred and right-aligned paragraphs; page breaks;
//! links. What only fits a printed page goes: fonts, point sizes, exact
//! spacing, page margins. Empty paragraphs used as spacing become one
//! space between paragraphs.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use super::Book;
use super::text_in::{add_document, decode_bytes};
use super::xhtml::{self, Elem, Node};

struct Zip(std::cell::RefCell<zip::ZipArchive<std::fs::File>>);

impl Zip {
    fn open(path: &Path) -> Result<Zip, String> {
        let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
        Ok(Zip(std::cell::RefCell::new(
            zip::ZipArchive::new(f).map_err(|_| "bookBroken")?,
        )))
    }

    fn bytes(&self, name: &str) -> Option<Vec<u8>> {
        let mut z = self.0.borrow_mut();
        let name = name.trim_start_matches('/');
        // Names differ in case in some files.
        let real = if z.index_for_name(name).is_some() {
            name.to_string()
        } else {
            z.file_names()
                .filter_map(Result::ok)
                .find(|n| n.eq_ignore_ascii_case(name))?
                .to_string()
        };
        let mut f = z.by_name(&real).ok()?;
        let mut v = Vec::new();
        (&mut f).take(256 << 20).read_to_end(&mut v).ok()?;
        Some(v)
    }

    fn xml(&self, name: &str) -> Option<Vec<Node>> {
        let b = self.bytes(name)?;
        Some(xhtml::parse_xml(&decode_bytes(&b, true)))
    }
}

fn kids<'a>(e: &'a Elem, name: &'a str) -> impl Iterator<Item = &'a Elem> + 'a {
    e.kids.iter().filter_map(move |k| match k {
        Node::Elem(x) if x.name == name => Some(x),
        _ => None,
    })
}

fn kid<'a>(e: &'a Elem, name: &str) -> Option<&'a Elem> {
    e.kids.iter().find_map(|k| match k {
        Node::Elem(x) if x.name == name => Some(x),
        _ => None,
    })
}

/// A Word on/off property: present and not `false`/`0`/`none`.
fn on(e: Option<&Elem>) -> Option<bool> {
    let e = e?;
    Some(!matches!(
        e.attr("w:val"),
        Some("false" | "0" | "off" | "none")
    ))
}

fn el(name: &str) -> Elem {
    Elem::new(name)
}

/// Details from `docProps/core.xml` or `meta.xml` (Dublin Core).
fn dublin_core(nodes: &[Node], book: &mut Book) {
    let get = |n: &str| {
        xhtml::find(nodes, n)
            .map(|e| crate::text::collapse_spaces(e.plain().trim()))
            .filter(|s| !s.is_empty())
    };
    let m = &mut book.meta;
    if let Some(t) = get("title") {
        m.title = t;
    }
    if let Some(a) = get("creator").or_else(|| get("initial-creator")) {
        m.authors = a
            .split([';', ','])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        // A Word default like "User" or the computer's name says nothing;
        // kept only when it looks like a name (two words or more).
        if m.authors.iter().all(|a| !a.contains(' ')) {
            m.authors.clear();
        }
    }
    if let Some(l) = get("language") {
        m.language = l;
    }
    if let Some(d) = get("description") {
        m.description = d;
    }
    for k in [get("subject"), get("keywords"), get("keyword")]
        .into_iter()
        .flatten()
    {
        for w in k.split([',', ';']) {
            let w = w.trim();
            if !w.is_empty() && !m.subjects.iter().any(|s| s == w) {
                m.subjects.push(w.into());
            }
        }
    }
}

// ---------------------------------------------------------------- DOCX

#[derive(Default, Clone)]
struct WStyle {
    name: String,
    based_on: Option<String>,
    outline: Option<usize>,
    bold: Option<bool>,
    italic: Option<bool>,
    center: Option<String>,
    first_line: bool,
    after: bool,
}

struct Docx<'a> {
    styles: HashMap<String, WStyle>,
    /// Numbering instance → level → is ordered.
    ordered: HashMap<(String, String), bool>,
    rels: HashMap<String, (String, bool)>,
    pics: &'a HashMap<String, String>,
    notes: Vec<(String, Elem)>,
    note_n: usize,
    foot: HashMap<String, Elem>,
    end: HashMap<String, Elem>,
}

impl Docx<'_> {
    fn style(&self, id: &str) -> WStyle {
        // Properties come down the `basedOn` chain.
        let mut s = self.styles.get(id).cloned().unwrap_or_default();
        let mut parent = s.based_on.clone();
        let mut depth = 0;
        while let Some(p) = parent {
            depth += 1;
            if depth > 10 {
                break;
            }
            let Some(ps) = self.styles.get(&p) else { break };
            s.outline = s.outline.or(ps.outline);
            s.bold = s.bold.or(ps.bold);
            s.italic = s.italic.or(ps.italic);
            s.center = s.center.clone().or(ps.center.clone());
            parent = ps.based_on.clone();
        }
        s
    }

    /// The heading level a paragraph style stands for.
    fn heading(&self, id: &str) -> Option<usize> {
        let s = self.style(id);
        let name = s.name.to_lowercase();
        if name == "title" || name == "título" || name == "titulo" {
            return Some(1);
        }
        for p in ["heading ", "título ", "titulo ", "überschrift ", "titre "] {
            if let Some(n) = name
                .strip_prefix(p)
                .and_then(|n| n.trim().parse::<usize>().ok())
            {
                return Some(n.clamp(1, 6));
            }
        }
        s.outline.filter(|l| *l < 6).map(|l| l + 1)
    }
}

/// What `word/styles.xml` says: each style, and whether the default
/// paragraph indents its first line or leaves room after it.
struct DocxStyles {
    styles: HashMap<String, WStyle>,
    default_first_line: bool,
    default_after: bool,
}

fn word_styles(z: &Zip) -> DocxStyles {
    let mut out = DocxStyles {
        styles: HashMap::new(),
        default_first_line: false,
        default_after: false,
    };
    let Some(st) = z.xml("word/styles.xml") else {
        return out;
    };
    let mut all = Vec::new();
    xhtml::find_all(&st, "style", &mut all);
    for s in all {
        let Some(id) = s.attr("w:styleid") else {
            continue;
        };
        let ppr = kid(s, "ppr");
        let rpr = kid(s, "rpr");
        let ws = WStyle {
            name: kid(s, "name")
                .and_then(|n| n.attr("w:val"))
                .unwrap_or_default()
                .to_string(),
            based_on: kid(s, "basedon")
                .and_then(|n| n.attr("w:val"))
                .map(String::from),
            outline: ppr
                .and_then(|p| kid(p, "outlinelvl"))
                .and_then(|o| o.attr("w:val"))
                .and_then(|v| v.parse().ok()),
            bold: on(rpr.and_then(|r| kid(r, "b"))),
            italic: on(rpr.and_then(|r| kid(r, "i"))),
            center: ppr
                .and_then(|p| kid(p, "jc"))
                .and_then(|j| j.attr("w:val"))
                .map(String::from),
            first_line: ppr
                .and_then(|p| kid(p, "ind"))
                .and_then(|i| i.attr("w:firstline").or_else(|| i.attr("w:firstlinechars")))
                .is_some_and(|v| v.parse::<i64>().unwrap_or(0) > 0),
            after: ppr
                .and_then(|p| kid(p, "spacing"))
                .and_then(|i| i.attr("w:after"))
                .is_some_and(|v| v.parse::<i64>().unwrap_or(0) > 60),
        };
        if s.attr("w:default") == Some("1") && s.attr("w:type") == Some("paragraph") {
            out.default_first_line = ws.first_line;
            out.default_after = ws.after;
        }
        out.styles.insert(id.to_string(), ws);
    }
    if let Some(d) = xhtml::find(&st, "pprdefault")
        && let Some(sp) = xhtml::find(&d.kids, "spacing")
        && sp
            .attr("w:after")
            .is_some_and(|v| v.parse::<i64>().unwrap_or(0) > 60)
    {
        out.default_after = true;
    }
    out
}

/// Lists: which levels of each list are numbered (`(numId, level)`).
fn word_numbering(z: &Zip) -> HashMap<(String, String), bool> {
    let mut ordered = HashMap::new();
    let Some(num) = z.xml("word/numbering.xml") else {
        return ordered;
    };
    let mut abstracts: HashMap<String, HashMap<String, bool>> = HashMap::new();
    let mut all = Vec::new();
    xhtml::find_all(&num, "abstractnum", &mut all);
    for a in all {
        let Some(id) = a.attr("w:abstractnumid") else {
            continue;
        };
        let mut lv = HashMap::new();
        for l in kids(a, "lvl") {
            let fmt = kid(l, "numfmt")
                .and_then(|f| f.attr("w:val"))
                .unwrap_or("bullet");
            lv.insert(
                l.attr("w:ilvl").unwrap_or("0").to_string(),
                !matches!(fmt, "bullet" | "none"),
            );
        }
        abstracts.insert(id.into(), lv);
    }
    let mut nums = Vec::new();
    xhtml::find_all(&num, "num", &mut nums);
    for n in nums {
        let (Some(id), Some(a)) = (
            n.attr("w:numid"),
            kid(n, "abstractnumid").and_then(|a| a.attr("w:val")),
        ) else {
            continue;
        };
        for (l, o) in abstracts.get(a).cloned().unwrap_or_default() {
            ordered.insert((id.to_string(), l), o);
        }
    }
    ordered
}

/// Relationships: the target of each id, and whether it is outside.
fn word_rels(z: &Zip) -> HashMap<String, (String, bool)> {
    let mut rels = HashMap::new();
    if let Some(r) = z.xml("word/_rels/document.xml.rels") {
        let mut all = Vec::new();
        xhtml::find_all(&r, "relationship", &mut all);
        for x in all {
            if let (Some(id), Some(t)) = (x.attr("id"), x.attr("target")) {
                rels.insert(
                    id.to_string(),
                    (t.to_string(), x.attr("targetmode") == Some("External")),
                );
            }
        }
    }
    rels
}

/// The footnotes or endnotes of a file, by id.
fn word_notes(z: &Zip, file: &str, tag: &str) -> HashMap<String, Elem> {
    let mut m = HashMap::new();
    if let Some(n) = z.xml(file) {
        let mut all = Vec::new();
        xhtml::find_all(&n, tag, &mut all);
        for e in all {
            if let Some(id) = e.attr("w:id")
                && e.attr("w:type").is_none_or(|t| t == "normal")
            {
                m.insert(id.to_string(), e.clone());
            }
        }
    }
    m
}

/// A Word document.
pub fn docx(path: &Path) -> Result<Book, String> {
    let z = Zip::open(path)?;
    let doc = z.xml("word/document.xml").ok_or("bookBroken")?;
    let mut book = Book::new();
    if let Some(core) = z.xml("docProps/core.xml") {
        dublin_core(&core, &mut book);
    }
    let DocxStyles {
        styles,
        default_first_line,
        default_after,
    } = word_styles(&z);
    let ordered = word_numbering(&z);
    let rels = word_rels(&z);
    let mut pics = HashMap::new();
    for (id, (t, ext)) in &rels {
        if *ext || !t.contains("media/") {
            continue;
        }
        if let Some(data) = z.bytes(&crate::ebook::join_zip("word/document.xml", t))
            && let Some(name) = book.add_file(data, &t.to_ascii_lowercase())
        {
            pics.insert(id.clone(), name);
        }
    }
    let mut d = Docx {
        styles,
        ordered,
        rels,
        pics: &pics,
        notes: Vec::new(),
        note_n: 0,
        foot: word_notes(&z, "word/footnotes.xml", "footnote"),
        end: word_notes(&z, "word/endnotes.xml", "endnote"),
    };
    let body = xhtml::find(&doc, "body").ok_or("bookBroken")?;
    let mut nodes = d.blocks(&body.kids);
    if book.meta.language.is_empty()
        && let Some(l) = xhtml::find(&doc, "lang").and_then(|l| l.attr("w:val"))
    {
        book.meta.language = l.into();
    }
    spacing(&mut nodes);
    if !default_first_line && default_after {
        book.css
            .push_str("\np { text-indent: 0; margin: 0 0 0.6em; }\n");
    }
    append_notes(&mut nodes, std::mem::take(&mut d.notes));
    finish(book, nodes)
}

fn finish(mut book: Book, nodes: Vec<Node>) -> Result<Book, String> {
    if nodes.iter().all(xhtml::is_blank) {
        return Err("bookBroken".into());
    }
    let keep = |h: &str, _pic: bool| Some(h.to_string());
    add_document(&mut book, nodes, &keep, true);
    if book.chapters.is_empty() {
        return Err("bookBroken".into());
    }
    super::fix_links(&mut book);
    Ok(book)
}

/// Notes go at the end, each with its number linking back.
fn append_notes(nodes: &mut Vec<Node>, notes: Vec<(String, Elem)>) {
    if notes.is_empty() {
        return;
    }
    let mut sec = el("section")
        .with("class", "notes")
        .with("epub:type", "footnotes");
    sec.push(Node::Elem({
        let mut h = el("h2");
        h.text("Notes");
        h
    }));
    for (id, mut note) in notes {
        note.set("id", id.clone());
        note.set("class", "note");
        note.set("epub:type", "footnote");
        sec.push(Node::Elem(note));
    }
    nodes.push(Node::Elem(sec));
}

/// Single empty paragraphs between paragraphs are spacing; with most
/// paragraphs spaced so, they go; else the next paragraph gets a space
/// above. Runs of them are a scene break.
fn spacing(nodes: &mut Vec<Node>) {
    let empty = |n: &Node| matches!(n, Node::Elem(e) if e.name == "p" && e.plain().trim().is_empty() && !has_img(e));
    let total = nodes
        .iter()
        .filter(|n| matches!(n, Node::Elem(e) if e.name == "p"))
        .count();
    let blanks = nodes.iter().filter(|n| empty(n)).count();
    let separators = blanks * 3 > total;
    let old = std::mem::take(nodes);
    let mut run = 0;
    for n in old {
        if empty(&n) {
            run += 1;
            continue;
        }
        if let Node::Elem(mut e) = n {
            if run > 0 && e.name == "p" && (!separators || run > 1) {
                let c = e
                    .attr("class")
                    .map(|c| format!("{c} space"))
                    .unwrap_or_else(|| "space".into());
                e.set("class", c);
            }
            nodes.push(Node::Elem(e));
        } else {
            nodes.push(n);
        }
        run = 0;
    }
}

fn has_img(e: &Elem) -> bool {
    let mut v = Vec::new();
    xhtml::find_all(&e.kids, "img", &mut v);
    !v.is_empty()
}

/// The lists open while reading: (numId, list element) per level.
#[derive(Default)]
struct Lists(Vec<(String, Elem)>);

impl Lists {
    /// Closes the lists deeper than `to`, each into its parent's last
    /// item (or the output).
    fn close_to(&mut self, out: &mut Vec<Node>, to: usize) {
        while self.0.len() > to {
            let (_, l) = self.0.pop().expect("the list is longer than `to`");
            match self.0.last_mut() {
                Some((_, parent)) => {
                    // A nested list goes in the parent's last item.
                    if let Some(Node::Elem(li)) = parent.kids.last_mut() {
                        li.push(Node::Elem(l));
                    } else {
                        parent.push(Node::Elem(l));
                    }
                }
                None => out.push(Node::Elem(l)),
            }
        }
    }

    /// Opens the lists down to level `lvl` of list `id`, closing another
    /// list's first.
    fn open_to(&mut self, out: &mut Vec<Node>, id: &str, lvl: usize, ordered: bool) {
        if self.0.first().is_some_and(|(nid, _)| nid != id) {
            self.close_to(out, 0);
        }
        self.close_to(out, lvl + 1);
        while self.0.len() < lvl + 1 {
            self.0
                .push((id.to_string(), el(if ordered { "ol" } else { "ul" })));
        }
    }

    fn add_item(&mut self, li: Elem) {
        self.0
            .last_mut()
            .expect("the list is never empty here")
            .1
            .push(Node::Elem(li));
    }
}

impl Docx<'_> {
    fn blocks(&mut self, nodes: &[Node]) -> Vec<Node> {
        let mut out: Vec<Node> = Vec::new();
        let mut lists = Lists::default();
        for n in nodes {
            let Node::Elem(e) = n else { continue };
            match e.name.as_str() {
                "p" => {
                    if let Some((id, lvl)) = self.list_place(e) {
                        let ordered = *self
                            .ordered
                            .get(&(id.clone(), lvl.to_string()))
                            .unwrap_or(&false);
                        lists.open_to(&mut out, &id, lvl, ordered);
                        let mut li = el("li");
                        li.kids = self.runs(&e.kids);
                        lists.add_item(li);
                        continue;
                    }
                    lists.close_to(&mut out, 0);
                    out.extend(self.paragraph(e));
                }
                "tbl" => {
                    lists.close_to(&mut out, 0);
                    out.push(Node::Elem(self.table(e)));
                }
                "sdt" => {
                    lists.close_to(&mut out, 0);
                    if let Some(c) = kid(e, "sdtcontent") {
                        out.extend(self.blocks(&c.kids));
                    }
                }
                "customxml" | "smarttag" | "ins" => {
                    lists.close_to(&mut out, 0);
                    out.extend(self.blocks(&e.kids));
                }
                "alternatecontent" => {
                    if let Some(c) = kid(e, "choice").or_else(|| kid(e, "fallback")) {
                        out.extend(self.blocks(&c.kids));
                    }
                }
                _ => {}
            }
        }
        lists.close_to(&mut out, 0);
        out
    }

    /// The list and level a paragraph is an item of (none: a heading or
    /// not in a list).
    fn list_place(&self, e: &Elem) -> Option<(String, usize)> {
        let ppr = kid(e, "ppr");
        let num = ppr.and_then(|p| kid(p, "numpr"));
        let id = num
            .and_then(|n| kid(n, "numid"))
            .and_then(|x| x.attr("w:val"))
            .filter(|v| *v != "0")?;
        let style = ppr
            .and_then(|p| kid(p, "pstyle"))
            .and_then(|s| s.attr("w:val"))
            .unwrap_or("");
        let is_heading =
            self.heading(style).is_some() || ppr.and_then(|p| kid(p, "outlinelvl")).is_some();
        if is_heading {
            return None;
        }
        let lvl: usize = num
            .and_then(|n| kid(n, "ilvl"))
            .and_then(|x| x.attr("w:val"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        Some((id.to_string(), lvl))
    }

    fn paragraph(&mut self, e: &Elem) -> Vec<Node> {
        let ppr = kid(e, "ppr");
        let style_id = ppr
            .and_then(|p| kid(p, "pstyle"))
            .and_then(|s| s.attr("w:val"))
            .unwrap_or("");
        let style = self.style(style_id);
        let sname = style.name.to_lowercase();
        let level = self.heading(style_id).or_else(|| {
            ppr.and_then(|p| kid(p, "outlinelvl"))
                .and_then(|o| o.attr("w:val"))
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|l| *l < 6)
                .map(|l| l + 1)
        });
        let tag = match level {
            Some(l) => format!("h{l}"),
            None if sname.contains("quote")
                || sname.contains("citação")
                || sname.contains("cita") =>
            {
                "blockquote".into()
            }
            None => "p".into(),
        };
        let mut p = el(if tag == "blockquote" { "p" } else { &tag });
        p.kids = self.runs(&e.kids);
        let jc = ppr
            .and_then(|p| kid(p, "jc"))
            .and_then(|j| j.attr("w:val"))
            .map(String::from)
            .or(style.center);
        let mut classes = Vec::new();
        match jc.as_deref() {
            Some("center") if level.is_none() => classes.push("center"),
            Some("right" | "end") => classes.push("right"),
            _ => {}
        }
        if sname == "subtitle" || sname == "subtítulo" {
            classes.push("center");
        }
        if !classes.is_empty() {
            p.set("class", classes.join(" "));
        }
        let mut out = Vec::new();
        let page_before = ppr
            .and_then(|p| kid(p, "pagebreakbefore"))
            .is_some_and(|b| on(Some(b)) == Some(true));
        if page_before
            || p.kids
                .iter()
                .any(|k| matches!(k, Node::Elem(x) if x.name == "pagebreak"))
        {
            out.push(Node::Elem(
                el("div").with("style", "page-break-before: always"),
            ));
            p.kids
                .retain(|k| !matches!(k, Node::Elem(x) if x.name == "pagebreak"));
        }
        // Text boxes inside the paragraph come after it.
        let mut boxes = Vec::new();
        xhtml::find_all(&e.kids, "txbxcontent", &mut boxes);
        let boxes: Vec<Elem> = boxes.into_iter().cloned().collect();
        if tag == "blockquote" {
            let mut q = el("blockquote");
            q.push(Node::Elem(p));
            out.push(Node::Elem(q));
        } else {
            out.push(Node::Elem(p));
        }
        for b in boxes {
            out.extend(self.blocks(&b.kids));
        }
        out
    }

    fn table(&mut self, t: &Elem) -> Elem {
        let mut table = el("table");
        let mut body = el("tbody");
        for tr in kids(t, "tr") {
            let mut row = el("tr");
            for tc in kids(tr, "tc") {
                let pr = kid(tc, "tcpr");
                // A cell merged with the one above is not written.
                if pr
                    .and_then(|p| kid(p, "vmerge"))
                    .is_some_and(|v| v.attr("w:val").is_none_or(|v| v == "continue"))
                    && pr
                        .and_then(|p| kid(p, "vmerge"))
                        .and_then(|v| v.attr("w:val"))
                        != Some("restart")
                {
                    continue;
                }
                let mut cell = el("td");
                if let Some(span) = pr
                    .and_then(|p| kid(p, "gridspan"))
                    .and_then(|g| g.attr("w:val"))
                {
                    cell.set("colspan", span);
                }
                let inner = self.blocks(&tc.kids);
                // One paragraph: its content right in the cell.
                if inner.len() == 1
                    && let Some(Node::Elem(p)) = inner.first()
                    && p.name == "p"
                {
                    cell.kids = p.kids.clone();
                } else {
                    cell.kids = inner;
                }
                row.push(Node::Elem(cell));
            }
            body.push(Node::Elem(row));
        }
        table.push(Node::Elem(body));
        table
    }

    fn runs(&mut self, nodes: &[Node]) -> Vec<Node> {
        let mut out: Vec<Node> = Vec::new();
        for n in nodes {
            let Node::Elem(e) = n else { continue };
            match e.name.as_str() {
                "r" => out.extend(self.run(e)),
                "hyperlink" => {
                    let inner = self.runs(&e.kids);
                    let href = e
                        .attr("r:id")
                        .and_then(|id| self.rels.get(id))
                        .filter(|(_, ext)| *ext)
                        .map(|(t, _)| t.clone())
                        .or_else(|| e.attr("w:anchor").map(|a| format!("#{a}")));
                    match href {
                        Some(h) => {
                            let mut a = el("a").with("href", h);
                            a.kids = inner;
                            out.push(Node::Elem(a));
                        }
                        None => out.extend(inner),
                    }
                }
                "bookmarkstart" => {
                    if let Some(name) = e
                        .attr("w:name")
                        .filter(|n| xhtml::valid_id(n) && !n.starts_with("_GoBack"))
                    {
                        out.push(Node::Elem(el("span").with("id", name)));
                    }
                }
                "ins" | "smarttag" | "customxml" | "fldsimple" | "sdtcontent" => {
                    out.extend(self.runs(&e.kids))
                }
                "sdt" => {
                    if let Some(c) = kid(e, "sdtcontent") {
                        out.extend(self.runs(&c.kids));
                    }
                }
                "alternatecontent" => {
                    if let Some(c) = kid(e, "choice").or_else(|| kid(e, "fallback")) {
                        out.extend(self.runs(&c.kids));
                    }
                }
                _ => {}
            }
        }
        merge_text(out)
    }

    fn run(&mut self, r: &Elem) -> Vec<Node> {
        let rpr = kid(r, "rpr");
        let rstyle = rpr
            .and_then(|p| kid(p, "rstyle"))
            .and_then(|s| s.attr("w:val"))
            .map(|s| self.style(s));
        let flag = |n: &str| on(rpr.and_then(|p| kid(p, n)));
        let bold = flag("b")
            .or(rstyle.as_ref().and_then(|s| s.bold))
            .unwrap_or(false);
        let italic = flag("i")
            .or(rstyle.as_ref().and_then(|s| s.italic))
            .unwrap_or(false);
        let underline = rpr
            .and_then(|p| kid(p, "u"))
            .is_some_and(|u| !matches!(u.attr("w:val"), Some("none") | None));
        let strike = flag("strike").unwrap_or(false) || flag("dstrike").unwrap_or(false);
        let small_caps = flag("smallcaps").unwrap_or(false);
        let vert = rpr
            .and_then(|p| kid(p, "vertalign"))
            .and_then(|v| v.attr("w:val"));
        let hidden = flag("vanish").unwrap_or(false);
        if hidden {
            return Vec::new();
        }
        let content = self.run_content(r, rpr);
        // Marks around the text, innermost first.
        let has_text = content
            .iter()
            .any(|n| matches!(n, Node::Text(t) if !t.trim().is_empty()));
        if !has_text {
            return content;
        }
        let mut marks: Vec<(&str, Option<&str>)> = Vec::new();
        if small_caps {
            marks.push(("span", Some("font-variant: small-caps")));
        }
        match vert {
            Some("superscript") => marks.push(("sup", None)),
            Some("subscript") => marks.push(("sub", None)),
            _ => {}
        }
        if strike {
            marks.push(("s", None));
        }
        if underline {
            marks.push(("u", None));
        }
        if italic {
            marks.push(("em", None));
        }
        if bold {
            marks.push(("strong", None));
        }
        wrap_marks(content, &marks)
    }

    /// What a run holds: text, tabs, breaks, symbols, note references and
    /// pictures.
    fn run_content(&mut self, r: &Elem, rpr: Option<&Elem>) -> Vec<Node> {
        let mut content: Vec<Node> = Vec::new();
        for k in &r.kids {
            let Node::Elem(e) = k else { continue };
            match e.name.as_str() {
                "t" => content.push(Node::Text(e.plain())),
                "tab" => content.push(Node::Text("\u{2003}".into())),
                "br" | "cr" => {
                    if e.attr("w:type") == Some("page") {
                        content.push(Node::Elem(el("pagebreak")));
                    } else {
                        content.push(Node::Elem(el("br")));
                    }
                }
                "nobreakhyphen" => content.push(Node::Text("\u{2011}".into())),
                "softhyphen" => content.push(Node::Text("\u{ad}".into())),
                "sym" => {
                    if let Some(c) = e
                        .attr("w:char")
                        .and_then(|c| u32::from_str_radix(c, 16).ok())
                    {
                        // Symbol fonts map letters to the private area.
                        let c = if (0xf020..=0xf0ff).contains(&c) {
                            c - 0xf000
                        } else {
                            c
                        };
                        if let Some(ch) = char::from_u32(c) {
                            content.push(Node::Text(ch.to_string()));
                        }
                    }
                }
                "footnotereference" | "endnotereference" => {
                    if let Some(a) = self.note_reference(e) {
                        content.push(a);
                    }
                }
                "drawing" | "pict" | "object" => {
                    let mut ids = Vec::new();
                    collect_embeds(&e.kids, &mut ids);
                    for id in ids {
                        if let Some(name) = self.pics.get(&id) {
                            content.push(Node::Elem(
                                el("img").with("src", name.clone()).with("alt", ""),
                            ));
                        }
                    }
                }
                "alternatecontent" => {
                    if let Some(c) = kid(e, "choice").or_else(|| kid(e, "fallback")) {
                        let mut wrapper = Elem::new("r");
                        wrapper.attrs = r.attrs.clone();
                        if let Some(p) = rpr {
                            wrapper.push(Node::Elem(p.clone()));
                        }
                        wrapper.kids.extend(c.kids.iter().cloned());
                        content.extend(self.run(&wrapper));
                    }
                }
                _ => {}
            }
        }
        content
    }

    /// A reference to a foot- or endnote: the link in the text; the note
    /// (with a link back) is kept for the end of the book.
    fn note_reference(&mut self, e: &Elem) -> Option<Node> {
        let id = e.attr("w:id").unwrap_or_default();
        let src = if e.name == "footnotereference" {
            self.foot.get(id)
        } else {
            self.end.get(id)
        };
        let note = src.cloned()?;
        self.note_n += 1;
        let n = self.note_n;
        let nid = format!("note{n}");
        let mut a = el("a")
            .with("href", format!("#{nid}"))
            .with("id", format!("ref{n}"))
            .with("class", "noteref")
            .with("epub:type", "noteref");
        a.text(n.to_string());
        let mut body = self.blocks(&note.kids);
        // The note's own number mark is replaced by a link back.
        let mut back = el("a")
            .with("href", format!("#ref{n}"))
            .with("class", "back");
        back.text(format!("{n}."));
        if let Some(Node::Elem(first)) = body.first_mut() {
            first.kids.insert(0, Node::Text(" ".into()));
            first.kids.insert(0, Node::Elem(back));
        } else {
            let mut p = el("p");
            p.push(Node::Elem(back));
            body.push(Node::Elem(p));
        }
        let mut aside = el("aside");
        aside.kids = body;
        self.notes.push((nid, aside));
        Some(Node::Elem(a))
    }
}

/// `content` inside each mark in turn, the first innermost.
fn wrap_marks(mut content: Vec<Node>, marks: &[(&str, Option<&str>)]) -> Vec<Node> {
    for (tag, style) in marks {
        let mut x = el(tag);
        if let Some(c) = *style {
            x.set("style", c);
        }
        x.kids = std::mem::take(&mut content);
        content = vec![Node::Elem(x)];
    }
    content
}

/// Picture relationship ids inside a drawing.
fn collect_embeds(nodes: &[Node], out: &mut Vec<String>) {
    for n in nodes {
        if let Node::Elem(e) = n {
            if let Some(id) = e.attr("r:embed").or_else(|| {
                if e.name == "imagedata" {
                    e.attr("r:id")
                } else {
                    None
                }
            }) {
                out.push(id.to_string());
            }
            collect_embeds(&e.kids, out);
        }
    }
}

/// Neighbouring runs with the same marks joined (`<em>a</em><em>b</em>`
/// → `<em>ab</em>`), text runs joined.
fn merge_text(nodes: Vec<Node>) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::new();
    for n in nodes {
        match (out.last_mut(), n) {
            (Some(Node::Text(a)), Node::Text(b)) => a.push_str(&b),
            (Some(Node::Elem(a)), Node::Elem(b))
                if a.name == b.name
                    && a.attrs == b.attrs
                    && matches!(a.name.as_str(), "em" | "strong" | "u" | "s" | "sup" | "sub") =>
            {
                a.kids.extend(b.kids);
                a.kids = merge_text(std::mem::take(&mut a.kids));
            }
            (_, n) => out.push(n),
        }
    }
    out
}

// ---------------------------------------------------------------- ODT

#[derive(Default, Clone)]
struct OStyle {
    parent: Option<String>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    strike: Option<bool>,
    small_caps: Option<bool>,
    position: Option<String>,
    align: Option<String>,
    break_before: bool,
    outline: Option<usize>,
    display: String,
}

struct Odt<'a> {
    styles: HashMap<String, OStyle>,
    /// List style → level → numbered.
    lists: HashMap<String, HashMap<usize, bool>>,
    pics: &'a HashMap<String, String>,
    notes: Vec<(String, Elem)>,
    note_n: usize,
}

fn ostyles(
    nodes: &[Node],
    styles: &mut HashMap<String, OStyle>,
    lists: &mut HashMap<String, HashMap<usize, bool>>,
) {
    let mut all = Vec::new();
    xhtml::find_all(nodes, "style", &mut all);
    for s in all {
        let Some(name) = s.attr("style:name") else {
            continue;
        };
        let tp = kid(s, "text-properties");
        let pp = kid(s, "paragraph-properties");
        let tattr = |a: &str| tp.and_then(|t| t.attr(a));
        styles.insert(
            name.to_string(),
            OStyle {
                parent: s.attr("style:parent-style-name").map(String::from),
                bold: tattr("fo:font-weight")
                    .map(|w| w == "bold" || w.parse::<u32>().is_ok_and(|n| n >= 600)),
                italic: tattr("fo:font-style").map(|v| v == "italic" || v == "oblique"),
                underline: tattr("style:text-underline-style").map(|v| v != "none"),
                strike: tattr("style:text-line-through-style").map(|v| v != "none"),
                small_caps: tattr("fo:font-variant").map(|v| v == "small-caps"),
                position: tattr("style:text-position").map(String::from),
                align: pp.and_then(|p| p.attr("fo:text-align")).map(String::from),
                break_before: pp.and_then(|p| p.attr("fo:break-before")) == Some("page"),
                outline: s
                    .attr("style:default-outline-level")
                    .and_then(|v| v.parse().ok()),
                display: s.attr("style:display-name").unwrap_or(name).to_string(),
            },
        );
    }
    let mut ls = Vec::new();
    xhtml::find_all(nodes, "list-style", &mut ls);
    for l in ls {
        let Some(name) = l.attr("style:name") else {
            continue;
        };
        let mut lv = HashMap::new();
        for k in &l.kids {
            if let Node::Elem(x) = k {
                let level = x
                    .attr("text:level")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1usize);
                lv.insert(level, x.name == "list-level-style-number");
            }
        }
        lists.insert(name.to_string(), lv);
    }
}

impl Odt<'_> {
    fn style(&self, name: &str) -> OStyle {
        let mut s = self.styles.get(name).cloned().unwrap_or_default();
        let mut parent = s.parent.clone();
        let mut depth = 0;
        while let Some(p) = parent {
            depth += 1;
            if depth > 10 {
                break;
            }
            let Some(ps) = self.styles.get(&p) else { break };
            s.bold = s.bold.or(ps.bold);
            s.italic = s.italic.or(ps.italic);
            s.underline = s.underline.or(ps.underline);
            s.strike = s.strike.or(ps.strike);
            s.small_caps = s.small_caps.or(ps.small_caps);
            s.position = s.position.clone().or(ps.position.clone());
            s.align = s.align.clone().or(ps.align.clone());
            s.outline = s.outline.or(ps.outline);
            if s.display.is_empty()
                || self
                    .styles
                    .get(name)
                    .is_some_and(|x| x.parent.is_some() && name.starts_with('P'))
            {
                s.display = ps.display.clone();
            }
            parent = ps.parent.clone();
        }
        s
    }

    fn blocks(&mut self, nodes: &[Node], list_style: Option<&str>, depth: usize) -> Vec<Node> {
        let mut out = Vec::new();
        for n in nodes {
            let Node::Elem(e) = n else { continue };
            match e.name.as_str() {
                "h" => self.heading(e, &mut out),
                "p" => self.paragraph(e, depth, &mut out),
                "list" => out.push(Node::Elem(self.list(e, list_style, depth))),
                "table" => out.push(Node::Elem(self.table(e, depth))),
                "section" | "index-body" | "illustration-index" => {
                    out.extend(self.blocks(&e.kids, None, depth))
                }
                "table-of-content" | "alphabetical-index" | "user-index" | "bibliography"
                | "tracked-changes" | "sequence-decls" | "variable-decls" | "user-field-decls" => {}
                _ => {}
            }
        }
        out
    }

    /// A heading (`text:h`).
    fn heading(&mut self, e: &Elem, out: &mut Vec<Node>) {
        let level = e
            .attr("text:outline-level")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(1)
            .clamp(1, 6);
        let st = self.style(e.attr("text:style-name").unwrap_or(""));
        if st.break_before {
            out.push(Node::Elem(
                el("div").with("style", "page-break-before: always"),
            ));
        }
        let mut h = el(&format!("h{level}"));
        h.kids = self.inline(&e.kids);
        out.push(Node::Elem(h));
    }

    /// A paragraph: a heading when its style says so, a quotation, a
    /// centered or right-aligned one; text boxes in it come after it.
    fn paragraph(&mut self, e: &Elem, depth: usize, out: &mut Vec<Node>) {
        let sname = e.attr("text:style-name").unwrap_or("");
        let st = self.style(sname);
        if st.break_before {
            out.push(Node::Elem(
                el("div").with("style", "page-break-before: always"),
            ));
        }
        let display = st.display.to_lowercase();
        let level = st.outline.or_else(|| {
            ["heading ", "título ", "titulo "]
                .iter()
                .find_map(|p| display.strip_prefix(p).and_then(|n| n.trim().parse().ok()))
        });
        let mut kids = self.inline(&e.kids);
        let mut after = Vec::new();
        // Frames with text boxes or pictures as blocks.
        let mut boxes = Vec::new();
        xhtml::find_all(&e.kids, "text-box", &mut boxes);
        for b in boxes.into_iter().cloned().collect::<Vec<_>>() {
            after.extend(self.blocks(&b.kids, None, depth));
        }
        if display == "title" || display == "título" {
            let mut h = el("h1");
            h.kids = std::mem::take(&mut kids);
            out.push(Node::Elem(h));
        } else if let Some(l) = level.filter(|l| *l >= 1 && *l <= 6) {
            let mut h = el(&format!("h{l}"));
            h.kids = kids;
            out.push(Node::Elem(h));
        } else {
            let mut p = el("p");
            p.kids = kids;
            match st.align.as_deref() {
                Some("center") => p.set("class", "center"),
                Some("end" | "right") => p.set("class", "right"),
                _ => {}
            }
            if display.contains("subtitle") || display.contains("subtítulo") {
                p.set("class", "center");
            }
            if display.contains("quotation")
                || display.contains("citaç")
                || display.contains("cita")
            {
                let mut q = el("blockquote");
                q.push(Node::Elem(p));
                out.push(Node::Elem(q));
            } else {
                out.push(Node::Elem(p));
            }
        }
        out.extend(after);
    }

    /// A list, numbered when its style numbers that level.
    fn list(&mut self, e: &Elem, list_style: Option<&str>, depth: usize) -> Elem {
        let style = e.attr("text:style-name").or(list_style);
        let ordered = style
            .and_then(|s| self.lists.get(s))
            .and_then(|l| l.get(&(depth + 1)))
            .copied()
            .unwrap_or(false);
        let mut list = el(if ordered { "ol" } else { "ul" });
        for item in kids(e, "list-item").chain(kids(e, "list-header")) {
            let mut li = el("li");
            let inner = self.blocks(&item.kids, style, depth + 1);
            // A single paragraph: its content in the item.
            if inner.len() == 1
                && let Some(Node::Elem(p)) = inner.first()
                && p.name == "p"
            {
                li.kids = p.kids.clone();
            } else {
                li.kids = inner;
            }
            list.push(Node::Elem(li));
        }
        list
    }

    fn table(&mut self, e: &Elem, depth: usize) -> Elem {
        let mut t = el("table");
        let mut body = el("tbody");
        let mut rows = Vec::new();
        find_rows(&e.kids, &mut rows);
        for r in rows {
            let mut row = el("tr");
            for c in kids(r, "table-cell") {
                let mut cell = el("td");
                if let Some(s) = c.attr("table:number-columns-spanned") {
                    cell.set("colspan", s);
                }
                if let Some(s) = c.attr("table:number-rows-spanned") {
                    cell.set("rowspan", s);
                }
                let inner = self.blocks(&c.kids, None, depth);
                if inner.len() == 1
                    && let Some(Node::Elem(p)) = inner.first()
                    && p.name == "p"
                {
                    cell.kids = p.kids.clone();
                } else {
                    cell.kids = inner;
                }
                row.push(Node::Elem(cell));
            }
            body.push(Node::Elem(row));
        }
        t.push(Node::Elem(body));
        t
    }

    fn inline(&mut self, nodes: &[Node]) -> Vec<Node> {
        let mut out = Vec::new();
        for n in nodes {
            match n {
                Node::Text(t) => out.push(Node::Text(
                    t.split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .pipe_keep_edges(t),
                )),
                Node::Elem(e) => match e.name.as_str() {
                    "span" => out.extend(self.span(e)),
                    "a" => {
                        let mut a = el("a");
                        if let Some(h) = e.attr("xlink:href") {
                            a.set("href", h);
                        }
                        a.kids = self.inline(&e.kids);
                        out.push(Node::Elem(a));
                    }
                    "s" => {
                        let c = e
                            .attr("text:c")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(1usize)
                            .min(20);
                        out.push(Node::Text(" ".repeat(c)));
                    }
                    "tab" => out.push(Node::Text("\u{2003}".into())),
                    "line-break" => out.push(Node::Elem(el("br"))),
                    "soft-page-break" | "annotation" | "annotation-end" | "bookmark-end"
                    | "reference-mark-end" => {}
                    "bookmark" | "bookmark-start" | "reference-mark-start" => {
                        if let Some(n) = e.attr("text:name").filter(|n| xhtml::valid_id(n)) {
                            out.push(Node::Elem(el("span").with("id", n)));
                        }
                    }
                    "note" => out.push(self.note(e)),
                    "frame" => {
                        for i in kids(e, "image") {
                            if let Some(name) = i.attr("xlink:href").and_then(|h| self.pics.get(h))
                            {
                                out.push(Node::Elem(
                                    el("img").with("src", name.clone()).with("alt", ""),
                                ));
                                break;
                            }
                        }
                    }
                    "text-box" => {}
                    _ => out.extend(self.inline(&e.kids)),
                },
            }
        }
        merge_text(out)
    }

    /// A span: its text in the marks its style gives.
    fn span(&mut self, e: &Elem) -> Vec<Node> {
        let st = self.style(e.attr("text:style-name").unwrap_or(""));
        let content = self.inline(&e.kids);
        let mut marks: Vec<(&str, Option<&str>)> = Vec::new();
        if st.small_caps == Some(true) {
            marks.push(("span", Some("font-variant: small-caps")));
        }
        if let Some(p) = &st.position {
            if p.starts_with("super")
                || p.trim_start_matches('+')
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.trim_end_matches('%').parse::<f32>().ok())
                    .is_some_and(|v| v > 0.0)
            {
                marks.push(("sup", None));
            } else if p.starts_with("sub") || p.starts_with('-') {
                marks.push(("sub", None));
            }
        }
        if st.strike == Some(true) {
            marks.push(("s", None));
        }
        if st.underline == Some(true) {
            marks.push(("u", None));
        }
        if st.italic == Some(true) {
            marks.push(("em", None));
        }
        if st.bold == Some(true) {
            marks.push(("strong", None));
        }
        wrap_marks(content, &marks)
    }

    /// A note: the link in the text; the note (with a link back) is kept
    /// for the end of the book.
    fn note(&mut self, e: &Elem) -> Node {
        self.note_n += 1;
        let n = self.note_n;
        let nid = format!("note{n}");
        let mut a = el("a")
            .with("href", format!("#{nid}"))
            .with("id", format!("ref{n}"))
            .with("class", "noteref")
            .with("epub:type", "noteref");
        let cit = kid(e, "note-citation")
            .map(|c| c.plain())
            .filter(|c| !c.trim().is_empty())
            .unwrap_or_else(|| n.to_string());
        a.text(cit.clone());
        let mut body = kid(e, "note-body")
            .map(|b| self.blocks(&b.kids, None, 0))
            .unwrap_or_default();
        let mut back = el("a")
            .with("href", format!("#ref{n}"))
            .with("class", "back");
        back.text(format!("{cit}."));
        if let Some(Node::Elem(first)) = body.first_mut() {
            first.kids.insert(0, Node::Text(" ".into()));
            first.kids.insert(0, Node::Elem(back));
        }
        let mut aside = el("aside");
        aside.kids = body;
        self.notes.push((nid, aside));
        Node::Elem(a)
    }
}

trait KeepEdges {
    fn pipe_keep_edges(self, orig: &str) -> String;
}

impl KeepEdges for String {
    /// ODT collapses runs of white space; a leading or trailing space
    /// still parts words from their neighbours.
    fn pipe_keep_edges(self, orig: &str) -> String {
        if self.is_empty() {
            return if orig.is_empty() {
                String::new()
            } else {
                " ".into()
            };
        }
        let mut s = String::new();
        if orig.starts_with(char::is_whitespace) {
            s.push(' ');
        }
        s.push_str(&self);
        if orig.ends_with(char::is_whitespace) {
            s.push(' ');
        }
        s
    }
}

fn find_rows<'a>(nodes: &'a [Node], out: &mut Vec<&'a Elem>) {
    for n in nodes {
        if let Node::Elem(e) = n {
            match e.name.as_str() {
                "table-row" => out.push(e),
                "table-header-rows" | "table-rows" | "table-row-group" => find_rows(&e.kids, out),
                _ => {}
            }
        }
    }
}

/// An OpenDocument text.
pub fn odt(path: &Path) -> Result<Book, String> {
    let z = Zip::open(path)?;
    let content = z.xml("content.xml").ok_or("bookBroken")?;
    let mut book = Book::new();
    if let Some(meta) = z.xml("meta.xml") {
        dublin_core(&meta, &mut book);
    }
    let mut styles = HashMap::new();
    let mut lists = HashMap::new();
    if let Some(st) = z.xml("styles.xml") {
        ostyles(&st, &mut styles, &mut lists);
    }
    ostyles(&content, &mut styles, &mut lists);
    let mut hrefs = Vec::new();
    let mut imgs = Vec::new();
    xhtml::find_all(&content, "image", &mut imgs);
    for i in imgs {
        if let Some(h) = i.attr("xlink:href")
            && !h.starts_with("http")
        {
            hrefs.push(h.to_string());
        }
    }
    let mut pics = HashMap::new();
    for h in hrefs {
        if pics.contains_key(&h) {
            continue;
        }
        if let Some(data) = z.bytes(h.trim_start_matches("./"))
            && let Some(name) = book.add_file(data, &h.to_ascii_lowercase())
        {
            pics.insert(h, name);
        }
    }
    let text = xhtml::find(&content, "text").ok_or("bookBroken")?;
    let mut o = Odt {
        styles,
        lists,
        pics: &pics,
        notes: Vec::new(),
        note_n: 0,
    };
    let mut nodes = o.blocks(&text.kids, None, 0);
    spacing(&mut nodes);
    append_notes(&mut nodes, std::mem::take(&mut o.notes));
    finish(book, nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn zip_of(path: &Path, files: &[(&str, &str)]) {
        let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (n, c) in files {
            z.start_file(*n, SimpleFileOptions::default()).unwrap();
            z.write_all(c.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }

    const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;

    #[test]
    fn reads_docx() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.docx");
        let doc = format!(
            r#"<?xml version="1.0"?><w:document {W}><w:body>
<w:p><w:pPr><w:pStyle w:val="Ttulo1"/></w:pPr><w:r><w:t>Capítulo Um</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">Texto </w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>itálico</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>junto</w:t></w:r><w:r><w:footnoteReference w:id="2"/></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>item</w:t></w:r></w:p>
<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>* * *</w:t></w:r></w:p>
<w:p><w:pPr><w:pStyle w:val="Ttulo1"/></w:pPr><w:r><w:t>Capítulo Dois</w:t></w:r></w:p>
<w:tbl><w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>b</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
</w:body></w:document>"#
        );
        let styles = format!(
            r#"<w:styles {W}><w:style w:type="paragraph" w:styleId="Ttulo1"><w:name w:val="heading 1"/></w:style></w:styles>"#
        );
        let numbering = format!(
            r#"<w:numbering {W}><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#
        );
        let notes = format!(
            r#"<w:footnotes {W}><w:footnote w:id="2"><w:p><w:r><w:t>Uma nota.</w:t></w:r></w:p></w:footnote></w:footnotes>"#
        );
        let core = r#"<cp:coreProperties xmlns:cp="x" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Meu Livro</dc:title><dc:creator>Lia Marta</dc:creator></cp:coreProperties>"#;
        zip_of(
            &p,
            &[
                ("word/document.xml", &doc),
                ("word/styles.xml", &styles),
                ("word/numbering.xml", &numbering),
                ("word/footnotes.xml", &notes),
                ("docProps/core.xml", core),
            ],
        );
        assert_eq!(
            super::super::source_of(&p),
            Some(super::super::Source::Docx)
        );
        let b = docx(&p).unwrap();
        assert_eq!(b.meta.title, "Meu Livro");
        assert_eq!(b.meta.authors, vec!["Lia Marta"]);
        assert_eq!(b.chapters.len(), 2, "{:#?}", b.chapters);
        let c = &b.chapters[0].body;
        assert!(c.contains("<h1>Capítulo Um</h1>"), "{c}");
        assert!(c.contains("Texto <em>itálicojunto</em>"), "{c}");
        assert!(c.contains("<ol><li>item</li></ol>"), "{c}");
        assert!(c.contains("<p class=\"center\">* * *</p>"), "{c}");
        assert!(c.contains("href=\"c0002.xhtml#note1\""), "{c}");
        let c2 = &b.chapters[1].body;
        assert!(c2.contains("<td colspan=\"2\">b</td>"), "{c2}");
        assert!(c2.contains("Uma nota."), "{c2}");
        assert!(c2.contains("href=\"c0001.xhtml#ref1\""), "{c2}");
    }

    #[test]
    fn reads_odt() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.odt");
        let content = r#"<?xml version="1.0"?><office:document-content xmlns:office="o" xmlns:text="t" xmlns:style="s" xmlns:fo="f">
<office:automatic-styles><style:style style:name="T1" style:family="text"><style:text-properties fo:font-style="italic"/></style:style>
<style:style style:name="P1" style:family="paragraph"><style:paragraph-properties fo:text-align="center"/></style:style></office:automatic-styles>
<office:body><office:text><text:h text:outline-level="1">Parte Um</text:h><text:p>Um <text:span text:style-name="T1">belo</text:span> dia<text:note text:note-class="footnote"><text:note-citation>1</text:note-citation><text:note-body><text:p>Nota.</text:p></text:note-body></text:note>.</text:p>
<text:p text:style-name="P1">fim</text:p><text:list><text:list-item><text:p>x</text:p></text:list-item></text:list>
<text:h text:outline-level="1">Parte Dois</text:h><text:p>mais</text:p></office:text></office:body></office:document-content>"#;
        let meta = r#"<office:document-meta xmlns:office="o" xmlns:dc="http://purl.org/dc/elements/1.1/"><office:meta><dc:title>Texto</dc:title></office:meta></office:document-meta>"#;
        zip_of(
            &p,
            &[
                ("mimetype", "application/vnd.oasis.opendocument.text"),
                ("content.xml", content),
                ("meta.xml", meta),
            ],
        );
        let b = odt(&p).unwrap();
        assert_eq!(b.meta.title, "Texto");
        assert_eq!(b.chapters.len(), 2);
        let c = &b.chapters[0].body;
        assert!(c.contains("Um <em>belo</em> dia"), "{c}");
        assert!(c.contains("<p class=\"center\">fim</p>"), "{c}");
        assert!(c.contains("<ul><li>x</li></ul>"), "{c}");
        assert!(b.chapters[1].body.contains("Nota."));
    }

    const PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
        0xcf, 0xc0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0xc9, 0xfe, 0x92, 0xef, 0x00, 0x00, 0x00,
        0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    fn zip_with(path: &Path, files: &[(&str, &[u8])]) {
        let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (n, c) in files {
            z.start_file(*n, SimpleFileOptions::default()).unwrap();
            z.write_all(c).unwrap();
        }
        z.finish().unwrap();
    }

    /// Pins what a rich Word and a rich OpenDocument text are read as, so
    /// the readers can be taken apart.
    #[test]
    fn office_does_not_change() {
        let d = tempfile::tempdir().unwrap();
        let doc = format!(
            r#"<?xml version="1.0"?><w:document {W} xmlns:mc="m"><w:body>
<w:p><w:pPr><w:pStyle w:val="H1"/></w:pPr><w:r><w:t>Primeiro</w:t></w:r></w:p>
<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>negrito</w:t></w:r><w:r><w:rPr><w:i/><w:u w:val="single"/></w:rPr><w:t> it-sub </w:t></w:r><w:r><w:rPr><w:strike/><w:smallCaps/></w:rPr><w:t>riscado</w:t></w:r><w:r><w:rPr><w:vertAlign w:val="superscript"/></w:rPr><w:t>sup</w:t></w:r><w:r><w:rPr><w:vertAlign w:val="subscript"/></w:rPr><w:t>sub</w:t></w:r><w:r><w:rPr><w:vanish/></w:rPr><w:t>oculto</w:t></w:r><w:r><w:rPr><w:rStyle w:val="Forte"/></w:rPr><w:t>estilo</w:t></w:r><w:r><w:tab/><w:t>tab</w:t><w:br/><w:t>quebra</w:t><w:br w:type="page"/><w:noBreakHyphen/><w:softHyphen/><w:sym w:char="F041"/></w:r></w:p>
<w:p><w:r><w:footnoteReference w:id="2"/></w:r><w:r><w:endnoteReference w:id="3"/></w:r><w:r><w:drawing><a:graphic xmlns:a="a"><a:blip r:embed="rId5"/></a:graphic></w:drawing></w:r><w:hyperlink r:id="rId6"><w:r><w:t>link</w:t></w:r></w:hyperlink><w:hyperlink w:anchor="alvo"><w:r><w:t>ancora</w:t></w:r></w:hyperlink><w:bookmarkStart w:id="1" w:name="alvo"/><w:bookmarkStart w:id="2" w:name="_GoBack"/><w:fldSimple w:instr="x"><w:r><w:t>campo</w:t></w:r></w:fldSimple><w:sdt><w:sdtContent><w:r><w:t>sdt</w:t></w:r></w:sdtContent></w:sdt><mc:AlternateContent><mc:Choice><w:r><w:t>alt</w:t></w:r></mc:Choice></mc:AlternateContent></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>um</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>sub um</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>dois</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="2"/></w:numPr></w:pPr><w:r><w:t>outra lista</w:t></w:r></w:p>
<w:p><w:pPr><w:jc w:val="right"/></w:pPr><w:r><w:t>direita</w:t></w:r></w:p>
<w:p><w:pPr><w:pStyle w:val="Citacao"/></w:pPr><w:r><w:t>citação</w:t></w:r></w:p>
<w:p><w:pPr><w:pStyle w:val="Sub"/></w:pPr><w:r><w:t>subtítulo</w:t></w:r></w:p>
<w:p><w:pPr><w:outlineLvl w:val="1"/><w:pageBreakBefore/></w:pPr><w:r><w:t>por contorno</w:t></w:r><w:r><w:pict><w:txbxContent><w:p><w:r><w:t>caixa</w:t></w:r></w:p></w:txbxContent></w:pict></w:r></w:p>
<w:tbl><w:tr><w:tc><w:tcPr><w:vMerge w:val="restart"/></w:tcPr><w:p><w:r><w:t>m</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr><w:p><w:r><w:t>a</w:t></w:r></w:p><w:p><w:r><w:t>b</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p/></w:tc><w:tc><w:p><w:r><w:t>c</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:p><w:pPr><w:pStyle w:val="H1"/></w:pPr><w:r><w:t>Segundo</w:t></w:r></w:p>
<w:sdt><w:sdtContent><w:p><w:r><w:t>em sdt</w:t></w:r></w:p></w:sdtContent></w:sdt>
<w:customXml><w:p><w:r><w:t>em xml</w:t></w:r></w:p></w:customXml>
<w:ins><w:p><w:r><w:t>inserido</w:t></w:r></w:p></w:ins>
<mc:AlternateContent><mc:Fallback><w:p><w:r><w:t>reserva</w:t></w:r></w:p></mc:Fallback></mc:AlternateContent>
<w:p><w:pPr><w:pStyle w:val="Normal"/></w:pPr><w:r><w:t>fim</w:t></w:r></w:p>
<w:sectPr/></w:body></w:document>"#
        );
        let styles = format!(
            r#"<w:styles {W}><w:docDefaults><w:pPrDefault><w:pPr><w:spacing w:after="160"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="H1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr></w:style><w:style w:type="paragraph" w:styleId="Citacao"><w:name w:val="Quote"/></w:style><w:style w:type="paragraph" w:styleId="Sub"><w:name w:val="Subtitle"/><w:pPr><w:jc w:val="center"/></w:pPr></w:style><w:style w:type="character" w:styleId="Forte"><w:name w:val="Strong"/><w:rPr><w:b/><w:i w:val="0"/></w:rPr></w:style></w:styles>"#
        );
        let numbering = format!(
            r#"<w:numbering {W}><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl><w:lvl w:ilvl="1"><w:numFmt w:val="bullet"/></w:lvl></w:abstractNum><w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num></w:numbering>"#
        );
        let notes = format!(
            r#"<w:footnotes {W}><w:footnote w:id="2"><w:p><w:r><w:t>Nota de rodapé.</w:t></w:r></w:p></w:footnote></w:footnotes>"#
        );
        let ends = format!(
            r#"<w:endnotes {W}><w:endnote w:id="3"><w:p><w:r><w:t>Nota final.</w:t></w:r></w:p></w:endnote></w:endnotes>"#
        );
        let rels = r#"<Relationships xmlns="r"><Relationship Id="rId5" Type="image" Target="media/image1.png"/><Relationship Id="rId6" Type="hyperlink" Target="https://example.org/" TargetMode="External"/></Relationships>"#;
        let core = r#"<cp:coreProperties xmlns:cp="x" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Rico</dc:title><dc:creator>Ana Mar</dc:creator><dc:language>pt-BR</dc:language></cp:coreProperties>"#;
        let p = d.path().join("rich.docx");
        zip_with(
            &p,
            &[
                ("word/document.xml", doc.as_bytes()),
                ("word/styles.xml", styles.as_bytes()),
                ("word/numbering.xml", numbering.as_bytes()),
                ("word/footnotes.xml", notes.as_bytes()),
                ("word/endnotes.xml", ends.as_bytes()),
                ("word/_rels/document.xml.rels", rels.as_bytes()),
                ("word/media/image1.png", PNG),
                ("docProps/core.xml", core.as_bytes()),
            ],
        );
        let mut all = vec![format!("{:?}", docx(&p))];

        let content = r##"<?xml version="1.0"?><office:document-content xmlns:office="o" xmlns:text="t" xmlns:style="s" xmlns:fo="f" xmlns:xlink="x" xmlns:draw="d" xmlns:table="tb">
<office:automatic-styles><style:style style:name="T1" style:family="text"><style:text-properties fo:font-weight="bold" fo:font-style="italic" style:text-underline-style="solid" style:text-line-through-style="solid" fo:font-variant="small-caps"/></style:style>
<style:style style:name="T2" style:family="text"><style:text-properties style:text-position="super 58%"/></style:style>
<style:style style:name="T3" style:family="text"><style:text-properties style:text-position="sub 58%"/></style:style>
<style:style style:name="P1" style:family="paragraph"><style:paragraph-properties fo:text-align="center"/></style:style>
<style:style style:name="P2" style:family="paragraph"><style:paragraph-properties fo:text-align="end" fo:break-before="page"/></style:style>
<style:style style:name="Q" style:family="paragraph" style:display-name="Quotation"/>
<style:style style:name="Tt" style:family="paragraph" style:display-name="Title"/>
<style:style style:name="St" style:family="paragraph" style:display-name="Subtitle"/>
<style:style style:name="Hh" style:family="paragraph" style:display-name="Heading 2"/>
<style:style style:name="Ol" style:family="paragraph" style:default-outline-level="3"/>
<style:style style:name="Child" style:family="paragraph" style:parent-style-name="P1"/>
<text:list-style style:name="L1"><text:list-level-style-number text:level="1"/><text:list-level-style-bullet text:level="2"/></text:list-style></office:automatic-styles>
<office:body><office:text><text:sequence-decls/><text:h text:outline-level="1">Parte Um</text:h>
<text:p>Texto <text:span text:style-name="T1">rico</text:span> com <text:span text:style-name="T2">sup</text:span><text:span text:style-name="T3">sub</text:span><text:a xlink:href="https://example.org/">link</text:a><text:s text:c="3"/><text:tab/>após<text:line-break/>quebra<text:bookmark text:name="alvo"/><text:bookmark-start text:name="b2"/><text:bookmark-end/><text:soft-page-break/><text:note text:note-class="footnote"><text:note-citation>a</text:note-citation><text:note-body><text:p>Nota um.</text:p></text:note-body></text:note><draw:frame><draw:image xlink:href="Pictures/a.png"/></draw:frame></text:p>
<text:p text:style-name="P1">centro</text:p><text:p text:style-name="P2">direita com quebra</text:p><text:p text:style-name="Q">citação</text:p><text:p text:style-name="Tt">Título</text:p><text:p text:style-name="St">Subtítulo</text:p><text:p text:style-name="Hh">Cabeçalho por nome</text:p><text:p text:style-name="Ol">Contorno</text:p><text:p text:style-name="Child">herdado</text:p>
<text:p>com caixa<draw:frame><draw:text-box><text:p>dentro da caixa</text:p></draw:text-box></draw:frame></text:p>
<text:list text:style-name="L1"><text:list-item><text:p>a</text:p><text:list><text:list-item><text:p>a1</text:p></text:list-item></text:list></text:list-item><text:list-item><text:p>b</text:p><text:p>b2</text:p></text:list-item></text:list>
<table:table><table:table-header-rows><table:table-row><table:table-cell table:number-columns-spanned="2"><text:p>h</text:p></table:table-cell></table:table-row></table:table-header-rows><table:table-row><table:table-cell table:number-rows-spanned="2"><text:p>x</text:p></table:table-cell><table:table-cell><text:p>y</text:p><text:p>z</text:p></table:table-cell></table:table-row></table:table>
<text:section><text:p>em seção</text:p></text:section><text:table-of-content><text:p>índice</text:p></text:table-of-content>
<text:h text:outline-level="1" text:style-name="P2">Parte Dois</text:h><text:p>fim</text:p></office:text></office:body></office:document-content>"##;
        let meta = r#"<office:document-meta xmlns:office="o" xmlns:dc="http://purl.org/dc/elements/1.1/"><office:meta><dc:title>Texto Rico</dc:title><dc:creator>Ana Mar</dc:creator></office:meta></office:document-meta>"#;
        let o = d.path().join("rich.odt");
        zip_with(
            &o,
            &[
                ("mimetype", b"application/vnd.oasis.opendocument.text"),
                ("content.xml", content.as_bytes()),
                ("meta.xml", meta.as_bytes()),
                ("Pictures/a.png", PNG),
            ],
        );
        all.push(format!("{:?}", odt(&o)));
        let sum = xxhash_rust::xxh3::xxh3_64(all.join("\n").as_bytes());
        println!("hash {sum:016x}\n{}", all.join("\n"));
        assert_eq!(
            sum, 0xeb17344dd01cc55f,
            "what the office readers give changed"
        );
    }
}
