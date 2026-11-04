//! Barcodes printed on a comic: the UPC (with its 5-digit add-on, which
//! tells the issue and cover) or the ISBN/EAN, found in the pictures of
//! its first and last pages, where covers carry them. Pictures are made
//! smaller first when large, and read in gray.

use std::collections::HashSet;
use std::path::Path;

use rxing::{BarcodeFormat, DecodeHints, RXingResultMetadataType, RXingResultMetadataValue};

/// Pages read at the start and the end of a comic.
const HEAD: usize = 1;
const TAIL: usize = 2;
/// The long side pictures are brought down to before they are read: a
/// barcode on a scanned cover is still wide enough there.
const LONG_SIDE: u32 = 2000;

/// What is wanted from a code: digits with a valid check digit.
fn check_digit_ok(code: &str) -> bool {
    let d: Vec<u32> = code.chars().filter_map(|c| c.to_digit(10)).collect();
    if d.len() != code.len() || !matches!(d.len(), 8 | 12 | 13) {
        return false;
    }
    let (body, check) = d.split_at(d.len() - 1);
    // From the right, weights 3, 1, 3…
    let sum: u32 = body
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &n)| if i % 2 == 0 { n * 3 } else { n })
        .sum();
    (10 - sum % 10) % 10 == check[0]
}

/// The barcode in a picture (JPEG, PNG): its digits, with the add-on after
/// them when printed.
pub fn in_picture(data: &[u8]) -> Option<String> {
    let img = image::load_from_memory(data).ok()?;
    let img = if img.width().max(img.height()) > LONG_SIDE {
        img.resize(LONG_SIDE, LONG_SIDE, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let gray = img.into_luma8();
    let (w, h) = gray.dimensions();
    let mut hints = DecodeHints {
        PossibleFormats: Some(HashSet::from([
            BarcodeFormat::UPC_A,
            BarcodeFormat::EAN_13,
            BarcodeFormat::UPC_E,
        ])),
        TryHarder: Some(true),
        ..Default::default()
    };
    let found =
        rxing::helpers::detect_in_luma_with_hints(gray.into_raw(), w, h, None, &mut hints).ok()?;
    let main = found.getText().trim().to_string();
    // Short EAN-8 codes are read in lines that are no barcode: left out.
    if !check_digit_ok(&main)
        || (main.len() == 8 && *found.getBarcodeFormat() != BarcodeFormat::UPC_E)
    {
        return None;
    }
    let addon = match found
        .getRXingResultMetadata()
        .get(&RXingResultMetadataType::UPC_EAN_EXTENSION)
    {
        Some(RXingResultMetadataValue::UpcEanExtension(a)) => a.trim().to_string(),
        _ => String::new(),
    };
    Some(format!("{main}{addon}"))
}

/// The barcode printed on a comic, read in its first page and its last
/// ones; `None` when there is none to read (or it is a PDF).
pub fn of_comic(path: &Path) -> Option<String> {
    let pages = crate::convert::end_pages(path, HEAD, TAIL);
    // The back cover first: that is where barcodes usually are.
    pages.iter().rev().find_map(|p| in_picture(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real comics listed (one path a line) in the file `RENOME_COMICS`
    /// names: the barcodes found, and how long it took.
    #[test]
    fn real_comics() {
        let Ok(list) = std::env::var("RENOME_COMICS") else {
            return;
        };
        let paths = std::fs::read_to_string(list).unwrap();
        let (mut found, mut total) = (0, 0);
        let start = std::time::Instant::now();
        for p in paths.lines().filter(|l| !l.trim().is_empty()) {
            total += 1;
            let t = std::time::Instant::now();
            let code = of_comic(Path::new(p));
            found += usize::from(code.is_some());
            println!("{:?} {:>5}ms {p}", code, t.elapsed().as_millis());
        }
        println!("{found}/{total} in {:?}", start.elapsed());
    }

    #[test]
    fn check_digits() {
        assert!(check_digit_ok("036000291452"));
        assert!(check_digit_ok("9780306406157"));
        assert!(!check_digit_ok("9780306406158"));
        assert!(!check_digit_ok("12345"));
    }

    /// An EAN-13 drawn by hand: its modules as black and white columns.
    fn ean13(code: &str) -> Vec<u8> {
        const L: [&str; 10] = [
            "0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011",
            "0110111", "0001011",
        ];
        const G: [&str; 10] = [
            "0100111", "0110011", "0011011", "0100001", "0011101", "0111001", "0000101", "0010001",
            "0001001", "0010111",
        ];
        const PARITY: [&str; 10] = [
            "LLLLLL", "LLGLGG", "LLGGLG", "LLGGGL", "LGLLGG", "LGGLLG", "LGGGLL", "LGLGLG",
            "LGLGGL", "LGGLGL",
        ];
        let d: Vec<usize> = code
            .chars()
            .map(|c| c.to_digit(10).unwrap() as usize)
            .collect();
        let mut bits = String::from("101");
        for (i, &n) in d[1..7].iter().enumerate() {
            let set = if PARITY[d[0]].as_bytes()[i] == b'L' {
                L
            } else {
                G
            };
            bits += set[n];
        }
        bits += "01010";
        for &n in &d[7..] {
            bits += &L[n]
                .chars()
                .map(|c| if c == '0' { '1' } else { '0' })
                .collect::<String>();
        }
        bits += "101";
        bits.into_bytes()
    }

    #[test]
    fn a_barcode_is_read_from_a_picture() {
        let code = "9780306406157";
        let bits = ean13(code);
        let (module, quiet) = (3u32, 30u32);
        let w = bits.len() as u32 * module + 2 * quiet;
        let h = 120u32;
        let img = image::GrayImage::from_fn(w, h, |x, _| {
            let i = x.checked_sub(quiet).map(|x| (x / module) as usize);
            match i.and_then(|i| bits.get(i)) {
                Some(b'1') => image::Luma([0]),
                _ => image::Luma([255]),
            }
        });
        let mut png = Vec::new();
        image::DynamicImage::ImageLuma8(img)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        assert_eq!(in_picture(&png).as_deref(), Some(code));
        let mut blank = Vec::new();
        image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(200, 100, image::Luma([255])))
            .write_to(
                &mut std::io::Cursor::new(&mut blank),
                image::ImageFormat::Png,
            )
            .unwrap();
        assert_eq!(in_picture(&blank), None);
    }
}
