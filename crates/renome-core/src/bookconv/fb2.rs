//! FictionBook 2 (FB2): read into a [`Book`] and written from one.
//!
//! An FB2 is one XML file: the book's details, a main body of nested
//! sections (each with its title, epigraphs, poems, quotes), a body of
//! notes, and the pictures in base64. Reading keeps that structure: each
//! top section is a chapter, deeper ones headings; poems keep their
//! stanzas and lines; notes become a notes chapter whose entries link
//! back to where they were called.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;

use super::text_in::decode_bytes;
use super::xhtml::{self, Elem, Node, esc, esc_attr};
use super::{Book, Chapter, Toc};

/// Reads an FB2 file's bytes.
pub fn read(bytes: &[u8]) -> Result<Book, String> {
    let text = decode_bytes(bytes, true);
    let nodes = xhtml::parse_xml(&text);
    let root = xhtml::find(&nodes, "fictionbook").ok_or("bookBroken")?;
    let mut book = Book::new();
    book.css.push_str(FB2_CSS);

    // Pictures first: the text points at them by id.
    let mut pics: HashMap<String, String> = HashMap::new();
    let mut bins = Vec::new();
    xhtml::find_all(&root.kids, "binary", &mut bins);
    for b in bins {
        let Some(id) = b.attr("id") else { continue };
        let data = base64(&b.plain());
        if let Some(name) = book.add_file(data, &id.to_ascii_lowercase()) {
            pics.insert(id.to_string(), name);
        }
    }

    if let Some(desc) = xhtml::find(&root.kids, "description") {
        details(desc, &mut book, &pics);
    }

    let mut bodies = Vec::new();
    for n in &root.kids {
        if let Node::Elem(e) = n
            && e.name == "body"
        {
            bodies.push(e);
        }
    }
    let mut r = Reader {
        pics: &pics,
        note_ids: Default::default(),
    };
    // Notes bodies (`name="notes"`, `"comments"`) after the main one.
    let mut main: Vec<&Elem> = Vec::new();
    let mut notes: Vec<&Elem> = Vec::new();
    for (i, b) in bodies.into_iter().enumerate() {
        if i > 0 && b.attr("name").is_some() {
            notes.push(b);
        } else {
            main.push(b);
        }
    }
    if !notes.is_empty() {
        for nb in &notes {
            let mut secs = Vec::new();
            xhtml::find_all(&nb.kids, "section", &mut secs);
            for s in secs {
                if let Some(id) = s.attr("id") {
                    r.note_ids.insert(id.to_string());
                }
            }
        }
    }
    for body in &main {
        r.body(body, &mut book);
    }
    if !notes.is_empty() {
        let mut s = String::from("<section class=\"notes\" epub:type=\"footnotes\">");
        let mut title = String::new();
        for nb in &notes {
            for k in &nb.kids {
                let Node::Elem(e) = k else { continue };
                match e.name.as_str() {
                    "title" => {
                        title = clean_line(&e.plain());
                        s.push_str(&format!("<h2>{}</h2>", esc(&title)));
                    }
                    "section" => r.note(e, &mut s),
                    _ => {}
                }
            }
        }
        s.push_str("</section>");
        let title = if title.is_empty() {
            "Notes".to_string()
        } else {
            title
        };
        book.chapters.push(Chapter {
            name: "notes.xhtml".into(),
            title,
            body: s,
            ..Default::default()
        });
    }
    if book.chapters.is_empty() {
        return Err("bookBroken".into());
    }
    super::fix_links(&mut book);
    Ok(book)
}

/// An FB2 inside a zip (`.fbz`, `.fb2.zip`).
pub fn read_zip(path: &Path) -> Result<Book, String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|_| "bookBroken")?;
    let name = z
        .file_names()
        .filter_map(Result::ok)
        .find(|n| n.to_ascii_lowercase().ends_with(".fb2"))
        .map(String::from)
        .ok_or("bookBroken")?;
    let mut bytes = Vec::new();
    z.by_name(&name)
        .map_err(|_| "bookBroken")?
        .take(512 << 20)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    read(&bytes)
}

const FB2_CSS: &str = "
.subtitle { text-align: center; text-indent: 0; font-weight: bold; margin: 1em 0; }
.empty { margin: 0; text-indent: 0; }
.annotation { margin: 1em 0; font-size: 0.95em; }
.title-page { text-align: center; margin-top: 20%; }
.title-page p { text-indent: 0; }
.noteref { vertical-align: super; font-size: 0.75em; line-height: 0; text-decoration: none; }
.notes h3 { font-size: 1em; text-align: left; margin: 1em 0 0.3em; }
.back { text-decoration: none; }
";

