//! The readers e-books and comics are made for (`src/lib/readers.ts`, the
//! window's list, made into tables by the build): each screen, each model
//! with the formats it opens as it comes, and what follows from them for
//! the window and the command line alike: the page a book is printed to
//! PDF on, and whether the format a rule makes opens on its reader.

use serde::Serialize;
use serde_json::Value;

include!(concat!(env!("OUT_DIR"), "/screens.rs"));

/// The formats a reader may be sent (the comic and book rules' choices).
const FORMATS: &[&str] = &["epub", "kepub", "cbz", "pdf", "azw3", "fb2"];

/// A model of the list: (id, name, screen, formats).
type Model = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
);

fn model(id: &str) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.0 == id)
}

/// The screen a model has.
pub fn device_of(model_id: &str) -> Option<&'static str> {
    model(model_id).map(|m| m.2)
}

/// The reader a rule names, as its menu shows it: its model when that is
/// one of the screen's, else (rules made before models) the screen's
/// first model, else the screen itself (a custom size, the pages as they
/// are).
pub fn reader_of(device: &str, model_id: &str) -> String {
    if model(model_id).is_some_and(|m| m.2 == device) {
        return model_id.to_string();
    }
    MODELS
        .iter()
        .find(|m| m.2 == device)
        .map_or_else(|| device.to_string(), |m| m.0.to_string())
}

/// The formats a reader opens as it comes, best first; `None`: any (not
/// one of the models).
pub fn formats(device: &str, model_id: &str) -> Option<&'static [&'static str]> {
    model(&reader_of(device, model_id)).map(|m| m.3)
}

/// The page size (width, height in mm) of a book printed to PDF for a
/// reader model: its screen's; `None` (A5) for a model not in the list.
pub fn pdf_page(model_id: &str) -> Option<(f64, f64)> {
    let device = device_of(model_id)?;
    let (_, w, h, _, ppi) = SCREENS.iter().find(|s| s.0 == device)?;
    let mm = |px: u32| ((f64::from(px) / f64::from(*ppi)) * 254.0).round() / 10.0;
    Some((mm(*w), mm(*h)))
}

/// What a comic or book rule's reader is (the window's card shows it).
#[derive(Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Reader {
    /// The reader as the comic rule's menu shows it (see [`reader_of`]).
    pub reader: String,
    /// The book rule's: the chosen model's screen.
    pub device: Option<&'static str>,
    /// The reader opens only some formats (others take a reading app).
    pub limited: bool,
    /// The formats it does not open as it comes, unless the rule says any
    /// reading program will do (marked in the rule's menu).
    pub marked: Vec<&'static str>,
    /// The format the rule makes, when the reader does not open it.
    pub not_opens: Option<NotOpens>,
}

/// A format the reader does not open as it comes.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NotOpens {
    /// The model: its id and name (an i18n key when it starts with `@`).
    pub model: &'static str,
    pub name: &'static str,
    pub format: String,
    /// The first format it opens.
    pub best: &'static str,
}

/// The reader a comic rule's e-book (`ebook`) or a book rule's conversion
/// (`convertTo`) is made for, and whether it opens that format.
pub fn reader(rule: &Value) -> Reader {
    use crate::catalog::set_value;
    let text = |k: &str| set_value(rule, k).as_str().unwrap_or("").to_string();
    let any = set_value(rule, "otherReaderApps")
        .as_bool()
        .unwrap_or(false);
    let model_id = text("model");
    let (format, device, book) = match rule.get("type").and_then(Value::as_str) {
        Some("comic") => (text("ebook"), Some(text("device")), None),
        Some("book") => {
            let d = device_of(&model_id);
            (text("convertTo"), d.map(str::to_string), d)
        }
        _ => return Reader::default(),
    };
    let Some(device) = device else {
        return Reader::default();
    };
    let reader = reader_of(&device, &model_id);
    let opens = formats(&device, &model_id);
    let marked: Vec<&'static str> = match opens {
        Some(list) if !any => FORMATS
            .iter()
            .copied()
            .filter(|f| !list.contains(f))
            .collect(),
        _ => Vec::new(),
    };
    let not_opens = (!format.is_empty() && marked.contains(&format.as_str()))
        .then(|| model(&reader))
        .flatten()
        .map(|m| NotOpens {
            model: m.0,
            name: m.1,
            format: format.clone(),
            best: m.3.first().copied().unwrap_or(""),
        });
    Reader {
        reader,
        device: book,
        limited: opens.is_some(),
        marked,
        not_opens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn every_model_has_its_screen_and_formats() {
        for (id, _, device, formats) in MODELS {
            assert!(SCREENS.iter().any(|s| s.0 == *device), "{id}: {device}");
            assert!(!formats.is_empty(), "{id}");
        }
        let (w, h) = pdf_page(MODELS[0].0).unwrap();
        assert!(w > 50.0 && h > w);
        assert_eq!(pdf_page("no-such-reader"), None);
    }

    #[test]
    fn a_format_the_reader_does_not_open_is_told() {
        // A Kindle opens AZW3 and PDF, not EPUB.
        let r = reader(
            &json!({ "type": "comic", "ebook": "epub", "device": "kindle-pw2024", "model": "kindle-pw-2024" }),
        );
        assert_eq!(r.reader, "kindle-pw-2024");
        let n = r.not_opens.unwrap();
        assert_eq!((n.format.as_str(), n.best), ("epub", "azw3"));
        // Any reading program will do: nothing marked.
        let r = reader(
            &json!({ "type": "comic", "ebook": "epub", "device": "kindle-pw2024", "otherReaderApps": true }),
        );
        assert!(r.not_opens.is_none() && r.marked.is_empty());
        // A rule made before models: the screen's first model.
        let r = reader(&json!({ "type": "comic", "ebook": "azw3", "device": "kindle-pw2024" }));
        assert!(r.reader.starts_with("kindle-") && r.not_opens.is_none());
        // A custom size opens anything.
        let r = reader(&json!({ "type": "comic", "ebook": "epub", "device": "custom" }));
        assert!(!r.limited && r.not_opens.is_none());
        // A book converted for a Kobo: KEPUB is fine, AZW3 is not.
        let r = reader(&json!({ "type": "book", "convertTo": "azw3", "model": "kobo-sage" }));
        assert_eq!(r.device, Some("kobo-sage"));
        assert_eq!(r.not_opens.unwrap().best, "kepub");
        // No model: any reader.
        assert_eq!(
            reader(&json!({ "type": "book", "convertTo": "azw3" })),
            Reader::default()
        );
    }
}
