//! An EPUB (2 or 3) read into a [`Book`], to be written in another format.
//! Its chapters stay as they are (one per file of the reading order), with
//! its own style sheets, classes, pictures and fonts (but for fonts that
//! are scrambled: those are left out), its table of contents and cover.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::xhtml::{self, Node, Out};
use super::{Book, Chapter, Meta, Toc};
use crate::ebook::join_zip;

struct Item {
    href: String,
    media: String,
    props: String,
}

pub fn read(path: &Path) -> Result<Book, String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(std::io::BufReader::new(f)).map_err(|_| "bookBroken")?;
    let mut get = |name: &str| -> Option<Vec<u8>> {
        let mut f = z.by_name(name).ok()?;
        let mut v = Vec::new();
        (&mut f).take(256 << 20).read_to_end(&mut v).ok()?;
        Some(v)
    };
    let container = get("META-INF/container.xml").ok_or("bookBroken")?;
    let container = String::from_utf8_lossy(&container).into_owned();
    let opf_path = parse(&container)
        .and_then(|d| {
            d.descendants()
                .find(|n| n.has_tag_name("rootfile"))
                .and_then(|n| n.attribute("full-path"))
                .map(String::from)
        })
        .ok_or("bookBroken")?;
    let opf = get(&opf_path).ok_or("bookBroken")?;
    let opf = super::text_in::decode_bytes(&opf, true);
    let doc = parse(&opf).ok_or("bookBroken")?;
    let scrambled: Vec<String> = get("META-INF/encryption.xml")
        .map(|e| super::drm::obfuscated(&String::from_utf8_lossy(&e)))
        .unwrap_or_default();

    let mut book = Book {
        css: String::new(),
        ..Default::default()
    };
    meta_from_opf(&doc, &mut book.meta);

    let mut items: HashMap<String, Item> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for n in doc.descendants().filter(|n| n.has_tag_name("item")) {
        let (Some(id), Some(href)) = (n.attribute("id"), n.attribute("href")) else {
            continue;
        };
        order.push(id.to_string());
        items.insert(
            id.to_string(),
            Item {
                href: join_zip(&opf_path, href),
                media: n.attribute("media-type").unwrap_or_default().to_string(),
                props: n.attribute("properties").unwrap_or_default().to_string(),
            },
        );
    }
    let spine: Vec<String> = doc
        .descendants()
        .filter(|n| n.has_tag_name("itemref"))
        .filter_map(|n| n.attribute("idref"))
        .filter_map(|id| items.get(id).map(|i| i.href.clone()))
        .collect();
    if spine.is_empty() {
        return Err("bookBroken".into());
    }
    let spine_node = doc.descendants().find(|n| n.has_tag_name("spine"));
    book.rtl = spine_node.and_then(|s| s.attribute("page-progression-direction")) == Some("rtl");
    book.fixed = doc.descendants().any(|n| {
        n.has_tag_name("meta")
            && n.attribute("property") == Some("rendition:layout")
            && n.text().map(str::trim) == Some("pre-paginated")
    });

    // Resources: pictures, fonts, style sheets (path in the zip → name).
    let mut names: HashMap<String, String> = HashMap::new();
    let mut css_files = Vec::new();
    for id in &order {
        let it = &items[id];
        if it.media == "application/xhtml+xml"
            || it.media.contains("ncx")
            || it.media == "text/html"
        {
            continue;
        }
        if scrambled.iter().any(|s| s == &it.href) {
            continue;
        }
        if it.media == "text/css" {
            css_files.push(it.href.clone());
            continue;
        }
        if let Some(data) = get(&it.href)
            && let Some(name) = book.add_file(data, &it.href.to_ascii_lowercase())
        {
            names.insert(it.href.clone(), name);
        }
    }
    // The cover picture.
    let cover_id = doc
        .descendants()
        .find(|n| n.has_tag_name("meta") && n.attribute("name") == Some("cover"))
        .and_then(|n| n.attribute("content"));
    book.cover = order
        .iter()
        .find(|id| {
            items[*id]
                .props
                .split_whitespace()
                .any(|p| p == "cover-image")
        })
        .or_else(|| cover_id.and_then(|c| order.iter().find(|id| id.as_str() == c)))
        .and_then(|id| names.get(&items[id].href).cloned());

    // Chapters get new names; links between them follow.
    let mut chapter_name: HashMap<String, String> = HashMap::new();
    for (i, href) in spine.iter().enumerate() {
        chapter_name
            .entry(href.clone())
            .or_insert_with(|| format!("c{:04}.xhtml", i + 1));
    }
    // Style sheets, their `url()`s pointing at the new names.
    for href in &css_files {
        if let Some(data) = get(href) {
            let css = super::text_in::decode_bytes(&data, false);
            book.css.push_str(&rewrite_css(&css, href, &names));
            book.css.push('\n');
        }
    }
    let mut seen = std::collections::HashSet::new();
    for href in &spine {
        if !seen.insert(href.clone()) {
            continue;
        }
        let Some(bytes) = get(href) else { continue };
        let text = super::text_in::decode_bytes(&bytes, true);
        let nodes = xhtml::parse(&text);
        let title = xhtml::find(&nodes, "title")
            .map(|t| crate::text::collapse_spaces(t.plain().trim()))
            .unwrap_or_default();
        // Style inside the chapter's head.
        let mut styles = Vec::new();
        xhtml::find_all(&nodes, "style", &mut styles);
        for s in styles {
            book.css.push_str(&rewrite_css(&s.plain(), href, &names));
            book.css.push('\n');
        }
        let page = if book.fixed {
            let mut metas = Vec::new();
            xhtml::find_all(&nodes, "meta", &mut metas);
            metas
                .iter()
                .find(|m| m.attr("name") == Some("viewport"))
                .and_then(|m| viewport(m.attr("content")?))
        } else {
            None
        };
        let lang = nodes
            .iter()
            .find_map(|n| match n {
                Node::Elem(e) if e.name == "html" => {
                    e.attr("xml:lang").or(e.attr("lang")).map(String::from)
                }
                _ => None,
            })
            .unwrap_or_default();
        let (mut body, body_attrs) = xhtml::body_of(nodes);
        xhtml::tidy(&mut body);
        let link = |h: &str, pic: bool| -> Option<String> {
            if h.starts_with("http:") || h.starts_with("https:") || h.starts_with("mailto:") {
                return Some(h.to_string());
            }
            if let Some(f) = h.strip_prefix('#') {
                return Some(format!("#{f}"));
            }
            let target = join_zip(href, h);
            let frag = h
                .split_once('#')
                .map(|(_, f)| format!("#{f}"))
                .unwrap_or_default();
            if pic {
                return names.get(&target).cloned();
            }
            chapter_name
                .get(&target)
                .map(|c| format!("{c}{frag}"))
                .or_else(|| names.get(&target).cloned())
        };
        let mut s = String::new();
        // The body's class and id (styles and links use them) on a wrapper.
        let body_class = body_attrs
            .iter()
            .find(|(k, _)| k == "class")
            .map(|(_, v)| v.clone());
        let body_id = body_attrs
            .iter()
            .find(|(k, _)| k == "id")
            .map(|(_, v)| v.clone())
            .filter(|v| xhtml::valid_id(v));
        let wrapped = body_class.is_some() || body_id.is_some();
        if wrapped {
            s.push_str("<div");
            if let Some(i) = &body_id {
                s.push_str(&format!(" id=\"{}\"", xhtml::esc_attr(i)));
            }
            if let Some(c) = &body_class {
                s.push_str(&format!(" class=\"{}\"", xhtml::esc_attr(c)));
            }
            s.push('>');
        }
        xhtml::write(
            &body,
            &Out {
                href: &link,
                classes: true,
            },
            &mut s,
        );
        if wrapped {
            s.push_str("</div>");
        }
        book.chapters.push(Chapter {
            name: chapter_name[href].clone(),
            title,
            body: s,
            lang: if lang == book.meta.language {
                String::new()
            } else {
                lang
            },
            page,
        });
    }

    // Table of contents: EPUB 3's nav, else the NCX.
    let nav = order
        .iter()
        .find(|id| items[*id].props.split_whitespace().any(|p| p == "nav"))
        .map(|id| items[id].href.clone());
    if let Some(nav) = nav
        && let Some(bytes) = get(&nav)
    {
        let nodes = xhtml::parse(&super::text_in::decode_bytes(&bytes, true));
        let mut navs = Vec::new();
        xhtml::find_all(&nodes, "nav", &mut navs);
        if let Some(toc) = navs
            .iter()
            .find(|n| n.attr("epub:type").is_some_and(|t| t.contains("toc")))
            .or(navs.first())
            && let Some(ol) = xhtml::find(&toc.kids, "ol")
        {
            book.toc = nav_entries(ol, &nav, &chapter_name);
        }
    }
    if book.toc.is_empty() {
        let ncx = order
            .iter()
            .find(|id| items[*id].media.contains("ncx"))
            .map(|id| items[id].href.clone());
        if let Some(ncx) = ncx
            && let Some(bytes) = get(&ncx)
        {
            let text = super::text_in::decode_bytes(&bytes, true);
            if let Some(d) = parse(&text)
                && let Some(map) = d.descendants().find(|n| n.has_tag_name("navMap"))
            {
                book.toc = ncx_entries(map, &ncx, &chapter_name);
            }
        }
    }
    if book.css.trim().is_empty() {
        book.css = super::BASE_CSS.into();
    }
    // A chapter's title is its entry in the table of contents; the
    // `<title>` of its file is often the book's.
    let mut titles: HashMap<String, String> = HashMap::new();
    fn walk(t: &[Toc], m: &mut HashMap<String, String>) {
        for e in t {
            let file = e.href.split('#').next().unwrap_or_default().to_string();
            m.entry(file).or_insert_with(|| e.title.clone());
            walk(&e.kids, m);
        }
    }
    walk(&book.toc, &mut titles);
    let book_title = book.meta.title.clone();
    for c in &mut book.chapters {
        match titles.get(&c.name) {
            Some(t) => c.title = t.clone(),
            None if c.title == book_title || c.title.is_empty() => c.title.clear(),
            None => {}
        }
    }
    Ok(book)
}