fn base64(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => continue,
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

pub fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len() * 4 / 3 + data.len() / 57 + 4);
    for (i, ch) in data.chunks(3).enumerate() {
        if i > 0 && i % 19 == 0 {
            s.push('\n');
        }
        let b = [ch[0], *ch.get(1).unwrap_or(&0), *ch.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if ch.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if ch.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}

fn clean_line(s: &str) -> String {
    crate::text::collapse_spaces(s.trim())
}

fn person(e: &Elem) -> String {
    let part = |n: &str| {
        e.kids
            .iter()
            .find_map(|k| match k {
                Node::Elem(x) if x.name == n => Some(clean_line(&x.plain())),
                _ => None,
            })
            .unwrap_or_default()
    };
    let full = [part("first-name"), part("middle-name"), part("last-name")]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if full.is_empty() {
        part("nickname")
    } else {
        full
    }
}

fn href_of(e: &Elem) -> Option<&str> {
    e.attrs
        .iter()
        .find(|(k, _)| k == "href" || k.ends_with(":href"))
        .map(|(_, v)| v.as_str())
}

fn details(desc: &Elem, book: &mut Book, pics: &HashMap<String, String>) {
    let child = |e: &Elem, n: &str| -> Option<Elem> {
        e.kids.iter().find_map(|k| match k {
            Node::Elem(x) if x.name == n => Some(x.clone()),
            _ => None,
        })
    };
    let m = &mut book.meta;
    if let Some(ti) = child(desc, "title-info") {
        let mut codes = Vec::new();
        for k in &ti.kids {
            let Node::Elem(e) = k else { continue };
            match e.name.as_str() {
                "author" => {
                    let p = person(e);
                    if !p.is_empty() {
                        m.authors.push(p);
                    }
                }
                "translator" => {
                    let p = person(e);
                    if !p.is_empty() {
                        m.contributors.push((p, "trl".into()));
                    }
                }
                "book-title" => m.title = clean_line(&e.plain()),
                "lang" => m.language = clean_line(&e.plain()),
                "date" => {
                    let v = e
                        .attr("value")
                        .map(String::from)
                        .unwrap_or_else(|| clean_line(&e.plain()));
                    if m.date.is_empty() {
                        m.date = v;
                    }
                }
                "genre" => {
                    let g = clean_line(&e.plain());
                    if g.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
                        // A code of the format's list: only when the book
                        // gives no words of its own.
                        if !g.is_empty() {
                            codes.push(g);
                        }
                    } else if !g.is_empty() {
                        m.subjects.push(g);
                    }
                }
                "keywords" => {
                    for w in e.plain().split(',') {
                        let w = clean_line(w);
                        if !w.is_empty() && !m.subjects.contains(&w) {
                            m.subjects.push(w);
                        }
                    }
                }
                "annotation" => m.description = clean_line(&e.plain()),
                "sequence" => {
                    if m.series.is_empty() {
                        m.series = e.attr("name").map(clean_line).unwrap_or_default();
                        m.series_index = e.attr("number").map(clean_line).unwrap_or_default();
                    }
                }
                "coverpage" => {
                    let mut imgs = Vec::new();
                    xhtml::find_all(&e.kids, "image", &mut imgs);
                    if let Some(h) = imgs.first().and_then(|i| href_of(i)) {
                        book.cover = pics.get(h.trim_start_matches('#')).cloned();
                    }
                }
                _ => {}
            }
        }
        if m.subjects.is_empty() {
            m.subjects = codes.into_iter().filter(|c| c != "unrecognised").collect();
        }
    }
    if let Some(pi) = child(desc, "publish-info") {
        for k in &pi.kids {
            let Node::Elem(e) = k else { continue };
            let v = clean_line(&e.plain());
            match e.name.as_str() {
                "publisher" if !v.is_empty() => m.publisher = v,
                "year" if !v.is_empty() => {
                    if m.date.len() < 4 || !m.date.starts_with(&v) {
                        m.date = v
                    }
                }
                "isbn" => {
                    if let Some(i) = crate::ebook::isbn13(&v) {
                        m.isbn = i;
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(di) = child(desc, "document-info")
        && let Some(id) = child(&di, "id")
    {
        let v = clean_line(&id.plain());
        if !v.is_empty() {
            m.id = format!("urn:fb2:{v}");
        }
    }
}

struct Reader<'a> {
    pics: &'a HashMap<String, String>,
    note_ids: std::collections::HashSet<String>,
}

impl Reader<'_> {
    /// The main body: its title (a title page), then each top section a
    /// chapter.
    fn body(&mut self, body: &Elem, book: &mut Book) {
        let mut lead = String::new();
        let mut sections = Vec::new();
        for k in &body.kids {
            let Node::Elem(e) = k else { continue };
            match e.name.as_str() {
                "section" => sections.push(e),
                "title" => {
                    lead.push_str("<div class=\"title-page\">");
                    for p in lines(e) {
                        lead.push_str(&format!("<h1>{}</h1>", self.inline(&p)));
                    }
                    lead.push_str("</div>");
                }
                "image" => lead.push_str(&self.block_image(e)),
                "epigraph" => lead.push_str(&self.block(e, 1)),
                _ => lead.push_str(&self.block(e, 1)),
            }
        }
        if !lead.is_empty() {
            book.chapters.push(Chapter {
                name: book.next_name(),
                title: String::new(),
                body: lead,
                ..Default::default()
            });
        }
        // A book whose only section holds the chapters: those are the
        // chapters.
        let unwrap = sections.len() == 1
            && !sections[0]
                .kids
                .iter()
                .any(|k| matches!(k, Node::Elem(e) if e.name == "p"))
            && sections[0]
                .kids
                .iter()
                .filter(|k| matches!(k, Node::Elem(e) if e.name == "section"))
                .count()
                > 1;
        if unwrap {
            let only = sections[0];
            let mut head = String::new();
            let mut subs = Vec::new();
            for k in &only.kids {
                if let Node::Elem(e) = k {
                    if e.name == "section" {
                        subs.push(e);
                    } else {
                        head.push_str(&self.block(e, 1));
                    }
                }
            }
            if !head.is_empty() {
                book.chapters.push(Chapter {
                    name: book.next_name(),
                    title: only
                        .kids
                        .iter()
                        .find_map(|k| match k {
                            Node::Elem(e) if e.name == "title" => Some(clean_line(&e.plain())),
                            _ => None,
                        })
                        .unwrap_or_default(),
                    body: head,
                    ..Default::default()
                });
            }
            sections = subs;
        }
        for s in sections {
            let title = s
                .kids
                .iter()
                .find_map(|k| match k {
                    Node::Elem(e) if e.name == "title" => Some(
                        lines(e)
                            .iter()
                            .map(|l| clean_line(&l.plain()))
                            .collect::<Vec<_>>()
                            .join(" "),
                    ),
                    _ => None,
                })
                .unwrap_or_default();
            let mut html = String::new();
            self.section(s, 1, &mut html);
            let name = book.next_name();
            let mut toc = Toc {
                title: title.clone(),
                href: name.clone(),
                kids: vec![],
            };
            // Titled subsections go in the table of contents.
            for k in &s.kids {
                if let Node::Elem(e) = k
                    && e.name == "section"
                    && let Some(id) = e.attr("id")
                    && let Some(t) = e.kids.iter().find_map(|k| match k {
                        Node::Elem(x) if x.name == "title" => Some(clean_line(&x.plain())),
                        _ => None,
                    })
                    && !t.is_empty()
                {
                    toc.kids.push(Toc {
                        title: t,
                        href: format!("{name}#{id}"),
                        kids: vec![],
                    });
                }
            }
            if !title.is_empty() {
                book.toc.push(toc);
            }
            book.chapters.push(Chapter {
                name,
                title,
                body: html,
                ..Default::default()
            });
        }
    }

    fn section(&self, s: &Elem, depth: usize, out: &mut String) {
        let id = s.attr("id").filter(|v| xhtml::valid_id(v));
        if let Some(id) = id {
            out.push_str(&format!("<div id=\"{}\">", esc_attr(id)));
        }
        for k in &s.kids {
            let Node::Elem(e) = k else { continue };
            match e.name.as_str() {
                "title" => {
                    let h = depth.clamp(1, 6);
                    let ls: Vec<String> = lines(e).iter().map(|l| self.inline(l)).collect();
                    out.push_str(&format!("<h{h}>{}</h{h}>", ls.join("<br/>")));
                }
                "section" => self.section(e, depth + 1, out),
                _ => out.push_str(&self.block(e, depth)),
            }
        }
        if id.is_some() {
            out.push_str("</div>");
        }
    }

    fn block_image(&self, e: &Elem) -> String {
        let Some(name) = href_of(e).and_then(|h| self.pics.get(h.trim_start_matches('#'))) else {
            return String::new();
        };
        let alt = e.attr("alt").unwrap_or_default();
        let id = e
            .attr("id")
            .filter(|v| xhtml::valid_id(v))
            .map(|v| format!(" id=\"{}\"", esc_attr(v)))
            .unwrap_or_default();
        format!(
            "<div class=\"center\"{id}><img src=\"{}\" alt=\"{}\"/></div>",
            esc_attr(name),
            esc_attr(alt)
        )
    }

    /// Paragraph-level FB2 → XHTML.
    fn block(&self, e: &Elem, depth: usize) -> String {
        let id = e
            .attr("id")
            .filter(|v| xhtml::valid_id(v))
            .map(|v| format!(" id=\"{}\"", esc_attr(v)))
            .unwrap_or_default();
        let kids = |cls: &str| -> String {
            let mut s = format!("<div class=\"{cls}\"{id}>");
            for k in &e.kids {
                if let Node::Elem(x) = k {
                    s.push_str(&self.block(x, depth));
                }
            }
            s.push_str("</div>");
            s
        };
        match e.name.as_str() {
            "p" => format!("<p{id}>{}</p>", self.inline(e)),
            "subtitle" => format!("<p class=\"subtitle\"{id}>{}</p>", self.inline(e)),
            "empty-line" => "<p class=\"empty\">\u{a0}</p>".into(),
            "text-author" => format!("<p class=\"author\"{id}>{}</p>", self.inline(e)),
            "date" => format!("<p class=\"author\"{id}>{}</p>", self.inline(e)),
            "v" => format!("<p{id}>{}</p>", self.inline(e)),
            "epigraph" => kids("epigraph"),
            "annotation" => kids("annotation"),
            "poem" => kids("poem"),
            "stanza" => kids("stanza"),
            "title" => {
                // A poem's or stanza's title.
                let ls: Vec<String> = lines(e).iter().map(|l| self.inline(l)).collect();
                format!("<p class=\"subtitle\">{}</p>", ls.join("<br/>"))
            }
            "cite" => {
                let mut s = format!("<blockquote{id}>");
                for k in &e.kids {
                    if let Node::Elem(x) = k {
                        s.push_str(&self.block(x, depth));
                    }
                }
                s.push_str("</blockquote>");
                s
            }
            "image" => self.block_image(e),
            "table" => {
                let mut s = format!("<table{id}><tbody>");
                for k in &e.kids {
                    let Node::Elem(tr) = k else { continue };
                    if tr.name != "tr" {
                        continue;
                    }
                    s.push_str("<tr>");
                    for c in &tr.kids {
                        let Node::Elem(td) = c else { continue };
                        let tag = if td.name == "th" { "th" } else { "td" };
                        let mut a = String::new();
                        for k in ["colspan", "rowspan"] {
                            if let Some(v) = td.attr(k).filter(|v| v.parse::<u32>().is_ok()) {
                                a.push_str(&format!(" {k}=\"{v}\""));
                            }
                        }
                        if let Some(al) = td
                            .attr("align")
                            .filter(|a| matches!(*a, "left" | "right" | "center"))
                        {
                            a.push_str(&format!(" style=\"text-align: {al}\""));
                        }
                        s.push_str(&format!("<{tag}{a}>{}</{tag}>", self.inline(td)));
                    }
                    s.push_str("</tr>");
                }
                s.push_str("</tbody></table>");
                s
            }
            "section" => {
                let mut s = String::new();
                self.section(e, depth + 1, &mut s);
                s
            }
            _ => {
                // Unknown: its text as a paragraph.
                let t = self.inline(e);
                if t.trim().is_empty() {
                    String::new()
                } else {
                    format!("<p>{t}</p>")
                }
            }
        }
    }

    fn inline(&self, e: &Elem) -> String {
        let mut s = String::new();
        self.inline_into(&e.kids, &mut s);
        s
    }

    fn inline_into(&self, nodes: &[Node], s: &mut String) {
        for n in nodes {
            match n {
                Node::Text(t) => s.push_str(&esc(t)),
                Node::Elem(e) => {
                    let tag = match e.name.as_str() {
                        "strong" => "strong",
                        "emphasis" => "em",
                        "strikethrough" => "s",
                        "sub" => "sub",
                        "sup" => "sup",
                        "code" => "code",
                        "image" => {
                            if let Some(name) =
                                href_of(e).and_then(|h| self.pics.get(h.trim_start_matches('#')))
                            {
                                s.push_str(&format!("<img src=\"{}\" alt=\"\"/>", esc_attr(name)));
                            }
                            continue;
                        }
                        "a" => {
                            let href = href_of(e).unwrap_or_default();
                            let target = href.trim_start_matches('#');
                            let is_note = e.attr("type") == Some("note")
                                || (href.starts_with('#') && self.note_ids.contains(target));
                            if is_note && self.note_ids.contains(target) {
                                s.push_str(&format!(
                                    "<a class=\"noteref\" epub:type=\"noteref\" id=\"ref_{t}\" href=\"#{t}\">",
                                    t = esc_attr(target)
                                ));
                            } else if href.starts_with('#')
                                || href.starts_with("http")
                                || href.starts_with("mailto:")
                            {
                                s.push_str(&format!("<a href=\"{}\">", esc_attr(href)));
                            } else {
                                self.inline_into(&e.kids, s);
                                continue;
                            }
                            self.inline_into(&e.kids, s);
                            s.push_str("</a>");
                            continue;
                        }
                        _ => {
                            self.inline_into(&e.kids, s);
                            continue;
                        }
                    };
                    s.push_str(&format!("<{tag}>"));
                    self.inline_into(&e.kids, s);
                    s.push_str(&format!("</{tag}>"));
                }
            }
        }
    }

    /// One note: its number, its text, a link back.
    fn note(&self, e: &Elem, out: &mut String) {
        let Some(id) = e.attr("id").filter(|v| xhtml::valid_id(v)) else {
            for k in &e.kids {
                if let Node::Elem(x) = k {
                    out.push_str(&self.block(x, 3));
                }
            }
            return;
        };
        out.push_str(&format!(
            "<aside id=\"{}\" class=\"note\" epub:type=\"footnote\">",
            esc_attr(id)
        ));
        let mut first = true;
        for k in &e.kids {
            let Node::Elem(x) = k else { continue };
            if x.name == "title" {
                out.push_str(&format!(
                    "<h3><a class=\"back\" href=\"#ref_{}\">{}</a></h3>",
                    esc_attr(id),
                    esc(&clean_line(&x.plain()))
                ));
                first = false;
            } else {
                if first {
                    out.push_str(&format!(
                        "<p><a class=\"back\" href=\"#ref_{}\">↑</a></p>",
                        esc_attr(id)
                    ));
                    first = false;
                }
                out.push_str(&self.block(x, 3));
            }
        }
        out.push_str("</aside>");
    }
}

/// A title's lines: its `<p>`s (or itself).
fn lines(title: &Elem) -> Vec<Elem> {
    let ps: Vec<Elem> = title
        .kids
        .iter()
        .filter_map(|k| match k {
            Node::Elem(e) if e.name == "p" => Some(e.clone()),
            _ => None,
        })
        .collect();
    if ps.is_empty() {
        vec![title.clone()]
    } else {
        ps
    }
}

// ---------------------------------------------------------------- writing

/// Writes `book` as an FB2 file.
pub fn write(book: &Book, to: &Path) -> Result<(), String> {
    let s = to_fb2(book);
    let mut f = std::fs::File::create(to).map_err(|e| e.to_string())?;
    f.write_all(s.as_bytes()).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())
}

fn person_xml(tag: &str, name: &str) -> String {
    let parts: Vec<&str> = name.split_whitespace().collect();
    match parts.as_slice() {
        [] => String::new(),
        [one] => format!("<{tag}><nickname>{}</nickname></{tag}>", esc(one)),
        [first, mid @ .., last] => format!(
            "<{tag}><first-name>{}</first-name>{}<last-name>{}</last-name></{tag}>",
            esc(first),
            if mid.is_empty() {
                String::new()
            } else {
                format!("<middle-name>{}</middle-name>", esc(&mid.join(" ")))
            },
            esc(last)
        ),
    }
}

pub fn to_fb2(book: &Book) -> String {
    let m = &book.meta;
    let mut w = Writer {
        book,
        pics: Vec::new(),
        out: String::new(),
        depth: 0,
    };
    let mut ti = String::new();
    // Genres come from the format's own list; the book's words go in its
    // keywords.
    let mut genres: Vec<&str> = Vec::new();
    for g in m.subjects.iter().filter_map(|s| genre_of(s)) {
        if !genres.contains(&g) && genres.len() < 3 {
            genres.push(g);
        }
    }
    if genres.is_empty() {
        genres.push("unrecognised");
    }
    for g in genres {
        ti.push_str(&format!("<genre>{g}</genre>"));
    }
    for a in &m.authors {
        ti.push_str(&person_xml("author", a));
    }
    if m.authors.is_empty() {
        ti.push_str("<author><nickname>-</nickname></author>");
    }
    ti.push_str(&format!("<book-title>{}</book-title>", esc(&m.title)));
    if !m.description.is_empty() {
        ti.push_str(&format!(
            "<annotation><p>{}</p></annotation>",
            esc(&m.description)
        ));
    }
    let words: Vec<&str> = m
        .subjects
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if !words.is_empty() {
        ti.push_str(&format!("<keywords>{}</keywords>", esc(&words.join(", "))));
    }
    if !m.date.is_empty() {
        ti.push_str(&format!("<date>{}</date>", esc(&m.date)));
    }
    if let Some(c) = &book.cover {
        ti.push_str(&format!(
            "<coverpage><image l:href=\"#{}\"/></coverpage>",
            esc_attr(c)
        ));
        w.pics.push(c.clone());
    }
    let lang = m.language.split('-').next().unwrap_or("und");
    ti.push_str(&format!(
        "<lang>{}</lang>",
        esc(if lang.is_empty() { "und" } else { lang })
    ));
    for (n, role) in &m.contributors {
        if role == "trl" {
            ti.push_str(&person_xml("translator", n));
        }
    }
    if !m.series.is_empty() {
        let num = m
            .series_index
            .split('.')
            .next()
            .unwrap_or("")
            .parse::<u32>()
            .ok();
        ti.push_str(&format!(
            "<sequence name=\"{}\"{}/>",
            esc_attr(&m.series),
            num.map(|n| format!(" number=\"{n}\"")).unwrap_or_default()
        ));
    }
    let mut pi = String::new();
    if !m.publisher.is_empty() {
        pi.push_str(&format!("<publisher>{}</publisher>", esc(&m.publisher)));
    }
    if let Some(y) = m
        .date
        .get(..4)
        .filter(|y| y.chars().all(|c| c.is_ascii_digit()))
    {
        pi.push_str(&format!("<year>{y}</year>"));
    }
    if !m.isbn.is_empty() {
        pi.push_str(&format!("<isbn>{}</isbn>", esc(&m.isbn)));
    }
    let id = if m.id.is_empty() {
        format!("{:016x}", xxhash_rust::xxh3::xxh3_64(m.title.as_bytes()))
    } else {
        m.id.clone()
    };
    let di = format!(
        "<document-info><author><nickname>Renome</nickname></author><program-used>Renome</program-used><date value=\"{d}\">{d}</date><id>{}</id><version>1.0</version></document-info>",
        esc(&id),
        d = chrono::Utc::now().format("%Y-%m-%d")
    );

    // Notes (asides marked as footnotes) go to their own body.
    let mut notes = String::new();
    w.out.push_str("<body>");
    if !m.title.is_empty() {
        w.out
            .push_str(&format!("<title><p>{}</p></title>", esc(&m.title)));
    }
    for c in &book.chapters {
        let (body, _) = xhtml::body_of(xhtml::parse(&c.body));
        let (main, ns) = split_notes(body);
        w.chapter(&c.title, &main);
        for n in ns {
            notes.push_str(&w.note(&n));
        }
    }
    w.close_to(0);
    w.out.push_str("</body>");
    if !notes.is_empty() {
        w.out.push_str(&format!(
            "<body name=\"notes\"><title><p>Notes</p></title>{notes}</body>"
        ));
    }
    w.out = valid_sections(&w.out);
    let mut bins = String::new();
    let mut seen = std::collections::HashSet::new();
    for p in &w.pics {
        if !seen.insert(p.clone()) {
            continue;
        }
        if let Some(r) = book.files.iter().find(|r| &r.name == p) {
            bins.push_str(&format!(
                "<binary id=\"{}\" content-type=\"{}\">{}</binary>\n",
                esc_attr(&r.name),
                esc_attr(&r.media),
                base64_encode(&r.data)
            ));
        }
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<FictionBook xmlns=\"http://www.gribuser.ru/xml/fictionbook/2.0\" xmlns:l=\"http://www.w3.org/1999/xlink\">\n<description><title-info>{ti}</title-info>{di}{}</description>\n{}\n{bins}</FictionBook>\n",
        if pi.is_empty() {
            String::new()
        } else {
            format!("<publish-info>{pi}</publish-info>")
        },
        w.out
    )
}

/// The genre of the format's list a subject stands for (in English,
/// Portuguese or Spanish words), if any.
fn genre_of(subject: &str) -> Option<&'static str> {
    const GENRES: &[(&str, &[&str])] = &[
        (
            "nonfiction",
            &[
                "nao-ficcao",
                "nao ficcao",
                "non-fiction",
                "nonfiction",
                "no ficcion",
                "no-ficcion",
            ],
        ),
        (
            "prose_history",
            &[
                "ficcao historica",
                "romance historico",
                "historical fiction",
                "novela historica",
            ],
        ),
        ("sf_cyberpunk", &["cyberpunk"]),
        ("sf_space", &["space opera", "opera espacial"]),
        ("sf_horror", &["horror", "terror"]),
        (
            "sf",
            &[
                "ficcao cientifica",
                "science fiction",
                "ciencia ficcion",
                "ficcion cientifica",
                "sci-fi",
                "scifi",
            ],
        ),
        ("sf_fantasy", &["fantasia", "fantasy"]),
        (
            "detective",
            &[
                "detective",
                "detetive",
                "policial",
                "mystery",
                "misterio",
                "crime",
                "noir",
            ],
        ),
        ("thriller", &["thriller", "suspense"]),
        (
            "love_contemporary",
            &[
                "romantic",
                "romantico",
                "romantica",
                "love stor",
                "historia de amor",
            ],
        ),
        ("adventure", &["aventura", "adventure"]),
        ("humor", &["humor", "comedia", "comedy"]),
        ("poetry", &["poesia", "poetry", "poema", "poem"]),
        (
            "dramaturgy",
            &["teatro", "theatre", "theater", "dramaturgia"],
        ),
        (
            "children",
            &["infantil", "children", "juvenil", "kids", "young adult"],
        ),
        (
            "nonf_biography",
            &["biografia", "biography", "memoir", "memoria", "autobiograf"],
        ),
        (
            "religion_self",
            &[
                "autoajuda",
                "auto-ajuda",
                "autoayuda",
                "self-help",
                "self help",
                "desenvolvimento pessoal",
                "personal development",
            ],
        ),
        (
            "religion",
            &[
                "religiao",
                "religion",
                "teologia",
                "theology",
                "cristianismo",
                "christian",
                "espiritualidade",
                "spiritual",
                "biblia",
                "bible",
            ],
        ),
        ("sci_philosophy", &["filosofia", "philosophy"]),
        ("psy_generic", &["psicologia", "psychology"]),
        ("sci_politics", &["politica", "politics"]),
        ("sci_history", &["historia", "history"]),
        ("antique_myths", &["mitologia", "mythology"]),
        (
            "home_cooking",
            &["culinaria", "cooking", "receita", "cookbook", "cocina"],
        ),
        (
            "computers",
            &[
                "computacao",
                "informatica",
                "programacao",
                "programming",
                "computer",
            ],
        ),
        (
            "personal_finance",
            &[
                "financas pessoais",
                "personal finance",
                "finanzas personales",
            ],
        ),
        (
            "popular_business",
            &[
                "negocio",
                "business",
                "empreendedor",
                "administracao",
                "management",
                "empresa",
            ],
        ),
        ("economics", &["economia", "economics"]),
        ("ref_dict", &["dicionario", "dictionary", "diccionario"]),
        (
            "reference",
            &["referencia", "reference", "guia", "guide", "manual"],
        ),
        ("essays", &["ensaio", "essay", "ensayo"]),
        (
            "short_story",
            &["contos", "short stor", "cuentos", "relatos"],
        ),
        ("prose_classic", &["classico", "classic", "clasico"]),
        ("science", &["ciencia", "science"]),
    ];
    let s = crate::text::strip_accents(subject).to_lowercase();
    if s.chars().all(|c| c.is_ascii_lowercase() || c == '_') && GENRES.iter().any(|(g, _)| *g == s)
    {
        return GENRES.iter().find(|(g, _)| *g == s).map(|(g, _)| *g);
    }
    GENRES
        .iter()
        .find(|(_, words)| words.iter().any(|w| s.contains(w)))
        .map(|(g, _)| *g)
}

/// A piece of the written body: an element with its pieces, or a leaf
/// (text, or an element with nothing inside) as written.
enum Piece {
    Elem {
        name: String,
        open: String,
        kids: Vec<Piece>,
    },
    Leaf {
        name: Option<String>,
        raw: String,
    },
}

impl Piece {
    fn name(&self) -> Option<&str> {
        match self {
            Piece::Elem { name, .. } => Some(name),
            Piece::Leaf { name, .. } => name.as_deref(),
        }
    }

    fn blank(&self) -> bool {
        matches!(self, Piece::Leaf { name: None, raw } if raw.trim().is_empty())
    }

    fn write(&self, out: &mut String) {
        match self {
            Piece::Elem { name, open, kids } => {
                out.push_str(open);
                for k in kids {
                    k.write(out);
                }
                out.push_str("</");
                out.push_str(name);
                out.push('>');
            }
            Piece::Leaf { raw, .. } => out.push_str(raw),
        }
    }
}

/// The written body read back into pieces (`None` if it does not nest).
fn pieces(s: &str) -> Option<Vec<Piece>> {
    let b = s.as_bytes();
    let mut stack: Vec<(String, String, Vec<Piece>)> =
        vec![(String::new(), String::new(), Vec::new())];
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            let j = s[i..].find('<').map_or(b.len(), |k| i + k);
            stack.last_mut()?.2.push(Piece::Leaf {
                name: None,
                raw: s[i..j].to_string(),
            });
            i = j;
            continue;
        }
        let mut j = i + 1;
        let mut quote = 0u8;
        while j < b.len() {
            let c = b[j];
            if quote != 0 {
                if c == quote {
                    quote = 0;
                }
            } else if c == b'"' || c == b'\'' {
                quote = c;
            } else if c == b'>' {
                break;
            }
            j += 1;
        }
        if j >= b.len() {
            return None;
        }
        let tag = &s[i..=j];
        i = j + 1;
        let name: String = tag
            .trim_start_matches("</")
            .trim_start_matches('<')
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != '/' && *c != '>')
            .collect();
        if tag.starts_with("</") {
            let (open_name, open, kids) = stack.pop()?;
            if open_name != name || stack.is_empty() {
                return None;
            }
            stack.last_mut()?.2.push(Piece::Elem { name, open, kids });
        } else if tag.ends_with("/>") || tag.starts_with("<?") || tag.starts_with("<!") {
            stack.last_mut()?.2.push(Piece::Leaf {
                name: Some(name),
                raw: tag.to_string(),
            });
        } else {
            stack.push((name, tag.to_string(), Vec::new()));
        }
    }
    if stack.len() != 1 {
        return None;
    }
    stack.pop().map(|(_, _, k)| k)
}

