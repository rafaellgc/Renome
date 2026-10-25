//! Writing a song's tags (and front cover) from its MusicBrainz release.
//! Only the fields below are touched, unless the other tags are cleaned
//! away too (site names, ripper notes); the audio stays as it is. The file
//! is written as a copy beside it and then put in its place, so a crash
//! never leaves it half written. What was changed is returned, so undo can
//! put it back.

use std::fs::{self, File, OpenOptions};
use std::io::{Seek, SeekFrom};
use std::path::{Path, PathBuf};

use lofty::config::{ParseOptions, WriteOptions};
use lofty::file::AudioFile;
use lofty::flac::FlacFile;
use lofty::id3::v2::Id3v2Tag;
use lofty::mp4::{Ilst, Mp4File};
use lofty::mpeg::MpegFile;
use lofty::ogg::tag::VorbisComments;
use lofty::ogg::{OggPictureStorage, OpusFile, VorbisFile};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::tag::{ItemKey, ItemValue, MergeTag, SplitTag, Tag, TagExt, TagItem};
use serde::{Deserialize, Serialize};

use crate::musicbrainz::{Credit, Release, SongMatch, plain};

/// A field this module writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Field {
    Title,
    Artist,
    AlbumArtist,
    Album,
    Date,
    Track,
    TrackTotal,
    Disc,
    DiscTotal,
    ReleaseId,
    RecordingId,
    TrackId,
    ReleaseGroupId,
    ArtistId,
    AlbumArtistId,
    OriginalDate,
    Label,
    CatalogNumber,
    Barcode,
    ReleaseCountry,
    ReleaseType,
    Media,
    DiscSubtitle,
    Isrc,
    Genre,
    Language,
    Script,
    Composer,
    Lyricist,
    Writer,
    Producer,
    Arranger,
    Mixer,
    Engineer,
    ArtistSort,
    AlbumArtistSort,
    WorkId,
    /// Each credited artist on its own (`os vagalumes`, `Rui`).
    Artists,
    AlbumArtists,
}

/// Every field this module writes.
const ALL: &[Field] = &[
    Field::Title,
    Field::Artist,
    Field::AlbumArtist,
    Field::Album,
    Field::Date,
    Field::Track,
    Field::TrackTotal,
    Field::Disc,
    Field::DiscTotal,
    Field::ReleaseId,
    Field::RecordingId,
    Field::TrackId,
    Field::ReleaseGroupId,
    Field::ArtistId,
    Field::AlbumArtistId,
    Field::OriginalDate,
    Field::Label,
    Field::CatalogNumber,
    Field::Barcode,
    Field::ReleaseCountry,
    Field::ReleaseType,
    Field::Media,
    Field::DiscSubtitle,
    Field::Isrc,
    Field::Genre,
    Field::Language,
    Field::Script,
    Field::Composer,
    Field::Lyricist,
    Field::Writer,
    Field::Producer,
    Field::Arranger,
    Field::Mixer,
    Field::Engineer,
    Field::ArtistSort,
    Field::AlbumArtistSort,
    Field::WorkId,
    Field::Artists,
    Field::AlbumArtists,
];

/// A tag item cleaning would keep: one this module writes, or one of
/// [`KEEP`]. Anything else in a song is what cleaning takes away.
pub fn kept(key: ItemKey) -> bool {
    KEEP.contains(&key) || ALL.iter().any(|f| f.key() == key)
}

impl Field {
    fn key(self) -> ItemKey {
        match self {
            Field::Title => ItemKey::TrackTitle,
            Field::Artist => ItemKey::TrackArtist,
            Field::AlbumArtist => ItemKey::AlbumArtist,
            Field::Album => ItemKey::AlbumTitle,
            Field::Date => ItemKey::RecordingDate,
            Field::Track => ItemKey::TrackNumber,
            Field::TrackTotal => ItemKey::TrackTotal,
            Field::Disc => ItemKey::DiscNumber,
            Field::DiscTotal => ItemKey::DiscTotal,
            Field::ReleaseId => ItemKey::MusicBrainzReleaseId,
            Field::RecordingId => ItemKey::MusicBrainzRecordingId,
            Field::TrackId => ItemKey::MusicBrainzTrackId,
            Field::ReleaseGroupId => ItemKey::MusicBrainzReleaseGroupId,
            Field::ArtistId => ItemKey::MusicBrainzArtistId,
            Field::AlbumArtistId => ItemKey::MusicBrainzReleaseArtistId,
            Field::OriginalDate => ItemKey::OriginalReleaseDate,
            Field::Label => ItemKey::Label,
            Field::CatalogNumber => ItemKey::CatalogNumber,
            Field::Barcode => ItemKey::Barcode,
            Field::ReleaseCountry => ItemKey::ReleaseCountry,
            Field::ReleaseType => ItemKey::MusicBrainzReleaseType,
            Field::Media => ItemKey::OriginalMediaType,
            Field::DiscSubtitle => ItemKey::SetSubtitle,
            Field::Isrc => ItemKey::Isrc,
            Field::Genre => ItemKey::Genre,
            Field::Language => ItemKey::Language,
            Field::Script => ItemKey::Script,
            Field::Composer => ItemKey::Composer,
            Field::Lyricist => ItemKey::Lyricist,
            Field::Writer => ItemKey::Writer,
            Field::Producer => ItemKey::Producer,
            Field::Arranger => ItemKey::Arranger,
            Field::Mixer => ItemKey::MixEngineer,
            Field::Engineer => ItemKey::Engineer,
            Field::ArtistSort => ItemKey::TrackArtistSortOrder,
            Field::AlbumArtistSort => ItemKey::AlbumArtistSortOrder,
            Field::WorkId => ItemKey::MusicBrainzWorkId,
            Field::Artists => ItemKey::TrackArtists,
            Field::AlbumArtists => ItemKey::AlbumArtists,
        }
    }
}