fn parse(text: &str) -> Option<roxmltree::Document<'_>> {
    roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )
    .ok()
}

fn viewport(s: &str) -> Option<(u32, u32)> {
    let mut w = None;
    let mut h = None;
    for part in s.split([',', ';']) {
        let (k, v) = part.split_once('=')?;
        let v: u32 = v.trim().trim_end_matches("px").parse().ok()?;
        match k.trim() {
            "width" => w = Some(v),
            "height" => h = Some(v),
            _ => {}
        }
    }
    Some((w?, h?))
}

fn map_href(h: &str, base: &str, chapters: &HashMap<String, String>) -> Option<String> {
    let target = join_zip(base, h);
    let frag = h
        .split_once('#')
        .map(|(_, f)| format!("#{f}"))
        .unwrap_or_default();
    chapters.get(&target).map(|c| format!("{c}{frag}"))
}

fn nav_entries(ol: &xhtml::Elem, base: &str, chapters: &HashMap<String, String>) -> Vec<Toc> {
    let mut out = Vec::new();
    for li in &ol.kids {
        let Node::Elem(li) = li else { continue };
        if li.name != "li" {
            continue;
        }
        let a = li.kids.iter().find_map(|k| match k {
            Node::Elem(e) if e.name == "a" || e.name == "span" => Some(e),
            _ => None,
        });
        let kids = li
            .kids
            .iter()
            .find_map(|k| match k {
                Node::Elem(e) if e.name == "ol" => Some(nav_entries(e, base, chapters)),
                _ => None,
            })
            .unwrap_or_default();
        let title = a
            .map(|a| crate::text::collapse_spaces(a.plain().trim()))
            .unwrap_or_default();
        match a
            .and_then(|a| a.attr("href"))
            .and_then(|h| map_href(h, base, chapters))
        {
            Some(href) => out.push(Toc { title, href, kids }),
            // An entry without a link: its children move up.
            None => out.extend(kids),
        }
    }
    out
}

