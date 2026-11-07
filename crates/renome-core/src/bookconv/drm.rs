//! Books locked by DRM, which are never converted: an EPUB whose content
//! is encrypted (Adobe, Apple, Readium LCP, Kobo, Barnes & Noble…), a
//! Kindle book with an encryption type, a KFX in a DRM envelope, a PDF
//! with an `/Encrypt` dictionary (even one that opens without a
//! password: its locks still say what may be done with it). Nothing here
//! tries to open a lock; a book is only looked at to tell.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use super::Source;

/// Font obfuscation: not DRM, only fonts are scrambled (they are left out
/// when converting).
const FONT_ONLY: &[&str] = &[
    "http://www.idpf.org/2008/embedding",
    "http://ns.adobe.com/pdf/enc#RC",
];

/// Whether the book at `path` (in format `s`) is locked.
pub fn locked(path: &Path, s: Source) -> bool {
    match s {
        Source::Epub => epub_locked(path),
        Source::Kindle => kindle_locked(path),
        Source::Pdf => pdf_locked(path),
        Source::Other => {
            let mut head = [0u8; 8];
            File::open(path)
                .and_then(|mut f| f.read_exact(&mut head))
                .is_ok_and(|_| head.starts_with(b"\xeaDRMION"))
        }
        _ => false,
    }
}

/// Files that only exist in locked EPUBs.
fn lock_file(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(
        n.as_str(),
        "meta-inf/rights.xml" | "meta-inf/sinf.xml" | "meta-inf/license.lcpl"
    ) || n.ends_with("/license.lcpl")
        || n == "rights.xml"
}

pub fn epub_locked(path: &Path) -> bool {
    let Ok(f) = File::open(path) else {
        return false;
    };
    let Ok(mut z) = zip::ZipArchive::new(std::io::BufReader::new(f)) else {
        return false;
    };
    if z.file_names().filter_map(Result::ok).any(|n| lock_file(&n)) {
        return true;
    }
    let Ok(mut enc) = z.by_name("META-INF/encryption.xml") else {
        return false;
    };
    let mut s = String::new();
    if (&mut enc).take(4 << 20).read_to_string(&mut s).is_err() {
        return true;
    }
    encrypts_content(&s)
}

/// Whether an `encryption.xml` encrypts more than fonts.
pub fn encrypts_content(xml: &str) -> bool {
    let Ok(doc) = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    ) else {
        // Unreadable: taken as locked, to be safe.
        return true;
    };
    doc.descendants()
        .filter(|n| n.tag_name().name() == "EncryptionMethod")
        .any(|n| {
            let alg = n.attribute("Algorithm").unwrap_or_default();
            !FONT_ONLY.contains(&alg)
        })
}

/// The URIs of the obfuscated fonts in an `encryption.xml`.
pub fn obfuscated(xml: &str) -> Vec<String> {
    let Ok(doc) = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    ) else {
        return Vec::new();
    };
    doc.descendants()
        .filter(|n| n.tag_name().name() == "CipherReference")
        .filter_map(|n| n.attribute("URI").map(String::from))
        .collect()
}

pub fn kindle_locked(path: &Path) -> bool {
    let Ok(mut f) = File::open(path) else {
        return false;
    };
    let mut head = [0u8; 86];
    if f.read_exact(&mut head).is_err() {
        return false;
    }
    let first = u32::from_be_bytes([head[78], head[79], head[80], head[81]]) as u64;
    let mut rec = [0u8; 16];
    if f.seek(SeekFrom::Start(first)).is_err() || f.read_exact(&mut rec).is_err() {
        return false;
    }
    // Record 0: PalmDOC header; its encryption type at 12.
    u16::from_be_bytes([rec[12], rec[13]]) != 0
}

pub fn pdf_locked(path: &Path) -> bool {
    // The trailer (or the cross-reference stream) names `/Encrypt`; it is
    // near the end, but an updated file may have it in an earlier
    // trailer too, so the whole file is looked at when small, else the
    // first and last megabytes.
    let Ok(mut f) = File::open(path) else {
        return false;
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let mut look = |at: u64, n: u64| -> bool {
        let mut buf = vec![0u8; n as usize];
        f.seek(SeekFrom::Start(at)).is_ok()
            && f.read_exact(&mut buf).is_ok()
            && buf.windows(8).any(|w| w == b"/Encrypt")
    };
    const MB: u64 = 1 << 20;
    if len <= 4 * MB {
        look(0, len)
    } else {
        look(len - MB, MB) || look(0, MB)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fonts_alone_are_not_drm() {
        let fonts = r#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#">
<enc:EncryptedData><enc:EncryptionMethod Algorithm="http://www.idpf.org/2008/embedding"/>
<enc:CipherData><enc:CipherReference URI="OEBPS/font.otf"/></enc:CipherData></enc:EncryptedData></encryption>"#;
        assert!(!encrypts_content(fonts));
        assert_eq!(obfuscated(fonts), vec!["OEBPS/font.otf"]);
        let locked = fonts.replace(
            "http://www.idpf.org/2008/embedding",
            "http://www.w3.org/2001/04/xmlenc#aes128-cbc",
        );
        assert!(encrypts_content(&locked));
    }

    #[test]
    fn pdf_with_encrypt_is_locked() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.pdf");
        std::fs::write(
            &p,
            b"%PDF-1.4\n1 0 obj<<>>endobj\ntrailer<</Root 1 0 R/Encrypt 2 0 R>>\n%%EOF",
        )
        .unwrap();
        assert!(pdf_locked(&p));
        std::fs::write(
            &p,
            b"%PDF-1.4\n1 0 obj<<>>endobj\ntrailer<</Root 1 0 R>>\n%%EOF",
        )
        .unwrap();
        assert!(!pdf_locked(&p));
    }
}