/// What cleaning keeps: what MusicBrainz does not give (lyrics, loudness,
/// tempo), genre and composer when it does not have them, and the encoder
/// (in Vorbis comments, a fixed part of the format).
const KEEP: &[ItemKey] = &[
    ItemKey::EncoderSoftware,
    ItemKey::Lyrics,
    ItemKey::UnsyncLyrics,
    ItemKey::ReplayGainTrackGain,
    ItemKey::ReplayGainTrackPeak,
    ItemKey::ReplayGainAlbumGain,
    ItemKey::ReplayGainAlbumPeak,
    ItemKey::Bpm,
    ItemKey::IntegerBpm,
    ItemKey::Genre,
    ItemKey::Composer,
    ItemKey::Lyricist,
];

/// Vorbis comments cleaning keeps beyond those: a cue sheet of the whole
/// disc in one file.
const KEEP_RAW: &[&str] = &["CUESHEET"];

/// Each field and its values (several for multi-artist ids).
pub type Fields = Vec<(Field, Vec<String>)>;

/// The fields to write for a song matched to its release; what the release
/// does not say is left as it is.
pub fn fields_of(m: &SongMatch) -> Fields {
    let one = |s: &str| (!s.is_empty()).then(|| vec![s.to_string()]);
    let date = if m.date.is_empty() {
        m.year.map(|y| y.to_string()).unwrap_or_default()
    } else {
        m.date.clone()
    };
    let fields = [
        (Field::Title, one(&m.title)),
        (Field::Artist, one(&m.artist)),
        (Field::AlbumArtist, one(&m.album_artist)),
        (Field::Album, one(&m.album)),
        (Field::Date, one(&date)),
        (Field::Track, Some(vec![m.track.to_string()])),
        (Field::TrackTotal, Some(vec![m.track_total.to_string()])),
        (Field::Disc, Some(vec![m.disc.to_string()])),
        (Field::DiscTotal, Some(vec![m.disc_total.to_string()])),
        (Field::ReleaseId, one(&m.release)),
        (Field::RecordingId, one(&m.recording)),
        (Field::TrackId, one(&m.track_id)),
        (Field::ReleaseGroupId, one(&m.release_group)),
        (
            Field::ArtistId,
            (!m.artist_ids.is_empty()).then(|| m.artist_ids.clone()),
        ),
        (
            Field::AlbumArtistId,
            (!m.album_artist_ids.is_empty()).then(|| m.album_artist_ids.clone()),
        ),
    ];
    fields
        .into_iter()
        .filter_map(|(f, v)| Some((f, v?)))
        .collect()
}