/// Makes every section what the format allows: its heading (title,
/// epigraphs, picture, annotation), then either sections or text, never
/// both (text beside sections goes into a section of its own) and never
/// nothing (an empty line then).
fn valid_sections(body: &str) -> String {
    fn fix(p: &mut Piece) {
        if let Piece::Elem { name, kids, .. } = p {
            for k in kids.iter_mut() {
                fix(k);
            }
            if name == "section" {
                fix_section(kids);
            }
        }
    }
    fn fix_section(kids: &mut Vec<Piece>) {
        let mut i = 0;
        let mut stage = 0;
        while i < kids.len() {
            let next = match kids[i].name() {
                _ if kids[i].blank() => stage,
                Some("title") if stage < 1 => 1,
                Some("epigraph") if stage <= 2 => 2,
                Some("image") if stage < 3 => 3,
                Some("annotation") if stage < 4 => 4,
                _ => break,
            };
            stage = next;
            i += 1;
        }
        let rest = kids.split_off(i);
        let sections = rest.iter().any(|k| k.name() == Some("section"));
        let other = rest
            .iter()
            .any(|k| !k.blank() && k.name() != Some("section"));
        if !other && !sections {
            kids.extend(rest);
            kids.push(Piece::Leaf {
                name: Some("empty-line".into()),
                raw: "<empty-line/>".into(),
            });
            return;
        }
        if !(sections && other) {
            kids.extend(rest);
            return;
        }
        let mut run: Vec<Piece> = Vec::new();
        let flush = |run: &mut Vec<Piece>, kids: &mut Vec<Piece>| {
            if run.iter().any(|k| !k.blank()) {
                kids.push(Piece::Elem {
                    name: "section".into(),
                    open: "<section>".into(),
                    kids: std::mem::take(run),
                });
            } else {
                kids.append(run);
            }
        };
        for k in rest {
            if k.name() == Some("section") {
                flush(&mut run, kids);
                kids.push(k);
            } else {
                run.push(k);
            }
        }
        flush(&mut run, kids);
    }
    let Some(mut all) = pieces(body) else {
        return body.to_string();
    };
    let mut out = String::with_capacity(body.len() + 64);
    for p in &mut all {
        fix(p);
        p.write(&mut out);
    }
    out
}

