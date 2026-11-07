//! Plain text and HTML read into a [`Book`], and the pieces every format
//! shares: guessing a text's encoding, and cutting a long document into
//! chapters at its headings, with a table of contents.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::xhtml::{self, Elem, Node, Out};
use super::{Book, Chapter, Toc};

/// Text from bytes: a byte order mark, else the encoding the document
/// declares (`<?xml encoding>`, `<meta charset>`), else UTF-8 when valid,
/// else a Windows code page (Cyrillic when most letters need it).
pub fn decode_bytes(b: &[u8], markup: bool) -> String {
    if let Some((enc, skip)) = encoding_rs::Encoding::for_bom(b) {
        return enc.decode_without_bom_handling(&b[skip..]).0.into_owned();
    }
    if markup {
        let head = String::from_utf8_lossy(&b[..b.len().min(2048)]).to_ascii_lowercase();
        static DECL: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r#"(?:encoding|charset)\s*=\s*["']?([a-z0-9_:.-]+)"#).unwrap()
        });
        if let Some(c) = DECL.captures(&head)
            && let Some(enc) = encoding_rs::Encoding::for_label(c[1].as_bytes())
            && (enc != encoding_rs::UTF_8 || std::str::from_utf8(b).is_ok())
        {
            return enc.decode_without_bom_handling(b).0.into_owned();
        }
    }
    if let Ok(s) = std::str::from_utf8(b) {
        return s.to_string();
    }
    // Cyrillic text in Windows-1251 is almost all high bytes in
    // 0xC0–0xFF; Western text has high bytes only for accents.
    let high = b.iter().filter(|c| **c >= 0xc0).count();
    let letters = b.iter().filter(|c| c.is_ascii_alphabetic()).count();
    let enc = if high > letters {
        encoding_rs::WINDOWS_1251
    } else {
        encoding_rs::WINDOWS_1252
    };
    enc.decode_without_bom_handling(b).0.into_owned()
}

// ---------------------------------------------------------------- chapters

/// Headings that start a chapter in a text without markup.
static CHAPTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)^\s*(
            (cap[ií]tulo|chapter|chapitre|kapitel|cap[ií]tol|libro|livro|book|parte?|part|partie|teil|tomo|volume|acto?|act|cena|scene|canto|lecci[oó]n|li[cç][aã]o|lesson|se[cç][aã]o|secci[oó]n|section)
                (\s+([0-9]+|[ivxlcdm]+|[a-zà-ü]+(\s+[a-zà-ü]+)?)\b.*)?
            |pr[oó]logo|prologue|ep[ií]logo|epilogue|pref[aá]cio|preface|prefacio|introdu[cç][aã]o|introduction|introducci[oó]n|ap[eê]ndice|appendix|posf[aá]cio|afterword|nota\s+do\s+autor|author'?s\s+note|agradecimentos|acknowledgments?|agradecimientos|dedicat[oó]ria|sum[aá]rio|[ií]ndice|contents
            |[0-9]{1,3}\.?
            |[ivxlcdm]{1,7}\.?
        )\s*$",
    )
    .unwrap()
});