/// Values without repeats, in order.
fn unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for v in values {
        let v = v.trim().to_string();
        if !v.is_empty() && !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

/// An artist credit's sort names (`Moths, The`), joined as credited.
fn sort_names(c: &[Credit]) -> String {
    c.iter()
        .map(|a| {
            let sort = a
                .artist
                .as_ref()
                .map(|x| x.sort_name.as_str())
                .unwrap_or("");
            format!(
                "{}{}",
                if sort.is_empty() { &a.name } else { sort },
                a.joinphrase
            )
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// `dance-rock` → `Dance-rock`.
fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// What a release looked up in full (labels, genres, relationships) adds
/// for one of its tracks: original date, label, catalog number, barcode,
/// country, type, medium, ISRCs, genres, language, script, the work's
/// composers and lyricists, the recording's producers, arrangers, mixers
/// and engineers, sort names and the work's id.
pub fn details(r: &Release, track_id: &str) -> Fields {
    let Some((medium, track)) = r
        .media
        .iter()
        .flat_map(|m| m.tracks.iter().map(move |t| (m, t)))
        .find(|(_, t)| t.id == track_id)
    else {
        return Vec::new();
    };
    let group = r.release_group.clone().unwrap_or_default();
    let recording = track.recording.clone().unwrap_or_default();
    let one = |s: Option<&str>| s.filter(|s| !s.is_empty()).map(|s| vec![s.to_string()]);
    let people = |rels: &[crate::musicbrainz::Relation], kinds: &[&str]| {
        unique(
            rels.iter()
                .filter(|x| kinds.contains(&x.kind.as_str()))
                .filter_map(|x| x.artist.as_ref().map(|a| plain(&a.name))),
        )
    };
    let works: Vec<&crate::musicbrainz::Work> = recording
        .relations
        .iter()
        .filter(|x| x.kind == "performance")
        .filter_map(|x| x.work.as_ref())
        .collect();
    let in_works = |kinds: &[&str]| unique(works.iter().flat_map(|w| people(&w.relations, kinds)));
    // The recording's own genres, else the release's, else the album's.
    let mut genres = [&recording.genres, &r.genres, &group.genres]
        .into_iter()
        .find(|g| !g.is_empty())
        .cloned()
        .unwrap_or_default();
    genres.sort_by_key(|g| std::cmp::Reverse(g.count));
    let release_type = unique(
        group
            .primary_type
            .iter()
            .chain(&group.secondary_types)
            .map(|t| t.to_lowercase()),
    );
    let text = r.text_representation.clone().unwrap_or_default();
    let fields = [
        (
            Field::OriginalDate,
            one(group.first_release_date.as_deref()),
        ),
        (
            Field::Label,
            Some(unique(
                r.label_info
                    .iter()
                    .filter_map(|l| l.label.as_ref().map(|n| plain(&n.name))),
            )),
        ),
        (
            Field::CatalogNumber,
            Some(unique(
                r.label_info
                    .iter()
                    .filter_map(|l| l.catalog_number.clone())
                    .filter(|c| c != "[none]"),
            )),
        ),
        (Field::Barcode, one(r.barcode.as_deref())),
        (Field::ReleaseCountry, one(r.country.as_deref())),
        (Field::ReleaseType, Some(release_type)),
        (Field::Media, one(medium.format.as_deref())),
        (Field::DiscSubtitle, one(Some(medium.title.as_str()))),
        (Field::Isrc, Some(unique(recording.isrcs.clone()))),
        (
            Field::Genre,
            Some(unique(genres.iter().take(3).map(|g| capitalized(&g.name)))),
        ),
        // `zxx`: no lyrics.
        (
            Field::Language,
            one(text.language.as_deref().filter(|l| *l != "zxx")),
        ),
        (Field::Script, one(text.script.as_deref())),
        (Field::Composer, Some(in_works(&["composer"]))),
        (Field::Lyricist, Some(in_works(&["lyricist"]))),
        (Field::Writer, Some(in_works(&["writer"]))),
        (
            Field::Producer,
            Some(people(&recording.relations, &["producer"])),
        ),
        (
            Field::Arranger,
            Some(people(&recording.relations, &["arranger"])),
        ),
        (Field::Mixer, Some(people(&recording.relations, &["mix"]))),
        (
            Field::Engineer,
            Some(people(&recording.relations, &["engineer", "recording"])),
        ),
        (
            Field::ArtistSort,
            one(Some(
                plain(&sort_names(if track.artist_credit.is_empty() {
                    &r.artist_credit
                } else {
                    &track.artist_credit
                }))
                .as_str(),
            )),
        ),
        (
            Field::AlbumArtistSort,
            one(Some(plain(&sort_names(&r.artist_credit)).as_str())),
        ),
        (
            Field::WorkId,
            Some(unique(works.iter().map(|w| w.id.clone()))),
        ),
        (
            Field::Artists,
            Some(unique(
                (if track.artist_credit.is_empty() {
                    &r.artist_credit
                } else {
                    &track.artist_credit
                })
                .iter()
                .map(|a| plain(&a.name)),
            )),
        ),
        (
            Field::AlbumArtists,
            Some(unique(r.artist_credit.iter().map(|a| plain(&a.name)))),
        ),
    ];
    fields
        .into_iter()
        .filter_map(|(f, v)| Some((f, v?)).filter(|(_, v)| !v.is_empty()))
        .collect()
}

/// A picture taken out of a file, kept in the history's folder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedPicture {
    pub kind: u8,
    pub mime: String,
    pub description: String,
    /// File name in the folder the backup's pictures are kept in.
    pub file: String,
}

/// A tag item cleaning took out, by its key in the file's tag format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedItem {
    pub key: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub locator: bool,
    /// Binary data, kept as a file like the pictures.
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub lang: String,
    #[serde(default)]
    pub description: String,
}

/// What a write changed in a file, to put it back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Backup {
    pub path: PathBuf,
    pub fields: Fields,
    /// The pictures the file had, when the cover replaced them.
    pub pictures: Option<Vec<SavedPicture>>,
    /// The other tags were cleaned away.
    #[serde(default)]
    pub cleaned: bool,
    #[serde(default)]
    pub removed: Vec<SavedItem>,
    /// Vorbis comments cleaning took out that no generic field stands for.
    #[serde(default)]
    pub raw: Vec<(String, String)>,
}

/// The formats whose tags can be written.
pub fn writable(path: &Path) -> bool {
    matches!(
        ext_of(path).as_str(),
        "mp3" | "flac" | "ogg" | "oga" | "opus" | "m4a"
    )
}

fn ext_of(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn mime_of(data: &[u8]) -> MimeType {
    if data.starts_with(b"\x89PNG") {
        MimeType::Png
    } else {
        MimeType::Jpeg
    }
}

/// What an edit sees beyond the generic tag.
#[derive(Default)]
struct Rest {
    /// Vorbis comments no generic field stands for (when rebuilding).
    raw: Vec<(String, String)>,
    /// Rebuilding drops other things too (unknown ID3v2 frames, MP4
    /// atoms, an MP3's ID3v1 or APE tag).
    more: bool,
}

/// Builds the Vorbis comments of a rebuild: the generic tag's, then the
/// raw ones.
fn vorbis(tag: Tag, raw: Vec<(String, String)>) -> VorbisComments {
    let mut vc = VorbisComments::from(tag);
    for (k, v) in raw {
        vc.push(k, v);
    }
    vc
}

/// Gives `edit` the file's main tag as a generic [`Tag`], with all its
/// pictures, and writes it back when `edit` says it changed something:
/// merged into what the generic tag does not hold, or, with `rebuild`, in
/// its place (only `Rest::raw` is kept of that). Works on a copy that then
/// takes the file's place.
fn edit(
    path: &Path,
    rebuild: bool,
    edit: impl FnOnce(&mut Tag, &mut Rest) -> bool,
) -> Result<bool, String> {
    let ext = ext_of(path);
    if !writable(path) {
        return Err("tagsUnsupported".into());
    }
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp = path.with_file_name(format!(".{name}.renome-tags"));
    fs::copy(path, &tmp).map_err(|e| e.to_string())?;
    let written = (|| -> Result<bool, String> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&tmp)
            .map_err(|e| e.to_string())?;
        let parse = ParseOptions::new();
        let mut write = WriteOptions::new();
        let err = |e: lofty::error::FileParseError| e.to_string();
        let mut rest = Rest::default();
        let changed = match ext.as_str() {
            "mp3" => {
                let mut a = MpegFile::read_from(&mut file, parse).map_err(err)?;
                let (r, mut tag) = a.remove_id3v2().unwrap_or_default().split_tag();
                if rebuild {
                    rest.more = !r.is_empty() || a.id3v1().is_some() || a.ape().is_some();
                }
                let changed = edit(&mut tag, &mut rest);
                if rebuild {
                    a.set_id3v2(Id3v2Tag::from(tag));
                    a.remove_id3v1();
                    a.remove_ape();
                } else {
                    a.set_id3v2(r.merge_tag(tag));
                }
                // ID3v2.3: what Windows and older players read.
                write = write.use_id3v23(true);
                changed.then(|| save(&a, &mut file, write)).transpose()?
            }
            "flac" => {
                let mut a = FlacFile::read_from(&mut file, parse).map_err(err)?;
                let (r, mut tag) = a.remove_vorbis_comments().unwrap_or_default().split_tag();
                // FLAC keeps pictures in blocks of their own.
                for (p, _) in a.remove_pictures() {
                    tag.push_picture(p);
                }
                if rebuild {
                    rest.raw = r
                        .items()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect();
                }
                let changed = edit(&mut tag, &mut rest);
                let pictures: Vec<Picture> = (0..tag.picture_count())
                    .map(|_| tag.remove_picture(0))
                    .collect();
                if rebuild {
                    a.set_vorbis_comments(vorbis(tag, std::mem::take(&mut rest.raw)));
                } else {
                    a.set_vorbis_comments(r.merge_tag(tag));
                }
                for p in pictures {
                    a.insert_picture(p, None).map_err(|e| e.to_string())?;
                }
                changed.then(|| save(&a, &mut file, write)).transpose()?
            }
            "ogg" | "oga" | "opus" => {
                // Both keep the comments (and pictures) in one tag.
                let (r, mut tag, mut vorbis_file, mut opus_file) = if ext == "opus" {
                    let mut a = OpusFile::read_from(&mut file, parse).map_err(err)?;
                    let (r, t) = std::mem::take(a.vorbis_comments_mut()).split_tag();
                    (r, t, None, Some(a))
                } else {
                    let mut a = VorbisFile::read_from(&mut file, parse).map_err(err)?;
                    let (r, t) = std::mem::take(a.vorbis_comments_mut()).split_tag();
                    (r, t, Some(a), None)
                };
                if rebuild {
                    rest.raw = r
                        .items()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect();
                }
                let changed = edit(&mut tag, &mut rest);
                let vc = if rebuild {
                    vorbis(tag, std::mem::take(&mut rest.raw))
                } else {
                    r.merge_tag(tag)
                };
                match (&mut vorbis_file, &mut opus_file) {
                    (Some(a), _) => {
                        a.set_vorbis_comments(vc);
                        changed.then(|| save(a, &mut file, write)).transpose()?
                    }
                    (_, Some(a)) => {
                        a.set_vorbis_comments(vc);
                        changed.then(|| save(a, &mut file, write)).transpose()?
                    }
                    _ => None,
                }
            }
            _ => {
                let mut a = Mp4File::read_from(&mut file, parse).map_err(err)?;
                let (r, mut tag) = a.remove_ilst().unwrap_or_default().split_tag();
                if rebuild {
                    rest.more = !r.is_empty();
                }
                let changed = edit(&mut tag, &mut rest);
                a.set_ilst(if rebuild {
                    Ilst::from(tag)
                } else {
                    r.merge_tag(tag)
                });
                changed.then(|| save(&a, &mut file, write)).transpose()?
            }
        };
        if changed.is_some() {
            file.sync_all().map_err(|e| e.to_string())?;
        }
        Ok(changed.is_some())
    })();
    match written {
        Ok(true) => fs::rename(&tmp, path).map(|()| true).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            e.to_string()
        }),
        other => {
            let _ = fs::remove_file(&tmp);
            other
        }
    }
}