fn ncx_entries(
    parent: roxmltree::Node,
    base: &str,
    chapters: &HashMap<String, String>,
) -> Vec<Toc> {
    let mut out = Vec::new();
    for p in parent.children().filter(|n| n.has_tag_name("navPoint")) {
        let title = p
            .descendants()
            .find(|n| n.has_tag_name("text"))
            .and_then(|n| n.text())
            .map(|t| crate::text::collapse_spaces(t.trim()))
            .unwrap_or_default();
        let kids = ncx_entries(p, base, chapters);
        let href = p
            .children()
            .find(|n| n.has_tag_name("content"))
            .and_then(|n| n.attribute("src"))
            .and_then(|h| map_href(h, base, chapters));
        match href {
            Some(href) => out.push(Toc { title, href, kids }),
            None => out.extend(kids),
        }
    }
    out
}

/// `url(...)` in a style sheet pointed at the book's new file names;
/// `@import` dropped (its sheet is in the book already).
fn rewrite_css(css: &str, base: &str, names: &HashMap<String, String>) -> String {
    static URL: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"url\(\s*["']?([^"')]+)["']?\s*\)"#).unwrap());
    static IMPORT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"@import[^;]*;"#).unwrap());
    let css = IMPORT.replace_all(css, "");
    URL.replace_all(&css, |c: &regex::Captures| {
        let h = &c[1];
        if h.starts_with("data:") {
            return c[0].to_string();
        }
        match names.get(&join_zip(base, h)) {
            Some(n) => format!("url(\"{n}\")"),
            None => "url(\"\")".into(),
        }
    })
    .into_owned()
}

