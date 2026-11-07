//! A [`Book`] written as EPUB 3, with what EPUB 2 readers look for too
//! (an NCX table of contents, the cover named in the metadata, a guide):
//! old e-readers and apps open it as well as new ones. All files sit in
//! one folder (`OEBPS/`), so links between them are plain names.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use zip::write::SimpleFileOptions;

use super::xhtml::{esc, esc_attr};
use super::{Book, Toc};

/// A chapter's full XHTML document.
pub fn chapter_doc(
    book: &Book,
    title: &str,
    body: &str,
    lang: &str,
    page: Option<(u32, u32)>,
) -> String {
    let lang = if lang.is_empty() {
        &book.meta.language
    } else {
        lang
    };
    let viewport = page
        .map(|(w, h)| format!("<meta name=\"viewport\" content=\"width={w}, height={h}\"/>"))
        .unwrap_or_default();
    let dir = if book.rtl { " dir=\"rtl\"" } else { "" };
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"{l}\" lang=\"{l}\"{dir}>\n<head>\n<meta charset=\"utf-8\"/>\n{viewport}<title>{t}</title>\n<link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\"/>\n</head>\n<body>\n{body}\n</body>\n</html>\n",
        l = esc_attr(lang),
        t = esc(if title.is_empty() {
            &book.meta.title
        } else {
            title
        }),
    )
}

/// A fixed-layout page showing one picture.
pub fn picture_page(name: &str) -> String {
    format!(
        "<div class=\"page\"><img src=\"{}\" alt=\"\" style=\"width: 100%; height: 100%\"/></div>",
        esc_attr(name)
    )
}

fn nav_list(toc: &[Toc], s: &mut String) {
    s.push_str("<ol>\n");
    for t in toc {
        s.push_str(&format!(
            "<li><a href=\"{}\">{}</a>",
            esc_attr(&t.href),
            esc(&t.title)
        ));
        if !t.kids.is_empty() {
            nav_list(&t.kids, s);
        }
        s.push_str("</li>\n");
    }
    s.push_str("</ol>\n");
}

fn ncx_points(toc: &[Toc], n: &mut usize, s: &mut String) {
    for t in toc {
        *n += 1;
        s.push_str(&format!(
            "<navPoint id=\"p{n}\" playOrder=\"{n}\"><navLabel><text>{}</text></navLabel><content src=\"{}\"/>\n",
            esc(&t.title),
            esc_attr(&t.href),
        ));
        ncx_points(&t.kids, n, s);
        s.push_str("</navPoint>\n");
    }
}

fn depth(toc: &[Toc]) -> usize {
    toc.iter().map(|t| 1 + depth(&t.kids)).max().unwrap_or(0)
}

/// A new identifier for a book without one: from its title, authors and
/// content, so the same book converted twice gets the same.
fn uuid_of(book: &Book) -> String {
    let mut h = xxhash_rust::xxh3::Xxh3::new();
    h.update(book.meta.title.as_bytes());
    for a in &book.meta.authors {
        h.update(a.as_bytes());
    }
    for c in &book.chapters {
        h.update(c.body.as_bytes());
    }
    let mut v = h.digest128().to_be_bytes();
    v[6] = (v[6] & 0x0f) | 0x40;
    v[8] = (v[8] & 0x3f) | 0x80;
    let x: String = v.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "urn:uuid:{}-{}-{}-{}-{}",
        &x[..8],
        &x[8..12],
        &x[12..16],
        &x[16..20],
        &x[20..]
    )
}