fn save(a: &impl AudioFile, file: &mut File, write: WriteOptions) -> Result<(), String> {
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    a.save_to(file, write).map_err(|e| e.to_string())
}

/// The values a tag has for a field.
fn values(tag: &Tag, f: Field) -> Vec<String> {
    tag.get_strings(f.key()).map(str::to_string).collect()
}

fn set(tag: &mut Tag, f: Field, values: &[String]) {
    tag.remove_key(f.key());
    for v in values {
        tag.push(TagItem::new(f.key(), ItemValue::Text(v.clone())));
    }
}

/// Keeps `data` in `store`, named by its content (a cover shared by an
/// album's songs is kept once); returns the name.
fn keep(data: &[u8], store: &Path) -> Result<String, String> {
    fs::create_dir_all(store).map_err(|e| e.to_string())?;
    let file = format!("{:016x}.img", xxhash_rust::xxh3::xxh3_64(data));
    let at = store.join(&file);
    if !at.exists() {
        fs::write(&at, data).map_err(|e| e.to_string())?;
    }
    Ok(file)
}

/// Writes the fields (and, with `cover`, the front cover in place of the
/// pictures the file has) into the song at `path`; with `clean`, the other
/// tags go away (but for [`KEEP`]). What is taken out is saved, pictures
/// in `store`. Returns what to put back, or `None` when the file already
/// had all that (it is then not written).
pub fn write(
    path: &Path,
    fields: &Fields,
    cover: Option<&[u8]>,
    clean: bool,
    store: &Path,
) -> Result<Option<Backup>, String> {
    let mut backup = Backup {
        path: path.to_path_buf(),
        fields: Vec::new(),
        pictures: None,
        cleaned: clean,
        removed: Vec::new(),
        raw: Vec::new(),
    };
    let mut saved: Result<(), String> = Ok(());
    let changed = edit(path, clean, |tag, rest| {
        let mut changed = false;
        if clean {
            let tag_type = tag.tag_type();
            let ours = |k: &ItemKey| kept(*k);
            for item in tag.items().filter(|i| !ours(&i.key())) {
                let Some(key) = item.key().map_key(tag_type) else {
                    continue;
                };
                let mut s = SavedItem {
                    key: key.to_string(),
                    text: None,
                    locator: false,
                    file: None,
                    lang: String::from_utf8_lossy(item.lang()).into_owned(),
                    description: item.description().to_string(),
                };
                match item.value() {
                    ItemValue::Text(t) => s.text = Some(t.clone()),
                    ItemValue::Locator(t) => {
                        s.text = Some(t.clone());
                        s.locator = true;
                    }
                    ItemValue::Binary(b) => match keep(b, store) {
                        Ok(f) => s.file = Some(f),
                        Err(e) => saved = Err(e),
                    },
                }
                backup.removed.push(s);
            }
            tag.retain(|i| ours(&i.key()));
            let (kept, out): (Vec<_>, Vec<_>) = std::mem::take(&mut rest.raw)
                .into_iter()
                .partition(|(k, _)| KEEP_RAW.iter().any(|x| x.eq_ignore_ascii_case(k)));
            rest.raw = kept;
            changed = !backup.removed.is_empty() || !out.is_empty() || rest.more;
            backup.raw = out;
        }
        for (f, new) in fields {
            let old = values(tag, *f);
            if old != *new {
                changed = true;
                backup.fields.push((*f, old));
                set(tag, *f, new);
            }
        }
        if let Some(data) = cover {
            // MP4 does not keep the picture's type: the data tells.
            let same = tag.pictures().len() == 1 && tag.pictures()[0].data() == data;
            if !same {
                changed = true;
                let kept: Result<Vec<SavedPicture>, String> = tag
                    .pictures()
                    .iter()
                    .map(|p| {
                        Ok(SavedPicture {
                            kind: p.pic_type().as_u8(),
                            mime: p
                                .mime_type()
                                .map(|m| m.as_str().to_string())
                                .unwrap_or_default(),
                            description: p.description().unwrap_or_default().to_string(),
                            file: keep(p.data(), store)?,
                        })
                    })
                    .collect();
                match kept {
                    Ok(p) => backup.pictures = Some(p),
                    Err(e) => saved = Err(e),
                }
                while tag.picture_count() > 0 {
                    tag.remove_picture(0);
                }
                tag.push_picture(
                    Picture::unchecked(data.to_vec())
                        .pic_type(PictureType::CoverFront)
                        .mime_type(mime_of(data))
                        .build(),
                );
            }
        }
        // Without what it takes out saved, the file is not touched.
        changed && saved.is_ok()
    })?;
    saved?;
    Ok(changed.then_some(backup))
}