/// Notes (`<aside>` footnotes, `epub:type` note elements) taken out of a
/// chapter's body.
fn split_notes(nodes: Vec<Node>) -> (Vec<Node>, Vec<Elem>) {
    let mut main = Vec::new();
    let mut notes = Vec::new();
    for n in nodes {
        match n {
            Node::Elem(e)
                if e.attr("epub:type").is_some_and(|t| {
                    t.contains("footnote") || t.contains("endnote") || t.contains("rearnote")
                }) && e.attr("id").is_some() =>
            {
                notes.push(e)
            }
            Node::Elem(mut e) => {
                let (m, ns) = split_notes(std::mem::take(&mut e.kids));
                e.kids = m;
                notes.extend(ns);
                main.push(Node::Elem(e));
            }
            t => main.push(t),
        }
    }
    (main, notes)
}

struct Writer<'a> {
    book: &'a Book,
    pics: Vec<String>,
    out: String,
    /// Sections open.
    depth: usize,
}

impl Writer<'_> {
    fn close_to(&mut self, d: usize) {
        while self.depth > d {
            self.out.push_str("</section>");
            self.depth -= 1;
        }
    }

    /// One chapter: a section; its headings start subsections.
    fn chapter(&mut self, title: &str, nodes: &[Node]) {
        self.close_to(0);
        self.out.push_str("<section>");
        self.depth = 1;
        // A chapter that does not start with a heading gets its title.
        let starts_with_heading = nodes
            .iter()
            .find(|n| !xhtml::is_blank(n))
            .is_some_and(|n| matches!(n, Node::Elem(e) if heading(&e.name).is_some()));
        if !starts_with_heading
            && !title.trim().is_empty()
            && title.trim() != self.book.meta.title.trim()
        {
            self.out
                .push_str(&format!("<title><p>{}</p></title>", esc(title.trim())));
        }
        let mut base: Option<usize> = None;
        let mut has_content = false;
        self.blocks(nodes, &mut base, &mut has_content);
        if !has_content {
            self.out.push_str("<empty-line/>");
        }
    }

    fn blocks(&mut self, nodes: &[Node], base: &mut Option<usize>, has_content: &mut bool) {
        let mut loose = String::new();
        for n in nodes {
            match n {
                Node::Text(t) => {
                    if !t.trim().is_empty() {
                        loose.push_str(&esc(t));
                    }
                }
                Node::Elem(e) => {
                    let inline = !xhtml::is_block(&e.name)
                        && !matches!(e.name.as_str(), "img" | "table" | "figure" | "br" | "li");
                    if inline {
                        loose.push_str(&self.inline_of(std::slice::from_ref(n)));
                        continue;
                    }
                    if !loose.trim().is_empty() {
                        self.out.push_str(&format!("<p>{}</p>", loose.trim()));
                        *has_content = true;
                    }
                    loose.clear();
                    self.block(e, base, has_content);
                }
            }
        }
        if !loose.trim().is_empty() {
            self.out.push_str(&format!("<p>{}</p>", loose.trim()));
            *has_content = true;
        }
    }

    fn block(&mut self, e: &Elem, base: &mut Option<usize>, has_content: &mut bool) {
        if let Some(level) = heading(&e.name) {
            let b = *base.get_or_insert(level);
            let want = 1 + level.saturating_sub(b);
            if *has_content || want > 1 {
                // A new section at the heading's depth.
                if want <= self.depth {
                    self.close_to(want - 1);
                }
                while self.depth < want {
                    self.out.push_str("<section>");
                    self.depth += 1;
                }
                *has_content = false;
            }
            let mut lines: Vec<Vec<Node>> = vec![Vec::new()];
            for k in &e.kids {
                if matches!(k, Node::Elem(x) if x.name == "br") {
                    lines.push(Vec::new());
                } else {
                    lines.last_mut().unwrap().push(k.clone());
                }
            }
            self.out.push_str("<title>");
            let mut any = false;
            for l in &lines {
                let t = self.inline_of(l);
                if !t.trim().is_empty() {
                    self.out.push_str(&format!("<p>{}</p>", t.trim()));
                    any = true;
                }
            }
            if !any {
                self.out.push_str("<p/>");
            }
            self.out.push_str("</title>");
            return;
        }
        *has_content = true;
        match e.name.as_str() {
            "p" | "dt" | "dd" | "li" | "address" | "figcaption" | "caption" => {
                // Lines parted by `<br>` are paragraphs of their own.
                let mut lines: Vec<Vec<Node>> = vec![Vec::new()];
                for k in &e.kids {
                    if matches!(k, Node::Elem(x) if x.name == "br") {
                        lines.push(Vec::new());
                    } else {
                        lines.last_mut().unwrap().push(k.clone());
                    }
                }
                let center = e.attr("class").is_some_and(|c| c.contains("center"));
                let mut any = false;
                for l in &lines {
                    let t = self.inline_of(l);
                    let t = t.trim();
                    if t.is_empty() || t == "\u{a0}" {
                        continue;
                    }
                    any = true;
                    if center && e.name == "p" && lines.len() == 1 && t.len() < 80 {
                        self.out.push_str(&format!("<subtitle>{t}</subtitle>"));
                    } else {
                        self.out.push_str(&format!("<p>{t}</p>"));
                    }
                }
                if !any
                    && !e
                        .kids
                        .iter()
                        .any(|k| matches!(k, Node::Elem(x) if x.name == "img"))
                {
                    self.out.push_str("<empty-line/>");
                }
                let mut imgs = Vec::new();
                xhtml::find_all(&e.kids, "img", &mut imgs);
                for x in imgs {
                    self.image(x);
                }
            }
            "img" => self.image(e),
            "hr" => self.out.push_str("<empty-line/>"),
            "br" => {}
            "pre" => {
                for line in e.plain().lines() {
                    self.out
                        .push_str(&format!("<p><code>{}</code></p>", esc(line)));
                }
            }
            "blockquote" => {
                self.out.push_str("<cite>");
                let mut inner = String::new();
                std::mem::swap(&mut inner, &mut self.out);
                let mut hc = false;
                let mut nb = None;
                self.flat(&e.kids, &mut nb, &mut hc);
                std::mem::swap(&mut inner, &mut self.out);
                self.out
                    .push_str(if inner.is_empty() { "<p/>" } else { &inner });
                self.out.push_str("</cite>");
            }
            "table" => {
                self.out.push_str("<table>");
                let mut rows = Vec::new();
                xhtml::find_all(&e.kids, "tr", &mut rows);
                for r in rows {
                    self.out.push_str("<tr>");
                    for c in &r.kids {
                        if let Node::Elem(td) = c
                            && (td.name == "td" || td.name == "th")
                        {
                            let mut a = String::new();
                            for k in ["colspan", "rowspan"] {
                                if let Some(v) = td.attr(k) {
                                    a.push_str(&format!(" {k}=\"{}\"", esc_attr(v)));
                                }
                            }
                            let t = self.inline_of(&td.kids);
                            self.out
                                .push_str(&format!("<{n}{a}>{}</{n}>", t.trim(), n = td.name));
                        }
                    }
                    self.out.push_str("</tr>");
                }
                self.out.push_str("</table>");
            }
            _ => {
                // A block holding others: what it holds.
                self.blocks(&e.kids, base, has_content);
            }
        }
    }

    /// Blocks inside a `<cite>`: no sections, headings as subtitles.
    fn flat(&mut self, nodes: &[Node], base: &mut Option<usize>, hc: &mut bool) {
        for n in nodes {
            match n {
                Node::Elem(e) if heading(&e.name).is_some() => {
                    let t = self.inline_of(&e.kids);
                    self.out
                        .push_str(&format!("<subtitle>{}</subtitle>", t.trim()));
                }
                Node::Elem(e) if e.name == "blockquote" => self.flat(&e.kids, base, hc),
                Node::Elem(e)
                    if xhtml::is_block(&e.name)
                        && !matches!(
                            e.name.as_str(),
                            "p" | "table" | "pre" | "li" | "dd" | "dt"
                        ) =>
                {
                    self.flat(&e.kids, base, hc)
                }
                Node::Elem(e) => self.block(e, base, hc),
                Node::Text(t) if !t.trim().is_empty() => {
                    self.out.push_str(&format!("<p>{}</p>", esc(t.trim())))
                }
                _ => {}
            }
        }
    }

    fn image(&mut self, e: &Elem) {
        if let Some(src) = e.attr("src")
            && self.book.files.iter().any(|r| {
                r.name == src && r.media.starts_with("image/") && r.media != "image/svg+xml"
            })
        {
            self.out
                .push_str(&format!("<image l:href=\"#{}\"/>", esc_attr(src)));
            self.pics.push(src.to_string());
        }
    }

    fn inline_of(&mut self, nodes: &[Node]) -> String {
        let mut s = String::new();
        self.inline_into(nodes, &mut s);
        s
    }

    fn inline_into(&mut self, nodes: &[Node], s: &mut String) {
        for n in nodes {
            match n {
                Node::Text(t) => s.push_str(&esc(&t.replace('\n', " "))),
                Node::Elem(e) => {
                    let tag = match e.name.as_str() {
                        "b" | "strong" => Some("strong"),
                        "i" | "em" | "cite" | "dfn" | "var" => Some("emphasis"),
                        "s" | "strike" | "del" => Some("strikethrough"),
                        "sub" => Some("sub"),
                        "sup" => Some("sup"),
                        "code" | "kbd" | "samp" | "tt" => Some("code"),
                        "br" => {
                            s.push(' ');
                            None
                        }
                        "img" => {
                            // Pictures inside text go after the paragraph.
                            None
                        }
                        "a" => {
                            let href = e.attr("href").unwrap_or_default();
                            let note = e.attr("epub:type").is_some_and(|t| t.contains("noteref"));
                            let local = href.split_once('#').map(|(_, f)| f);
                            if let Some(f) = local.filter(|_| note) {
                                s.push_str(&format!(
                                    "<a l:href=\"#{}\" type=\"note\">",
                                    esc_attr(f)
                                ));
                                self.inline_into(&e.kids, s);
                                s.push_str("</a>");
                            } else if href.starts_with("http") || href.starts_with("mailto:") {
                                s.push_str(&format!("<a l:href=\"{}\">", esc_attr(href)));
                                self.inline_into(&e.kids, s);
                                s.push_str("</a>");
                            } else {
                                self.inline_into(&e.kids, s);
                            }
                            None
                        }
                        _ => {
                            self.inline_into(&e.kids, s);
                            None
                        }
                    };
                    if let Some(tag) = tag {
                        s.push_str(&format!("<{tag}>"));
                        self.inline_into(&e.kids, s);
                        s.push_str(&format!("</{tag}>"));
                    }
                }
            }
        }
    }

    fn note(&mut self, e: &Elem) -> String {
        let id = e.attr("id").unwrap_or_default();
        let saved = std::mem::take(&mut self.out);
        let mut hc = false;
        let mut nb = None;
        // A note's first link back (↑, its number) is its title.
        self.flat(&e.kids, &mut nb, &mut hc);
        let inner = std::mem::replace(&mut self.out, saved);
        format!(
            "<section id=\"{}\">{}</section>",
            esc_attr(id),
            if inner.is_empty() {
                "<p/>".into()
            } else {
                inner
            }
        )
    }
}