/// The package document.
pub fn opf(book: &Book, has_cover_page: bool) -> String {
    let m = &book.meta;
    let id = if !m.isbn.is_empty() {
        format!("urn:isbn:{}", m.isbn)
    } else if !m.id.is_empty() {
        m.id.clone()
    } else {
        uuid_of(book)
    };
    let mut meta = String::new();
    meta.push_str(&format!(
        "<dc:identifier id=\"bookid\">{}</dc:identifier>\n",
        esc(&id)
    ));
    meta.push_str(&format!(
        "<dc:title id=\"title\">{}</dc:title>\n",
        esc(&m.title)
    ));
    meta.push_str(&format!(
        "<dc:language>{}</dc:language>\n",
        esc(if m.language.is_empty() {
            "und"
        } else {
            &m.language
        })
    ));
    for (i, a) in m.authors.iter().enumerate() {
        meta.push_str(&format!(
            "<dc:creator id=\"a{i}\">{}</dc:creator>\n<meta refines=\"#a{i}\" property=\"role\" scheme=\"marc:relators\">aut</meta>\n",
            esc(a)
        ));
    }
    for (i, (name, role)) in m.contributors.iter().enumerate() {
        meta.push_str(&format!(
            "<dc:contributor id=\"c{i}\">{}</dc:contributor>\n<meta refines=\"#c{i}\" property=\"role\" scheme=\"marc:relators\">{r}</meta>\n",
            esc(name),
            r = esc_attr(role)
        ));
    }
    if !m.publisher.is_empty() {
        meta.push_str(&format!(
            "<dc:publisher>{}</dc:publisher>\n",
            esc(&m.publisher)
        ));
    }
    if !m.date.is_empty() {
        meta.push_str(&format!("<dc:date>{}</dc:date>\n", esc(&m.date)));
    }
    if !m.isbn.is_empty() && !m.id.is_empty() && m.id != format!("urn:isbn:{}", m.isbn) {
        meta.push_str(&format!("<dc:identifier>{}</dc:identifier>\n", esc(&m.id)));
    }
    if !m.description.is_empty() {
        meta.push_str(&format!(
            "<dc:description>{}</dc:description>\n",
            esc(&m.description)
        ));
    }
    for s in &m.subjects {
        meta.push_str(&format!("<dc:subject>{}</dc:subject>\n", esc(s)));
    }
    if !m.series.is_empty() {
        meta.push_str(&format!(
            "<meta property=\"belongs-to-collection\" id=\"series\">{}</meta>\n<meta refines=\"#series\" property=\"collection-type\">series</meta>\n",
            esc(&m.series)
        ));
        if !m.series_index.is_empty() {
            meta.push_str(&format!(
                "<meta refines=\"#series\" property=\"group-position\">{}</meta>\n",
                esc(&m.series_index)
            ));
        }
        // What most reading apps read for series.
        meta.push_str(&format!(
            "<meta name=\"calibre:series\" content=\"{}\"/>\n",
            esc_attr(&m.series)
        ));
        if !m.series_index.is_empty() {
            meta.push_str(&format!(
                "<meta name=\"calibre:series_index\" content=\"{}\"/>\n",
                esc_attr(&m.series_index)
            ));
        }
    }
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    meta.push_str(&format!(
        "<meta property=\"dcterms:modified\">{now}</meta>\n"
    ));
    if let Some(c) = &book.cover {
        meta.push_str(&format!(
            "<meta name=\"cover\" content=\"{}\"/>\n",
            id_of(c)
        ));
    }
    if book.fixed {
        meta.push_str("<meta property=\"rendition:layout\">pre-paginated</meta>\n<meta property=\"rendition:spread\">auto</meta>\n");
    }

    let mut items = String::new();
    items.push_str("<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n");
    items.push_str("<item id=\"ncx\" href=\"toc.ncx\" media-type=\"application/x-dtbncx+xml\"/>\n");
    items.push_str("<item id=\"css\" href=\"style.css\" media-type=\"text/css\"/>\n");
    if has_cover_page {
        items.push_str(
            "<item id=\"coverpage\" href=\"cover.xhtml\" media-type=\"application/xhtml+xml\"/>\n",
        );
    }
    for c in &book.chapters {
        let mut props = Vec::new();
        if c.body.contains("<svg") {
            props.push("svg");
        }
        let props = if props.is_empty() {
            String::new()
        } else {
            format!(" properties=\"{}\"", props.join(" "))
        };
        items.push_str(&format!(
            "<item id=\"{}\" href=\"{}\" media-type=\"application/xhtml+xml\"{props}/>\n",
            id_of(&c.name),
            esc_attr(&c.name)
        ));
    }
    for r in &book.files {
        let cover = if book.cover.as_deref() == Some(&r.name) {
            " properties=\"cover-image\""
        } else {
            ""
        };
        items.push_str(&format!(
            "<item id=\"{}\" href=\"{}\" media-type=\"{}\"{cover}/>\n",
            id_of(&r.name),
            esc_attr(&r.name),
            esc_attr(&r.media)
        ));
    }
    let mut spine = String::new();
    if has_cover_page {
        spine.push_str("<itemref idref=\"coverpage\" linear=\"yes\"/>\n");
    }
    for c in &book.chapters {
        spine.push_str(&format!("<itemref idref=\"{}\"/>\n", id_of(&c.name)));
    }
    let dir = if book.rtl {
        " page-progression-direction=\"rtl\""
    } else {
        ""
    };
    let mut guide = String::new();
    if has_cover_page {
        guide.push_str("<reference type=\"cover\" title=\"Cover\" href=\"cover.xhtml\"/>\n");
    }
    if let Some(first) = book.chapters.first() {
        guide.push_str(&format!(
            "<reference type=\"text\" title=\"Text\" href=\"{}\"/>\n",
            esc_attr(&first.name)
        ));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"bookid\" xml:lang=\"{lang}\" prefix=\"rendition: http://www.idpf.org/vocab/rendition/#\">\n<metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:opf=\"http://www.idpf.org/2007/opf\">\n{meta}</metadata>\n<manifest>\n{items}</manifest>\n<spine toc=\"ncx\"{dir}>\n{spine}</spine>\n<guide>\n{guide}</guide>\n</package>\n",
        lang = esc_attr(if book.meta.language.is_empty() {
            "und"
        } else {
            &book.meta.language
        }),
    )
}