/// Puts back what [`write`] changed (pictures and data from `store`).
pub fn restore(b: &Backup, store: &Path) -> Result<(), String> {
    let read = |f: &str| fs::read(store.join(f)).map_err(|e| e.to_string());
    let pictures = match &b.pictures {
        Some(saved) => Some(
            saved
                .iter()
                .map(|s| {
                    let mut p =
                        Picture::unchecked(read(&s.file)?).pic_type(PictureType::from_u8(s.kind));
                    if !s.mime.is_empty() {
                        p = p.mime_type(MimeType::from_str(&s.mime));
                    }
                    if !s.description.is_empty() {
                        p = p.description(s.description.clone());
                    }
                    Ok(p.build())
                })
                .collect::<Result<Vec<_>, String>>()?,
        ),
        None => None,
    };
    let mut removed = Vec::new();
    for s in &b.removed {
        let value = match (&s.text, &s.file) {
            (Some(t), _) if s.locator => ItemValue::Locator(t.clone()),
            (Some(t), _) => ItemValue::Text(t.clone()),
            (None, Some(f)) => ItemValue::Binary(read(f)?),
            (None, None) => continue,
        };
        removed.push((s, value));
    }
    edit(&b.path, b.cleaned, |tag, rest| {
        let tag_type = tag.tag_type();
        for (s, value) in removed {
            let Some(key) = ItemKey::from_key(tag_type, &s.key) else {
                continue;
            };
            let mut item = TagItem::new(key, value);
            if let Ok(lang) = <[u8; 3]>::try_from(s.lang.as_bytes()) {
                item.set_lang(lang);
            }
            item.set_description(s.description.clone());
            tag.push(item);
        }
        for (f, old) in &b.fields {
            set(tag, *f, old);
        }
        rest.raw.extend(b.raw.iter().cloned());
        if let Some(pictures) = pictures {
            while tag.picture_count() > 0 {
                tag.remove_picture(0);
            }
            for p in pictures {
                tag.push_picture(p);
            }
        }
        true
    })
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::file::TaggedFileExt;

    fn song_match() -> SongMatch {
        SongMatch {
            release: "rel-1".into(),
            title: "Take On Me".into(),
            artist: "a-ha".into(),
            album_artist: "a-ha".into(),
            album: "Hunting High and Low".into(),
            year: Some(1985),
            track: 1,
            track_total: 10,
            disc: 1,
            disc_total: 1,
            date: "1985-06-01".into(),
            recording: "rec-1".into(),
            track_id: "trk-1".into(),
            release_group: "grp-1".into(),
            artist_ids: vec!["art-1".into()],
            album_artist_ids: vec!["art-1".into()],
        }
    }

    /// The main tag's text for a key, and the pictures' data.
    fn read(path: &Path, key: ItemKey) -> (Option<String>, Vec<Vec<u8>>) {
        let f = lofty::read_from_path(path).unwrap();
        let tag = f.primary_tag().or_else(|| f.first_tag()).unwrap();
        let mut pictures: Vec<Vec<u8>> = tag.pictures().iter().map(|p| p.data().to_vec()).collect();
        for t in f.tags() {
            if !std::ptr::eq(t, tag) {
                pictures.extend(t.pictures().iter().map(|p| p.data().to_vec()));
            }
        }
        (tag.get_string(key).map(str::to_string), pictures)
    }

    fn has(p: &Path, text: &[u8]) -> bool {
        fs::read(p).unwrap().windows(text.len()).any(|w| w == text)
    }

    const FORMATS: [&str; 5] = ["song.mp3", "song.flac", "song.ogg", "song.opus", "song.m4a"];

    #[test]
    fn writes_and_puts_back() {
        let cover = b"\xFF\xD8\xFF\xE0new cover".to_vec();
        for name in FORMATS {
            let d = tempfile::tempdir().unwrap();
            let p = d.path().join(name);
            fs::copy(Path::new("tests/fixtures").join(name), &p).unwrap();
            let store = d.path().join("store");
            let (_, old_pictures) = read(&p, ItemKey::TrackTitle);
            let original = fs::read(&p).unwrap();

            let b = write(&p, &fields_of(&song_match()), Some(&cover), false, &store)
                .unwrap()
                .expect(name);
            let t = crate::music::read(&p).unwrap();
            assert_eq!(
                (t.title.as_str(), t.album.as_str(), t.album_artist.as_str()),
                ("Take On Me", "Hunting High and Low", "a-ha"),
                "{name}"
            );
            assert_eq!(
                (t.track, t.track_total, t.year),
                (Some(1), Some(10), Some(1985)),
                "{name}"
            );
            assert_eq!(t.mb_release, "rel-1", "{name}");
            // The rest of the tag stays.
            assert_eq!(t.genre, "Keepme", "{name}");
            assert!(has(&p, b"keep"), "{name}: the comment is gone");
            // The cover replaces the pictures there were.
            assert_eq!(
                read(&p, ItemKey::TrackTitle).1,
                vec![cover.clone()],
                "{name}"
            );
            assert_eq!(
                b.pictures.as_ref().map(Vec::len),
                Some(old_pictures.len()),
                "{name}"
            );
            // The audio is still there.
            assert!(t.duration.is_some(), "{name}");
            // Nothing new: the file is left alone.
            let before = fs::read(&p).unwrap();
            let again = write(&p, &fields_of(&song_match()), Some(&cover), false, &store).unwrap();
            assert!(again.is_none(), "{name}: {again:?}");
            assert_eq!(fs::read(&p).unwrap(), before, "{name}");

            restore(&b, &store).unwrap();
            let t = crate::music::read(&p).unwrap();
            assert_eq!(
                (t.title.as_str(), t.album.as_str(), t.artist.as_str()),
                ("Old", "Old", "Someone"),
                "{name}"
            );
            assert_eq!(t.mb_release, "", "{name}");
            assert_eq!(t.genre, "Keepme", "{name}");
            assert_eq!(read(&p, ItemKey::TrackTitle).1, old_pictures, "{name}");
            assert_ne!(fs::read(&p).unwrap(), original, "{name}: written again");
            // No copy left behind.
            let left: Vec<_> = fs::read_dir(d.path())
                .unwrap()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(".renome-tags"))
                .collect();
            assert!(left.is_empty(), "{name}: {left:?}");
        }
    }

    #[test]
    fn cleans_and_puts_back() {
        for name in FORMATS {
            let d = tempfile::tempdir().unwrap();
            let p = d.path().join(name);
            fs::copy(Path::new("tests/fixtures").join(name), &p).unwrap();
            let store = d.path().join("store");
            let b = write(&p, &fields_of(&song_match()), None, true, &store)
                .unwrap()
                .expect(name);
            let t = crate::music::read(&p).unwrap();
            assert_eq!(t.title, "Take On Me", "{name}");
            // The comment goes; the genre (not from MusicBrainz) stays.
            assert!(!has(&p, b"keep"), "{name}: the comment stays");
            assert_eq!(t.genre, "Keepme", "{name}");
            // Cleaned already: nothing to do.
            let again = write(&p, &fields_of(&song_match()), None, true, &store).unwrap();
            assert!(again.is_none(), "{name}: {again:?}");
            restore(&b, &store).unwrap();
            let t = crate::music::read(&p).unwrap();
            assert_eq!(
                (t.title.as_str(), t.genre.as_str()),
                ("Old", "Keepme"),
                "{name}"
            );
            // Vorbis comments come back whole; elsewhere, what lofty knows.
            if name != "song.mp3" && name != "song.m4a" {
                assert!(has(&p, b"keep"), "{name}: the comment is back");
            }
        }
    }

    #[test]
    fn cleaning_keeps_a_cue_sheet() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("song.flac");
        fs::copy("tests/fixtures/song.flac", &p).unwrap();
        {
            let mut f = OpenOptions::new().read(true).write(true).open(&p).unwrap();
            let mut a = FlacFile::read_from(&mut f, ParseOptions::new()).unwrap();
            let mut vc = a.remove_vorbis_comments().unwrap_or_default();
            vc.insert("CUESHEET".into(), "FILE \"song.flac\" WAVE".into());
            vc.insert("PURCHASED_FROM".into(), "some site".into());
            a.set_vorbis_comments(vc);
            f.seek(SeekFrom::Start(0)).unwrap();
            a.save_to(&mut f, WriteOptions::new()).unwrap();
        }
        let store = d.path().join("store");
        let b = write(&p, &fields_of(&song_match()), None, true, &store)
            .unwrap()
            .unwrap();
        assert!(has(&p, b"CUESHEET"));
        assert!(!has(&p, b"some site"));
        restore(&b, &store).unwrap();
        assert!(has(&p, b"some site"));
        assert_eq!(
            fs::read(&p)
                .unwrap()
                .windows(8)
                .filter(|w| *w == b"CUESHEET")
                .count(),
            1
        );
    }

    #[test]
    fn without_cover_pictures_stay() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("song.mp3");
        fs::copy("tests/fixtures/song.mp3", &p).unwrap();
        let (_, pictures) = read(&p, ItemKey::TrackTitle);
        let b = write(
            &p,
            &fields_of(&song_match()),
            None,
            false,
            &d.path().join("s"),
        )
        .unwrap()
        .unwrap();
        assert!(b.pictures.is_none());
        assert_eq!(read(&p, ItemKey::TrackTitle).1, pictures);
        assert!(!writable(Path::new("a.wav")));
        assert_eq!(
            write(Path::new("a.wav"), &Vec::new(), None, false, d.path()),
            Err("tagsUnsupported".into())
        );
    }

    #[test]
    fn details_of_a_track() {
        let r: Release = serde_json::from_str(
            r#"{"id":"r","title":"True North","date":"2022-10-21","country":"XW","barcode":"886449730162",
            "text-representation":{"language":"eng","script":"Latn"},
            "artist-credit":[{"name":"a‐ha","artist":{"id":"a","name":"a‐ha","sort-name":"a‐ha"}}],
            "release-group":{"id":"g","primary-type":"Album","secondary-types":["Soundtrack"],
                "first-release-date":"2022-10-20",
                "genres":[{"name":"europop","count":1},{"name":"pop","count":2}]},
            "label-info":[{"catalog-number":"[none]","label":{"name":"Ariola"}},{"catalog-number":"X1","label":{"name":"Ariola"}}],
            "media":[{"position":1,"format":"Digital Media","title":"","tracks":[
                {"id":"t5","position":5,"title":"True North","recording":{"id":"rec","isrcs":["DEA812200043"],
                 "relations":[
                    {"type":"producer","artist":{"name":"Magne Furuholmen"}},
                    {"type":"mix","artist":{"name":"Craig Silvey"}},
                    {"type":"instrument","artist":{"name":"Paul Waaktaar-Savoy"}},
                    {"type":"performance","work":{"id":"w","relations":[
                        {"type":"composer","artist":{"name":"Magne Furuholmen"}},
                        {"type":"lyricist","artist":{"name":"Magne Furuholmen"}}]}}]}}]}]}"#,
        )
        .unwrap();
        let f = details(&r, "t5");
        let get = |k: Field| f.iter().find(|(x, _)| *x == k).map(|(_, v)| v.clone());
        assert_eq!(get(Field::OriginalDate), Some(vec!["2022-10-20".into()]));
        assert_eq!(get(Field::Label), Some(vec!["Ariola".into()]));
        assert_eq!(get(Field::CatalogNumber), Some(vec!["X1".into()]));
        assert_eq!(
            get(Field::ReleaseType),
            Some(vec!["album".into(), "soundtrack".into()])
        );
        assert_eq!(
            get(Field::Genre),
            Some(vec!["Pop".into(), "Europop".into()])
        );
        assert_eq!(get(Field::Composer), Some(vec!["Magne Furuholmen".into()]));
        assert_eq!(get(Field::Producer), Some(vec!["Magne Furuholmen".into()]));
        assert_eq!(get(Field::Mixer), Some(vec!["Craig Silvey".into()]));
        assert_eq!(get(Field::Isrc), Some(vec!["DEA812200043".into()]));
        assert_eq!(get(Field::AlbumArtistSort), Some(vec!["a-ha".into()]));
        assert_eq!(get(Field::Language), Some(vec!["eng".into()]));
        assert_eq!(get(Field::WorkId), Some(vec!["w".into()]));
        assert_eq!(get(Field::Artists), Some(vec!["a-ha".into()]));
        // An empty disc title is not written.
        assert!(get(Field::DiscSubtitle).is_none());
        assert!(details(&r, "other").is_empty());
    }
}
