//! Base64 as the program writes and reads it (the `base64` crate): pictures
//! for the window's pages, FB2's embedded images, Chromaprint fingerprints.

use base64::Engine;
use base64::engine::DecodePaddingMode;
use base64::engine::general_purpose::{
    GeneralPurpose, GeneralPurposeConfig, STANDARD, URL_SAFE_NO_PAD,
};

/// Standard alphabet, padded (`data:` addresses).
pub fn encode(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

/// URL alphabet without padding, as Chromaprint writes fingerprints.
pub fn encode_url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Standard alphabet in lines of `width` characters (FB2's binaries).
pub fn encode_lines(bytes: &[u8], width: usize) -> String {
    let flat = encode(bytes);
    let mut out = String::with_capacity(flat.len() + flat.len() / width.max(1) + 1);
    for (i, chunk) in flat.as_bytes().chunks(width.max(1)).enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(std::str::from_utf8(chunk).unwrap_or_default());
    }
    out
}

/// Reads base64 as files write it: either alphabet, padded or not, with
/// line breaks and spaces (or other stray characters) left out; what
/// cannot be read is empty.
pub fn decode(text: &str) -> Vec<u8> {
    const LENIENT: GeneralPurpose = GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        GeneralPurposeConfig::new()
            .with_decode_padding_mode(DecodePaddingMode::Indifferent)
            .with_decode_allow_trailing_bits(true),
    );
    let clean: String = text
        .chars()
        .filter_map(|c| match c {
            '-' => Some('+'),
            '_' => Some('/'),
            c if c.is_ascii_alphanumeric() || c == '+' || c == '/' => Some(c),
            _ => None,
        })
        .collect();
    LENIENT.decode(clean).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_ways() {
        for d in [&b""[..], b"a", b"ab", b"abc", b"\xff\xfe\x00 picture"] {
            assert_eq!(decode(&encode(d)), d);
            assert_eq!(decode(&encode_url(d)), d);
            assert_eq!(decode(&encode_lines(d, 4)), d);
        }
        assert_eq!(encode_url(&[0xfb, 0xff]), "-_8");
        assert_eq!(encode(b"ab"), "YWI=");
    }
}