/// A scene break: `* * *`, `***`, `---`, `#`, `~`.
static BREAK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([*#~=•·◊§-]\s*){1,7}$|^\s*(\*\s*){3,}$").unwrap());

/// Splits `nodes` (a document's body, cleaned) into chapters at its main
/// headings, and adds them to `book` with a table of contents; links and
/// pictures are rewritten by `href`.
pub fn add_document(
    book: &mut Book,
    nodes: Vec<Node>,
    href: &dyn Fn(&str, bool) -> Option<String>,
    classes: bool,
) {
    add_document_with(book, nodes, href, classes, false)
}

/// [`add_document`]; with `breaks`, page breaks start chapters too (old
/// Kindle books mark chapters so).
pub fn add_document_with(
    book: &mut Book,
    nodes: Vec<Node>,
    href: &dyn Fn(&str, bool) -> Option<String>,
    classes: bool,
    breaks: bool,
) {
    let mut nodes = unwrap_wrappers(nodes);
    xhtml::tidy(&mut nodes);
    // The heading level chapters start at: the highest one used more
    // than once (or once, after some text, in a short book).
    let mut counts = [0usize; 7];
    for n in &nodes {
        if let Node::Elem(e) = n
            && let Some(l) = level(&e.name)
        {
            counts[l] += 1;
        }
    }
    let split = (1..=6).find(|l| counts[*l] >= 2);
    let mut parts: Vec<Vec<Node>> = vec![Vec::new()];
    for n in nodes {
        let starts = matches!(&n, Node::Elem(e) if level(&e.name).is_some_and(|l| Some(l) <= split))
            || matches!(&n, Node::Elem(e) if (e.name == "div" && e.attr("style").is_some_and(|s| s.contains("page-break-before: always")) || e.name == "mbp:pagebreak") && (split.is_none() || breaks));
        // A part with no text or picture yet (an anchor, a break) is the
        // start of the next one.
        let has_content = |p: &Vec<Node>| {
            p.iter().any(|x| match x {
                Node::Text(t) => !t.trim().is_empty(),
                Node::Elem(e) => {
                    let mut imgs = Vec::new();
                    xhtml::find_all(std::slice::from_ref(x), "img", &mut imgs);
                    e.name == "img" || !imgs.is_empty() || !e.plain().trim().is_empty()
                }
            })
        };
        if starts && parts.last().is_some_and(has_content) {
            parts.push(Vec::new());
        }
        parts.last_mut().unwrap().push(n);
    }
    // Long parts are cut at paragraphs: readers slow down on huge files.
    let mut cut = Vec::new();
    for p in parts {
        cut.extend(cut_long(p));
    }
    let out = Out { href, classes };
    let mut sub_id = 0usize;
    for mut p in cut {
        if p.iter().all(xhtml::is_blank) {
            continue;
        }
        let name = book.next_name();
        let mut title = String::new();
        let mut kids = Vec::new();
        for n in p.iter_mut() {
            if let Node::Elem(e) = n
                && let Some(l) = level(&e.name)
            {
                let t = crate::text::collapse_spaces(e.plain().trim());
                if t.is_empty() {
                    continue;
                }
                if title.is_empty() && split.is_none_or(|s| l <= s) {
                    title = t;
                } else if split.is_some_and(|s| l == s + 1) {
                    let id = match e.attr("id") {
                        Some(id) if xhtml::valid_id(id) => id.to_string(),
                        _ => {
                            sub_id += 1;
                            let id = format!("h{sub_id}");
                            e.set("id", id.clone());
                            id
                        }
                    };
                    kids.push(Toc {
                        title: t,
                        href: format!("{name}#{id}"),
                        kids: vec![],
                    });
                }
            }
        }
        let mut body = String::new();
        xhtml::write(&p, &out, &mut body);
        if !title.is_empty() {
            book.toc.push(Toc {
                title: title.clone(),
                href: name.clone(),
                kids,
            });
        } else if let Some(last) = book.toc.last_mut() {
            last.kids.extend(kids);
        }
        book.chapters.push(Chapter {
            name,
            title,
            body,
            ..Default::default()
        });
    }
}

fn level(name: &str) -> Option<usize> {
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

/// Wrappers (`<div>`, `<section>`…) holding headings give their content
/// to the level above, so chapters can be found; loose text there goes
/// in paragraphs.
fn unwrap_wrappers(nodes: Vec<Node>) -> Vec<Node> {
    let holds_heading = |e: &Elem| {
        e.kids.iter().any(|k| matches!(k, Node::Elem(x) if level(&x.name).is_some()
            || (matches!(x.name.as_str(), "div" | "section" | "article" | "main") && x.kids.iter().any(|y| matches!(y, Node::Elem(z) if level(&z.name).is_some())))))
    };
    let mut out = Vec::new();
    for n in nodes {
        match n {
            Node::Elem(e)
                if matches!(
                    e.name.as_str(),
                    "div" | "section" | "article" | "main" | "body" | "html"
                ) && holds_heading(&e)
                    && e.attr("class").is_none_or(|c| !c.contains("notes")) =>
            {
                let id = e
                    .attr("id")
                    .filter(|i| xhtml::valid_id(i))
                    .map(String::from);
                let mut kids = unwrap_wrappers(e.kids);
                // Its id goes to its first element, for links to it.
                if let Some(id) = id
                    && let Some(Node::Elem(first)) =
                        kids.iter_mut().find(|k| matches!(k, Node::Elem(_)))
                    && first.attr("id").is_none()
                {
                    first.set("id", id);
                }
                out.extend(kids);
            }
            n => out.push(n),
        }
    }
    wrap_loose(out)
}

/// Runs of text and inline elements at the top level, in a `<p>`.
pub fn wrap_loose(nodes: Vec<Node>) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::new();
    let mut run: Vec<Node> = Vec::new();
    let flush = |run: &mut Vec<Node>, out: &mut Vec<Node>| {
        if run.iter().any(|n| !xhtml::is_blank(n)) {
            let mut p = Elem::new("p");
            p.kids = std::mem::take(run);
            out.push(Node::Elem(p));
        }
        run.clear();
    };
    for n in nodes {
        let inline = match &n {
            Node::Text(_) => true,
            Node::Elem(e) => {
                !xhtml::is_block(&e.name)
                    && !matches!(
                        e.name.as_str(),
                        "table" | "img" | "figure" | "aside" | "svg" | "hr" | "br"
                    )
            }
        };
        if inline {
            run.push(n);
        } else {
            flush(&mut run, &mut out);
            out.push(n);
        }
    }
    flush(&mut run, &mut out);
    out
}

/// About 150 KB of text per file at most.
fn cut_long(part: Vec<Node>) -> Vec<Vec<Node>> {
    const MAX: usize = 150_000;
    let size = |n: &Node| match n {
        Node::Text(t) => t.len(),
        Node::Elem(e) => e.plain().len() + 16,
    };
    if part.iter().map(size).sum::<usize>() <= MAX {
        return vec![part];
    }
    let mut out = vec![Vec::new()];
    let mut at = 0;
    for n in part {
        let s = size(&n);
        let block = matches!(&n, Node::Elem(e) if matches!(e.name.as_str(), "p" | "div" | "blockquote" | "table" | "ul" | "ol" | "pre" | "hr"));
        if at + s > MAX && block && !out.last().unwrap().is_empty() {
            out.push(Vec::new());
            at = 0;
        }
        at += s;
        out.last_mut().unwrap().push(n);
    }
    out
}

// ---------------------------------------------------------------- text

/// A plain text file.
pub fn txt(bytes: &[u8]) -> Result<Book, String> {
    let text = decode_bytes(bytes, false)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    if text.trim().is_empty() {
        return Err("bookBroken".into());
    }
    let mut book = Book::new();
    let nodes = text_nodes(&text);
    add_document(&mut book, nodes, &|h, _| Some(h.to_string()), true);
    Ok(book)
}

fn elem(name: &str, text: &str) -> Node {
    let mut e = Elem::new(name);
    inline_marks(text, &mut e);
    Node::Elem(e)
}

/// `_italic_` and `*bold*` as plain texts mark them (Project-style
/// transcriptions): only around whole words.
fn inline_marks(text: &str, e: &mut Elem) {
    static MARK: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(^|[\s(\[“‘—-])(_([^_\s][^_]{0,300}?[^_\s]|[^_\s])_|\*([^*\s][^*]{0,300}?[^*\s]|[^*\s])\*)([\s.,;:!?)\]”’—-]|$)").unwrap()
    });
    let mut at = 0;
    for c in MARK.captures_iter(text) {
        let whole = c.get(2).unwrap();
        let lead = c.get(1).unwrap();
        e.text(&text[at..lead.end()]);
        let (tag, inner) = match (c.get(3), c.get(4)) {
            (Some(i), _) => ("em", i.as_str()),
            (_, Some(b)) => ("strong", b.as_str()),
            _ => continue,
        };
        let mut x = Elem::new(tag);
        x.text(inner);
        e.push(Node::Elem(x));
        at = whole.end();
    }
    e.text(&text[at..]);
}

