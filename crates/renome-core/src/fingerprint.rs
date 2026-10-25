//! Audio fingerprints (Chromaprint, as AcoustID expects them), from the
//! first two minutes of a song. Only the decoded audio is used; the file is
//! read, never changed.

use std::fs::File;
use std::path::Path;

use rusty_chromaprint::{Configuration, FingerprintCompressor, Fingerprinter};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

/// What AcoustID looks at: the start of the song.
const SECONDS: u64 = 120;

/// A song's fingerprint, compressed and encoded as AcoustID takes it, and
/// its length in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Print {
    pub fingerprint: String,
    pub duration: u32,
}

/// Base64 with the URL alphabet and no padding, as Chromaprint writes it.
fn base64url(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len() * 4 / 3 + 3);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

/// The fingerprint of the song at `path`; `None` when its audio cannot be
/// decoded (a format not supported, a damaged file) or is too short.
pub fn of(path: &Path) -> Option<Print> {
    let (print, duration) = raw(path)?;
    let config = Configuration::preset_test2();
    Some(Print {
        fingerprint: base64url(&FingerprintCompressor::from(&config).compress(&print)),
        duration,
    })
}

/// The fingerprint as numbers (uncompressed) and the length in seconds.
pub fn raw(path: &Path) -> Option<(Vec<u32>, u32)> {
    let file = File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .ok()?;
    let track = format.default_track(TrackType::Audio)?;
    let id = track.id;
    let Some(CodecParameters::Audio(params)) = &track.codec_params else {
        return None;
    };
    let length = match (track.num_frames, params.sample_rate) {
        (Some(frames), Some(rate)) if rate > 0 => Some(frames / u64::from(rate)),
        _ => None,
    };
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .ok()?;

    let config = Configuration::preset_test2();
    let mut printer = Fingerprinter::new(&config);
    let mut samples: Vec<i16> = Vec::new();
    let mut started = None;
    let mut frames = 0u64;
    while let Ok(Some(packet)) = format.next_packet() {
        if packet.track_id != id {
            continue;
        }
        let audio = match decoder.decode(&packet) {
            Ok(a) => a,
            // A damaged frame: skip it, as players do.
            Err(Error::DecodeError(_)) => continue,
            Err(_) => break,
        };
        let spec = audio.spec();
        let (rate, channels) = (spec.rate(), spec.channels().count() as u32);
        if started.is_none() {
            printer.start(rate, channels).ok()?;
            started = Some(rate);
        }
        audio.copy_to_vec_interleaved(&mut samples);
        printer.consume(&samples);
        frames += audio.frames() as u64;
        if frames >= SECONDS * u64::from(rate) {
            break;
        }
    }
    let rate = started?;
    printer.finish();
    let print = printer.fingerprint();
    if print.is_empty() {
        return None;
    }
    let duration = length
        .filter(|&s| s > 0)
        .unwrap_or(frames / u64::from(rate));
    Some((print.to_vec(), duration as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_like_chromaprint() {
        assert_eq!(base64url(b""), "");
        assert_eq!(base64url(b"f"), "Zg");
        assert_eq!(base64url(b"fo"), "Zm8");
        assert_eq!(base64url(b"foo"), "Zm9v");
        assert_eq!(base64url(&[0xfb, 0xff]), "-_8");
    }

    /// A WAV of a few tones, long enough to be fingerprinted.
    fn wav(path: &Path, secs: u32) {
        let rate = 11025u32;
        let n = rate * secs;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + n * 2).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes()); // PCM
        b.extend_from_slice(&1u16.to_le_bytes()); // mono
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * 2).to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(n * 2).to_le_bytes());
        for i in 0..n {
            let t = f64::from(i) / f64::from(rate);
            let f = [220.0, 330.0, 440.0, 550.0][(i / rate % 4) as usize];
            let s = (t * f * std::f64::consts::TAU).sin() * 8000.0;
            b.extend_from_slice(&(s as i16).to_le_bytes());
        }
        std::fs::write(path, b).unwrap();
    }

    #[test]
    fn fingerprints_a_song() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.wav");
        wav(&p, 20);
        let print = of(&p).unwrap();
        assert_eq!(print.duration, 20);
        // Compressed: algorithm 1 (test2) first.
        assert!(print.fingerprint.starts_with("AQ"));
        // The same audio, the same fingerprint.
        let q = d.path().join("b.wav");
        std::fs::copy(&p, &q).unwrap();
        assert_eq!(of(&q).unwrap(), print);
        // Not audio.
        std::fs::write(d.path().join("c.mp3"), "nothing").unwrap();
        assert!(of(&d.path().join("c.mp3")).is_none());
    }
}