/// A manifest id for a file name.
pub fn id_of(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if !s.starts_with(|c: char| c.is_ascii_alphabetic()) {
        s.insert(0, 'x');
    }
    s
}

/// Writes `book` as an EPUB at `to`.
pub fn write(book: &Book, to: &Path) -> Result<(), String> {
    write_inner(book, to).map_err(|e| e.to_string())
}

fn write_inner(book: &Book, to: &Path) -> std::io::Result<()> {
    let file = File::create(to)?;
    let mut z = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    z.start_file("mimetype", stored)?;
    z.write_all(b"application/epub+zip")?;
    z.start_file("META-INF/container.xml", deflated)?;
    z.write_all(
        b"<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n<rootfiles>\n<rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/>\n</rootfiles>\n</container>\n",
    )?;
    // A cover page, unless the first chapter already shows the cover.
    let cover_page = book.cover.as_ref().filter(|c| {
        !book.fixed
            && !book
                .chapters
                .first()
                .is_some_and(|f| f.body.contains(&format!("\"{c}\"")))
    });
    z.start_file("OEBPS/content.opf", deflated)?;
    z.write_all(opf(book, cover_page.is_some()).as_bytes())?;

    // Table of contents: the given one, else the chapters' titles, else
    // the first chapter (a nav must have one link).
    let mut toc = book.toc.clone();
    if toc.is_empty() {
        toc = book
            .chapters
            .iter()
            .filter(|c| !c.title.trim().is_empty())
            .map(|c| Toc {
                title: c.title.clone(),
                href: c.name.clone(),
                kids: vec![],
            })
            .collect();
    }
    if toc.is_empty()
        && let Some(f) = book.chapters.first()
    {
        toc.push(Toc {
            title: if book.meta.title.is_empty() {
                "Start".into()
            } else {
                book.meta.title.clone()
            },
            href: f.name.clone(),
            kids: vec![],
        });
    }
    let mut nav = String::from("<nav epub:type=\"toc\" id=\"toc\">\n");
    nav.push_str(&format!("<h1>{}</h1>\n", esc(&book.meta.title)));
    nav_list(&toc, &mut nav);
    nav.push_str("</nav>\n");
    if cover_page.is_some() || !book.chapters.is_empty() {
        nav.push_str("<nav epub:type=\"landmarks\" hidden=\"\">\n<ol>\n");
        if cover_page.is_some() {
            nav.push_str("<li><a epub:type=\"cover\" href=\"cover.xhtml\">Cover</a></li>\n");
        }
        if let Some(f) = book.chapters.first() {
            nav.push_str(&format!(
                "<li><a epub:type=\"bodymatter\" href=\"{}\">Start</a></li>\n",
                esc_attr(&f.name)
            ));
        }
        nav.push_str("</ol>\n</nav>\n");
    }
    z.start_file("OEBPS/nav.xhtml", deflated)?;
    z.write_all(chapter_doc(book, &book.meta.title, &nav, "", None).as_bytes())?;

    let mut points = String::new();
    let mut n = 0;
    ncx_points(&toc, &mut n, &mut points);
    let ncx = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ncx xmlns=\"http://www.daisy.org/z3986/2005/ncx/\" version=\"2005-1\">\n<head>\n<meta name=\"dtb:uid\" content=\"{uid}\"/>\n<meta name=\"dtb:depth\" content=\"{d}\"/>\n<meta name=\"dtb:totalPageCount\" content=\"0\"/>\n<meta name=\"dtb:maxPageNumber\" content=\"0\"/>\n</head>\n<docTitle><text>{t}</text></docTitle>\n<navMap>\n{points}</navMap>\n</ncx>\n",
        uid = esc_attr(&if book.meta.isbn.is_empty() {
            uuid_of(book)
        } else {
            format!("urn:isbn:{}", book.meta.isbn)
        }),
        d = depth(&toc).max(1),
        t = esc(&book.meta.title),
    );
    z.start_file("OEBPS/toc.ncx", deflated)?;
    z.write_all(ncx.as_bytes())?;

    let mut css = book.css.clone();
    if book.fixed {
        css.push_str("\nhtml, body { margin: 0; padding: 0; width: 100%; height: 100%; }\n.page { margin: 0; padding: 0; width: 100%; height: 100%; text-align: center; }\n.page img { display: block; margin: 0 auto; }\n");
    }
    z.start_file("OEBPS/style.css", deflated)?;
    z.write_all(css.as_bytes())?;

    if let Some(c) = cover_page {
        let body = format!(
            "<div class=\"cover\"><img src=\"{}\" alt=\"\"/></div>",
            esc_attr(c)
        );
        z.start_file("OEBPS/cover.xhtml", deflated)?;
        z.write_all(chapter_doc(book, &book.meta.title, &body, "", None).as_bytes())?;
    }
    for c in &book.chapters {
        z.start_file(format!("OEBPS/{}", c.name), deflated)?;
        z.write_all(chapter_doc(book, &c.title, &c.body, &c.lang, c.page).as_bytes())?;
    }
    for r in &book.files {
        // Pictures are compressed already.
        let o = if r.media.starts_with("image/") && r.media != "image/svg+xml" {
            stored
        } else {
            deflated
        };
        z.start_file(format!("OEBPS/{}", r.name), o)?;
        z.write_all(&r.data)?;
    }
    let w = z.finish()?;
    let f = w.into_inner().map_err(|e| e.into_error())?;
    f.sync_all()?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::*;
    use std::io::Read;

    pub fn read_entry(path: &std::path::Path, name: &str) -> String {
        let mut z = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        let mut s = String::new();
        z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
        s
    }

    pub fn sample() -> Book {
        let mut b = Book::new();
        b.meta.title = "Um Livro & Outro".into();
        b.meta.authors = vec!["Lia Marta".into()];
        b.meta.language = "pt".into();
        b.meta.series = "Ciclo".into();
        b.meta.series_index = "2".into();
        let pic = b.add_file(b"\x89PNGfake".to_vec(), "x.png").unwrap();
        b.cover = Some(pic.clone());
        b.chapters.push(Chapter {
            name: "c0001.xhtml".into(),
            title: "Capítulo 1".into(),
            body: "<h1>Capítulo 1</h1><p>Texto &amp; mais.</p>".into(),
            ..Default::default()
        });
        b.toc_from_chapters();
        b
    }

    #[test]
    fn writes_a_valid_epub() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("b.epub");
        super::write(&sample(), &p).unwrap();
        let z = zip::ZipArchive::new(std::fs::File::open(&p).unwrap()).unwrap();
        assert_eq!(
            z.name_for_index(0).and_then(Result::ok).as_deref(),
            Some("mimetype")
        );
        for name in [
            "OEBPS/content.opf",
            "OEBPS/nav.xhtml",
            "OEBPS/toc.ncx",
            "OEBPS/c0001.xhtml",
            "OEBPS/cover.xhtml",
        ] {
            let s = read_entry(&p, name);
            roxmltree::Document::parse_with_options(
                &s,
                roxmltree::ParsingOptions {
                    allow_dtd: true,
                    ..Default::default()
                },
            )
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        let opf = read_entry(&p, "OEBPS/content.opf");
        assert!(opf.contains("<dc:title id=\"title\">Um Livro &amp; Outro</dc:title>"));
        assert!(opf.contains("properties=\"cover-image\""));
        assert!(opf.contains("group-position\">2<"));
        // What the book reader of this program reads back.
        let m = crate::ebook::read(&p);
        assert_eq!(m.title, "Um Livro & Outro");
        assert_eq!(m.authors, vec!["Lia Marta"]);
        assert_eq!(m.series, "Ciclo");
    }
}