/// Paragraphs from plain text: blank lines part them; a block of long
/// lines broken at a fixed width is joined back; a block of short lines
/// (a poem, an address) keeps its lines. Lines that look like chapter
/// titles become headings, and `* * *` a scene break.
pub fn text_nodes(text: &str) -> Vec<Node> {
    let lines: Vec<&str> = text.lines().map(|l| l.trim_end()).collect();
    let len = |l: &str| l.trim().chars().count();
    let blank = lines.iter().filter(|l| l.trim().is_empty()).count();
    let filled = lines.len() - blank;
    let mut lengths: Vec<usize> = lines.iter().map(|l| len(l)).filter(|n| *n > 0).collect();
    lengths.sort_unstable();
    // The width the text was broken at, if it was: most lines end near it.
    let wrap = lengths.get(lengths.len() * 9 / 10).copied().unwrap_or(0);
    let wrapped = (45..=110).contains(&wrap) && {
        let near = lengths.iter().filter(|n| **n + 20 >= wrap).count();
        near * 2 > lengths.len()
    };
    let indented = lines
        .iter()
        .filter(|l| l.starts_with("  ") || l.starts_with('\t') || l.starts_with('\u{3000}'))
        .count();

    // Blocks of lines that belong together.
    let mut blocks: Vec<Vec<&str>> = vec![Vec::new()];
    let by_blank = blank * 8 >= filled;
    for l in &lines {
        if l.trim().is_empty() {
            if !blocks.last().unwrap().is_empty() {
                blocks.push(Vec::new());
            }
            continue;
        }
        let cur = blocks.last().unwrap();
        let starts = if by_blank {
            false
        } else if indented * 10 > filled && indented * 10 < filled * 9 {
            // Paragraphs start indented.
            l.starts_with("  ") || l.starts_with('\t') || l.starts_with('\u{3000}')
        } else if wrapped {
            // A line well short of the width ended the paragraph before.
            cur.last().is_some_and(|prev| len(prev) * 4 < wrap * 3)
        } else {
            true
        };
        if starts && !cur.is_empty() {
            blocks.push(Vec::new());
        }
        blocks.last_mut().unwrap().push(l);
    }

    let mut out = Vec::new();
    let mut after_break = true;
    for b in blocks.into_iter().filter(|b| !b.is_empty()) {
        let first = b[0].trim();
        if b.len() == 1 && BREAK.is_match(first) {
            let mut e = Elem::new("p");
            e.set("class", "break");
            e.text("* * *");
            out.push(Node::Elem(e));
            after_break = true;
            continue;
        }
        if b.len() <= 2 && CHAPTER.is_match(first) && first.chars().count() <= 60 {
            let title = b.iter().map(|l| l.trim()).collect::<Vec<_>>().join(" — ");
            out.push(elem("h2", &title));
            after_break = true;
            continue;
        }
        let max = b.iter().map(|l| len(l)).max().unwrap_or(0);
        // Prose broken at a width: every line but the last near the
        // longest. Else (a poem, an address, a list) lines are kept.
        let prose = b.len() == 1
            || !by_blank
            || (max >= 45 && b[..b.len() - 1].iter().all(|l| len(l) * 10 >= max * 6));
        let mut e = Elem::new("p");
        if after_break {
            e.set("class", "noindent");
        }
        if prose {
            inline_marks(&join_lines(&b), &mut e);
        } else {
            for (i, l) in b.iter().enumerate() {
                if i > 0 {
                    e.push(Node::Elem(Elem::new("br")));
                }
                inline_marks(l.trim(), &mut e);
            }
        }
        out.push(Node::Elem(e));
        after_break = false;
    }
    out
}