/// The details of an OPF package document, EPUB 2 or 3.
pub fn meta_from_opf(doc: &roxmltree::Document, m: &mut Meta) {
    let own = crate::ebook::opf_meta(doc);
    let set = |to: &mut String, v: &str| {
        if to.is_empty() && !v.is_empty() {
            *to = v.to_string();
        }
    };
    set(&mut m.title, &own.title);
    if m.authors.is_empty() {
        m.authors = own.authors.clone();
    }
    set(&mut m.language, &own.language);
    set(&mut m.publisher, &own.publisher);
    if let Some(y) = own.year {
        set(&mut m.date, &y.to_string());
    }
    if let Some(i) = own.isbns.first() {
        set(&mut m.isbn, i);
    }
    set(&mut m.series, &own.series);
    set(&mut m.series_index, &own.series_index);
    let Some(meta) = doc.descendants().find(|n| n.has_tag_name("metadata")) else {
        return;
    };
    for n in meta.children().filter(|n| n.is_element()) {
        let text = crate::text::collapse_spaces(n.text().unwrap_or_default().trim());
        match n.tag_name().name() {
            "description" if m.description.is_empty() => {
                // Often HTML: its text.
                let mut s = String::new();
                xhtml::plain_into(&xhtml::parse(&text), &mut s);
                m.description = crate::text::collapse_spaces(s.trim());
            }
            "subject" if !text.is_empty() && !m.subjects.contains(&text) => m.subjects.push(text),
            "date" if m.date.len() < 10 && text.len() >= 4 => {
                if m.date.is_empty() || text.starts_with(&m.date) {
                    m.date = text.chars().take(10).collect();
                }
            }
            "contributor" if !text.is_empty() => {
                let role = n
                    .attribute(("http://www.idpf.org/2007/opf", "role"))
                    .or_else(|| {
                        let id = n.attribute("id")?;
                        meta.children()
                            .find(|x| {
                                x.attribute("refines").map(|r| r.trim_start_matches('#'))
                                    == Some(id)
                                    && x.attribute("property") == Some("role")
                            })
                            .and_then(|x| x.text())
                    })
                    .unwrap_or("ctb");
                if !matches!(role, "bkp" | "mfr") {
                    m.contributors.push((text, role.trim().to_string()));
                }
            }
            "identifier" if m.id.is_empty() && !text.is_empty() => m.id = text,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn epub_round_trip() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("b.epub");
        let mut b = epub_out::tests::sample();
        b.chapters.push(Chapter {
            name: "c0002.xhtml".into(),
            title: "Dois".into(),
            body: "<h1>Dois</h1><p class=\"x\">Link <a href=\"c0001.xhtml\">volta</a>.</p>".into(),
            ..Default::default()
        });
        b.toc_from_chapters();
        b.toc.truncate(1);
        b.toc.push(Toc {
            title: "Dois".into(),
            href: "c0002.xhtml".into(),
            kids: vec![],
        });
        epub_out::write(&b, &p).unwrap();
        assert!(!drm::locked(&p, Source::Epub));
        let r = super::read(&p).unwrap();
        assert_eq!(r.meta.title, "Um Livro & Outro");
        assert_eq!(r.meta.series, "Ciclo");
        assert_eq!(r.chapters.len(), 3, "cover page, two chapters");
        assert!(r.cover.is_some());
        let last = &r.chapters[2].body;
        assert!(last.contains("<p class=\"x\">"), "{last}");
        assert!(last.contains("href=\"c0002.xhtml\""), "{last}");
        assert_eq!(r.toc.len(), 2);
        assert_eq!(r.toc[1].href, "c0003.xhtml");
        assert!(r.css.contains("text-indent"));
    }
}