fn heading(name: &str) -> Option<usize> {
    match name {
        "h1" => Some(1),
        "h2" => Some(2),
        "h3" => Some(3),
        "h4" => Some(4),
        "h5" => Some(5),
        "h6" => Some(6),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0" xmlns:l="http://www.w3.org/1999/xlink">
<description><title-info><genre>prose</genre><author><first-name>Lia</first-name><last-name>Marta</last-name></author>
<book-title>A Casa Vazia</book-title><coverpage><image l:href="#c.png"/></coverpage><lang>pt</lang>
<sequence name="Ciclo" number="2"/></title-info>
<publish-info><publisher>Editora Leve</publisher><year>2019</year><isbn>978-0-00-000001-9</isbn></publish-info></description>
<body><title><p>A Casa Vazia</p></title>
<section id="s1"><title><p>Capítulo 1</p></title><epigraph><p>Uma frase.</p><text-author>Alguém</text-author></epigraph>
<p>Primeiro <emphasis>parágrafo</emphasis> com nota<a l:href="#n1" type="note">1</a>.</p><empty-line/>
<poem><stanza><v>Verso um</v><v>Verso dois</v></stanza></poem></section>
<section><title><p>Capítulo 2</p></title><p>Fim &amp; adeus.</p><image l:href="#c.png"/></section>
</body>
<body name="notes"><title><p>Notas</p></title><section id="n1"><title><p>1</p></title><p>Texto da nota.</p></section></body>
<binary id="c.png" content-type="image/png">iVBORw0KGgo=</binary>
</FictionBook>"##;

    #[test]
    fn sections_made_valid() {
        let body = "<body><section><title><p>A</p></title></section><section><p>x</p><section><title><p>B</p></title><p>y</p></section><p>z</p></section></body>";
        assert_eq!(
            valid_sections(body),
            "<body><section><title><p>A</p></title><empty-line/></section><section><section><p>x</p></section><section><title><p>B</p></title><p>y</p></section><section><p>z</p></section></section></body>"
        );
        // Not nesting: left as it is.
        assert_eq!(valid_sections("<body><p>x</body>"), "<body><p>x</body>");
    }

    #[test]
    fn genres_from_the_list() {
        assert_eq!(genre_of("Ficção Científica"), Some("sf"));
        assert_eq!(genre_of("Não-ficção"), Some("nonfiction"));
        assert_eq!(genre_of("Teologia"), Some("religion"));
        assert_eq!(genre_of("Poesia"), Some("poetry"));
        assert_eq!(genre_of("sf_fantasy"), Some("sf_fantasy"));
        assert_eq!(genre_of("Xyz"), None);
    }

    #[test]
    fn reads_fb2() {
        let b = read(SAMPLE.as_bytes()).unwrap();
        assert_eq!(b.meta.title, "A Casa Vazia");
        assert_eq!(b.meta.authors, vec!["Lia Marta"]);
        assert_eq!(b.meta.series, "Ciclo");
        assert_eq!(b.meta.series_index, "2");
        assert_eq!(b.meta.publisher, "Editora Leve");
        assert_eq!(b.meta.date, "2019");
        assert_eq!(b.cover.as_deref(), Some("i0001.png"));
        // Title page, two chapters, notes.
        assert_eq!(b.chapters.len(), 4);
        let c1 = &b.chapters[1];
        assert_eq!(c1.title, "Capítulo 1");
        assert!(c1.body.contains("<h1>Capítulo 1</h1>"), "{}", c1.body);
        assert!(c1.body.contains(
            "<div class=\"epigraph\"><p>Uma frase.</p><p class=\"author\">Alguém</p></div>"
        ));
        assert!(c1.body.contains("href=\"notes.xhtml#n1\""), "{}", c1.body);
        assert!(
            c1.body
                .contains("<div class=\"stanza\"><p>Verso um</p><p>Verso dois</p></div>")
        );
        let notes = &b.chapters[3];
        assert!(notes.body.contains("<aside id=\"n1\""));
        assert!(
            notes.body.contains("href=\"c0002.xhtml#ref_n1\""),
            "{}",
            notes.body
        );
        assert_eq!(b.toc.len(), 2);
    }

    #[test]
    fn fb2_round_trip() {
        let b = read(SAMPLE.as_bytes()).unwrap();
        let s = to_fb2(&b);
        roxmltree::Document::parse(&s).unwrap_or_else(|e| panic!("{e}\n{s}"));
        let again = read(s.as_bytes()).unwrap();
        assert_eq!(again.meta.title, "A Casa Vazia");
        assert_eq!(again.meta.authors, vec!["Lia Marta"]);
        assert_eq!(again.meta.series, "Ciclo");
        assert!(again.chapters.iter().any(|c| c.body.contains("Verso um")));
        assert!(
            again
                .chapters
                .iter()
                .any(|c| c.body.contains("Texto da nota."))
        );
        assert!(again.cover.is_some());
    }

    #[test]
    fn base64_both_ways() {
        let d = b"any bytes \x00\xff here";
        assert_eq!(base64(&base64_encode(d)), d);
    }
}