/// Lines of one paragraph joined; a word broken with a hyphen at the end
/// of a line is joined back when the next line starts in lower case.
fn join_lines(lines: &[&str]) -> String {
    let mut s = String::new();
    for l in lines {
        let l = l.trim();
        if s.ends_with('-')
            && !s.ends_with("--")
            && s[..s.len() - 1]
                .chars()
                .last()
                .is_some_and(|c| c.is_alphabetic())
            && l.chars().next().is_some_and(|c| c.is_lowercase())
        {
            s.pop();
            s.push_str(l);
        } else {
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str(l);
        }
    }
    s
}

// ---------------------------------------------------------------- HTML

/// An HTML file with its pictures beside it.
pub fn html_file(path: &Path) -> Result<Book, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    html(&bytes, &|src: &str| {
        let p = dir.join(crate::ebook::join_zip("x", src));
        // Only files beside it or below.
        let real = p.canonicalize().ok()?;
        if !real.starts_with(dir.canonicalize().ok()?) {
            return None;
        }
        std::fs::read(real).ok().filter(|d| d.len() < 64 << 20)
    })
}

/// An HTMLZ: a zip with `index.html`, its pictures and a `metadata.opf`.
pub fn htmlz(path: &Path) -> Result<Book, String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let z = std::cell::RefCell::new(zip::ZipArchive::new(f).map_err(|_| "bookBroken")?);
    let read = |name: &str| -> Option<Vec<u8>> {
        let mut z = z.borrow_mut();
        let mut f = z.by_name(name).ok()?;
        let mut v = Vec::new();
        (&mut f).take(64 << 20).read_to_end(&mut v).ok()?;
        Some(v)
    };
    let index = read("index.html")
        .or_else(|| read("index.htm"))
        .ok_or("bookBroken")?;
    let mut book = html(&index, &|src| {
        read(&crate::ebook::join_zip("index.html", src))
    })?;
    if let Some(opf) = read("metadata.opf") {
        let opf = String::from_utf8_lossy(&opf).into_owned();
        if let Ok(doc) = roxmltree::Document::parse_with_options(
            &opf,
            roxmltree::ParsingOptions {
                allow_dtd: true,
                ..Default::default()
            },
        ) {
            super::epub_in::meta_from_opf(&doc, &mut book.meta);
        }
    }
    Ok(book)
}

/// An HTML document; `load` gives the bytes of a picture it names.
pub fn html(bytes: &[u8], load: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<Book, String> {
    let text = decode_bytes(bytes, true);
    let nodes = xhtml::parse(&text);
    let mut book = Book::new();
    if let Some(t) = xhtml::find(&nodes, "title") {
        book.meta.title = crate::text::collapse_spaces(t.plain().trim());
    }
    let mut metas = Vec::new();
    xhtml::find_all(&nodes, "meta", &mut metas);
    for m in metas {
        let (Some(n), Some(c)) = (m.attr("name"), m.attr("content")) else {
            continue;
        };
        match n.to_ascii_lowercase().as_str() {
            "author" | "dc.creator" if !c.trim().is_empty() => {
                book.meta.authors.push(c.trim().into())
            }
            "description" | "dc.description" => book.meta.description = c.trim().into(),
            "dc.language" | "language" => book.meta.language = c.trim().into(),
            _ => {}
        }
    }
    if let Some(Node::Elem(h)) = nodes
        .iter()
        .find(|n| matches!(n, Node::Elem(e) if e.name == "html"))
        && let Some(l) = h.attr("lang").or_else(|| h.attr("xml:lang"))
        && book.meta.language.is_empty()
    {
        book.meta.language = l.into();
    }
    let (body, _) = xhtml::body_of(nodes);
    let pics = load_pictures(&mut book, &body, load);
    let href = |h: &str, pic: bool| -> Option<String> {
        if pic {
            return pics.get(h).cloned();
        }
        if h.starts_with('#')
            || h.starts_with("http:")
            || h.starts_with("https:")
            || h.starts_with("mailto:")
        {
            Some(h.to_string())
        } else {
            h.split_once('#').map(|(_, f)| format!("#{f}"))
        }
    };
    add_document(&mut book, body, &href, false);
    if book.chapters.is_empty() {
        return Err("bookBroken".into());
    }
    super::fix_links(&mut book);
    Ok(book)
}

/// Adds every picture `nodes` show to the book: their addresses → names.
pub fn load_pictures(
    book: &mut Book,
    nodes: &[Node],
    load: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> HashMap<String, String> {
    let mut srcs = Vec::new();
    collect_pictures(nodes, &mut srcs);
    let mut pics = HashMap::new();
    for s in srcs {
        if pics.contains_key(&s) {
            continue;
        }
        if let Some(data) = load(&s)
            && let Some(name) = book.add_file(data, &s.to_ascii_lowercase())
            && book
                .files
                .iter()
                .any(|r| r.name == name && r.media.starts_with("image/"))
        {
            pics.insert(s, name);
        }
    }
    pics
}

fn collect_pictures(nodes: &[Node], out: &mut Vec<String>) {
    for n in nodes {
        if let Node::Elem(e) = n {
            if e.name == "img" || e.name == "image" {
                for k in ["src", "href", "xlink:href", "recindex"] {
                    if let Some(v) = e.attr(k) {
                        out.push(v.to_string());
                        break;
                    }
                }
            }
            collect_pictures(&e.kids, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_text_becomes_paragraphs_and_chapters() {
        let t = "Capítulo 1\n\nEra uma vez uma casa muito antiga que ficava no alto de\numa colina, longe de tudo e de todos, onde ninguém ia.\n\nO vento soprava forte naquela noite e as janelas batiam so-\nzinhas. Dizia-se _muito_ isso.\n\n* * *\n\nOutra parte.\n\nCapítulo 2\n\nFim.\n";
        let b = txt(t.as_bytes()).unwrap();
        assert_eq!(b.chapters.len(), 2);
        assert_eq!(b.chapters[0].title, "Capítulo 1");
        let c = &b.chapters[0].body;
        assert!(c.contains("<h2>Capítulo 1</h2>"), "{c}");
        assert!(c.contains("alto de uma colina"), "{c}");
        assert!(c.contains("batiam sozinhas"), "{c}");
        assert!(c.contains("<em>muito</em>"), "{c}");
        assert!(c.contains("<p class=\"break\">* * *</p>"), "{c}");
        assert_eq!(b.toc.len(), 2);
    }

    #[test]
    fn short_lines_stay_lines() {
        let t = "Verso um\nverso dois\nverso três\n\nOutra estrofe\ncom duas linhas\n";
        let nodes = text_nodes(t);
        let mut s = String::new();
        xhtml::write(
            &nodes,
            &Out {
                href: &|h, _| Some(h.into()),
                classes: true,
            },
            &mut s,
        );
        assert!(s.contains("Verso um<br/>verso dois<br/>verso três"), "{s}");
    }

    #[test]
    fn decodes_old_encodings() {
        assert_eq!(
            decode_bytes(b"caf\xe9 com a\xe7\xfacar", false),
            "café com açúcar"
        );
        assert_eq!(decode_bytes("ok ç".as_bytes(), false), "ok ç");
        let cyr = encoding_rs::WINDOWS_1251.encode("Привет мир").0;
        assert_eq!(decode_bytes(&cyr, false), "Привет мир");
        assert_eq!(
            decode_bytes(
                b"<?xml version=\"1.0\" encoding=\"windows-1252\"?><a>\x93x\x94</a>",
                true
            ),
            "<?xml version=\"1.0\" encoding=\"windows-1252\"?><a>“x”</a>"
        );
    }

    #[test]
    fn html_splits_at_headings() {
        let h = "<html lang=pt><head><title>Livro</title><meta name=author content='Lia Marta'></head><body><div><h1>Um</h1><p>a<h2>Sub</h2><p>b<h1>Dois</h1><p>c<a href='#x'>link</a></div><p id=x>alvo</body></html>";
        let b = html(h.as_bytes(), &|_| None).unwrap();
        assert_eq!(b.meta.title, "Livro");
        assert_eq!(b.meta.authors, vec!["Lia Marta"]);
        assert_eq!(b.meta.language, "pt");
        assert_eq!(b.chapters.len(), 2);
        assert_eq!(b.toc[0].kids[0].title, "Sub");
        assert!(
            b.chapters[0].body.contains("<h2 id=\"h1\">Sub</h2>"),
            "{}",
            b.chapters[0].body
        );
        assert!(
            b.chapters[1].body.contains("href=\"#x\""),
            "{}",
            b.chapters[1].body
        );
    }
}
