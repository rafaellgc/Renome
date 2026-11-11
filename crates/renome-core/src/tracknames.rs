//! Names for the audio and subtitle tracks of a video, made from what
//! each track says (language, flags, codec, channels) and what its old
//! name says, in place of the names that came with it (sites, groups,
//! "Track 2"). Each name is written in the track's own language:
//! `Português (Brasil)`, `English – SDH`, `Español (Latinoamérica) –
//! Forzada`. Tracks that would end up with the same name are told apart
//! by what else their old names said (`Dublagem clássica`), then by
//! their channels and codec (`5.1 AC3`) or format (`ASS`), then by a
//! number. Video tracks lose their names (a name there is almost always
//! a group's or a site's); other tracks keep theirs.

use crate::text::strip_accents;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrackKind {
    Video,
    Audio,
    Subtitle,
    #[default]
    Other,
}

/// A track as the video has it.
#[derive(Debug, Clone, Default)]
pub struct TrackInfo {
    pub kind: TrackKind,
    /// As the file says: `por`, `pt-BR`, `und`, or empty.
    pub lang: String,
    pub name: String,
    /// Short codec name: `AC3`, `SRT`, `PGS` (empty when not known).
    pub codec: String,
    pub channels: u32,
    pub forced: bool,
    pub hearing: bool,
    pub commentary: bool,
    /// Audio description, for the visually impaired.
    pub described: bool,
}

/// What a track gets.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewName {
    /// Empty: no name.
    pub name: String,
    /// The language (three letters, `por`) told by the old name of a
    /// track that did not say it.
    pub lang: Option<&'static str>,
    pub forced: bool,
    pub hearing: bool,
    pub commentary: bool,
    pub described: bool,
}

/// A language: two letters, its bibliographic three-letter code (the
/// one Matroska writes), the other three-letter codes it is written
/// with, its own name, and words naming it in track names.
struct Language {
    two: &'static str,
    three: &'static str,
    others: &'static [&'static str],
    own: &'static str,
    words: &'static [&'static str],
}

const LANGUAGES: &[Language] = &[
    Language { two: "pt", three: "por", others: &["pob", "ptb"], own: "Português", words: &["portuguese", "portugues", "portugais", "portugues-br", "brazilian", "brasileiro", "pt-br", "ptbr", "pt-pt"] },
    Language { two: "en", three: "eng", others: &[], own: "English", words: &["english", "ingles", "anglais", "englisch", "inglese"] },
    Language { two: "es", three: "spa", others: &["spl", "esp"], own: "Español", words: &["spanish", "espanol", "espanhol", "castellano", "castilian", "latino", "espagnol", "spanisch", "spagnolo", "es-419", "es-es", "es-mx"] },
    Language { two: "fr", three: "fre", others: &["fra"], own: "Français", words: &["french", "francais", "frances", "franzosisch", "francese", "vff", "vfq", "vfi"] },
    Language { two: "de", three: "ger", others: &["deu"], own: "Deutsch", words: &["german", "deutsch", "alemao", "aleman", "allemand", "tedesco"] },
    Language { two: "it", three: "ita", others: &[], own: "Italiano", words: &["italian", "italiano", "italien", "italienisch"] },
    Language { two: "ja", three: "jpn", others: &[], own: "日本語", words: &["japanese", "japones", "japonais", "japanisch", "giapponese", "日本語"] },
    Language { two: "ko", three: "kor", others: &[], own: "한국어", words: &["korean", "coreano", "coreen", "koreanisch", "한국어"] },
    Language { two: "zh", three: "chi", others: &["zho", "chs", "cht"], own: "中文", words: &["chinese", "chines", "chino", "chinois", "chinesisch", "cinese", "mandarin", "cantonese", "中文", "简体", "繁體", "繁体"] },
    Language { two: "ru", three: "rus", others: &[], own: "Русский", words: &["russian", "russo", "ruso", "russe", "russisch", "русский"] },
    Language { two: "nl", three: "dut", others: &["nld"], own: "Nederlands", words: &["dutch", "nederlands", "holandes", "neerlandais", "niederlandisch", "olandese", "flemish"] },
    Language { two: "sv", three: "swe", others: &[], own: "Svenska", words: &["swedish", "svenska", "sueco", "suedois", "schwedisch", "svedese"] },
    Language { two: "no", three: "nor", others: &["nob", "nno", "nb", "nn"], own: "Norsk", words: &["norwegian", "norsk", "bokmal", "nynorsk", "noruegues", "noruego", "norvegien", "norwegisch", "norvegese"] },
    Language { two: "da", three: "dan", others: &[], own: "Dansk", words: &["danish", "dansk", "dinamarques", "danes", "danois", "danisch", "danese"] },
    Language { two: "fi", three: "fin", others: &[], own: "Suomi", words: &["finnish", "suomi", "finlandes", "finnois", "finnisch", "finlandese"] },
    Language { two: "pl", three: "pol", others: &[], own: "Polski", words: &["polish", "polski", "polones", "polaco", "polonais", "polnisch", "polacco"] },
    Language { two: "tr", three: "tur", others: &[], own: "Türkçe", words: &["turkish", "turkce", "turco", "turc", "turkisch"] },
    Language { two: "ar", three: "ara", others: &[], own: "العربية", words: &["arabic", "arabe", "arabisch", "arabo", "العربية"] },
    Language { two: "he", three: "heb", others: &[], own: "עברית", words: &["hebrew", "hebraico", "hebreo", "hebreu", "hebraisch", "ebraico", "עברית"] },
    Language { two: "el", three: "gre", others: &["ell"], own: "Ελληνικά", words: &["greek", "grego", "griego", "grec", "griechisch", "greco", "ελληνικα"] },
    Language { two: "cs", three: "cze", others: &["ces"], own: "Čeština", words: &["czech", "cestina", "tcheco", "checo", "tcheque", "tschechisch", "ceco"] },
    Language { two: "hu", three: "hun", others: &[], own: "Magyar", words: &["hungarian", "magyar", "hungaro", "hongrois", "ungarisch", "ungherese"] },
    Language { two: "ro", three: "rum", others: &["ron"], own: "Română", words: &["romanian", "romana", "romeno", "rumano", "roumain", "rumanisch"] },
    Language { two: "uk", three: "ukr", others: &[], own: "Українська", words: &["ukrainian", "ucraniano", "ukrainien", "ukrainisch", "ucraino", "українська"] },
    Language { two: "th", three: "tha", others: &[], own: "ไทย", words: &["thai", "tailandes", "thailandais", "thailandisch", "ไทย"] },
    Language { two: "vi", three: "vie", others: &[], own: "Tiếng Việt", words: &["vietnamese", "vietnamita", "vietnamien", "vietnamesisch"] },
    Language { two: "id", three: "ind", others: &[], own: "Bahasa Indonesia", words: &["indonesian", "indonesio", "indonesien", "indonesisch"] },
    Language { two: "ms", three: "may", others: &["msa"], own: "Bahasa Melayu", words: &["malay", "malaio", "malayo", "malais"] },
    Language { two: "ca", three: "cat", others: &[], own: "Català", words: &["catalan", "catala", "catalao"] },
    Language { two: "hr", three: "hrv", others: &["scr"], own: "Hrvatski", words: &["croatian", "hrvatski", "croata", "croate", "kroatisch"] },
    Language { two: "sr", three: "srp", others: &["scc"], own: "Српски", words: &["serbian", "srpski", "servio", "serbio", "serbe", "serbisch"] },
    Language { two: "bg", three: "bul", others: &[], own: "Български", words: &["bulgarian", "bulgaro", "bulgare", "bulgarisch"] },
    Language { two: "sk", three: "slo", others: &["slk"], own: "Slovenčina", words: &["slovak", "slovencina", "eslovaco", "slovaque", "slowakisch"] },
    Language { two: "sl", three: "slv", others: &[], own: "Slovenščina", words: &["slovenian", "slovene", "slovenscina", "esloveno", "slovene"] },
    Language { two: "et", three: "est", others: &[], own: "Eesti", words: &["estonian", "eesti", "estonio", "estonien", "estnisch"] },
    Language { two: "lv", three: "lav", others: &[], own: "Latviešu", words: &["latvian", "latviesu", "leton", "letao", "letton", "lettisch"] },
    Language { two: "lt", three: "lit", others: &[], own: "Lietuvių", words: &["lithuanian", "lietuviu", "lituano", "lituanien", "litauisch"] },
    Language { two: "is", three: "ice", others: &["isl"], own: "Íslenska", words: &["icelandic", "islenska", "islandes", "islandais", "islandisch"] },
    Language { two: "hi", three: "hin", others: &[], own: "हिन्दी", words: &["hindi", "हिन्दी"] },
    Language { two: "ta", three: "tam", others: &[], own: "தமிழ்", words: &["tamil"] },
    Language { two: "te", three: "tel", others: &[], own: "తెలుగు", words: &["telugu"] },
    Language { two: "bn", three: "ben", others: &[], own: "বাংলা", words: &["bengali", "bangla"] },
    Language { two: "fa", three: "per", others: &["fas"], own: "فارسی", words: &["persian", "farsi", "persa", "persan"] },
    Language { two: "ur", three: "urd", others: &[], own: "اردو", words: &["urdu"] },
    Language { two: "fil", three: "fil", others: &["tgl"], own: "Filipino", words: &["filipino", "tagalog"] },
    Language { two: "eu", three: "baq", others: &["eus"], own: "Euskara", words: &["basque", "euskara", "basco", "vasco", "euskera"] },
    Language { two: "gl", three: "glg", others: &[], own: "Galego", words: &["galician", "galego", "gallego"] },
];

fn language(code: &str) -> Option<&'static Language> {
    let c = code.to_ascii_lowercase();
    LANGUAGES
        .iter()
        .find(|l| l.two == c || l.three == c || l.others.contains(&c.as_str()))
}

/// The regions shown, by language: what tells them in a code or a
/// name, and how the language writes them.
fn region_of_word(two: &str, word: &str) -> Option<&'static str> {
    Some(match (two, word) {
        ("pt", "brasil" | "brazil" | "brazilian" | "brasileiro" | "brasileira" | "pt-br" | "ptbr" | "pob" | "ptb" | "portugues-br") => "BR",
        ("pt", "portugal" | "europeu" | "european" | "pt-pt" | "ptpt") => "PT",
        ("es", "latino" | "latin" | "america" | "latam" | "latinoamerica" | "latinoamericano" | "es-419" | "es-mx" | "mexico" | "mexicano" | "spl") => "419",
        ("es", "castellano" | "castilian" | "espana" | "spain" | "es-es" | "europeo" | "european") => "ES",
        ("zh", "simplified" | "simplificado" | "简体" | "简" | "chs" | "zh-hans" | "zh-cn" | "mandarin") => "Hans",
        ("zh", "traditional" | "tradicional" | "繁體" | "繁体" | "繁" | "cht" | "zh-hant" | "zh-tw" | "zh-hk" | "cantonese") => "Hant",
        ("fr", "canada" | "canadien" | "quebec" | "quebecois" | "vfq" | "fr-ca") => "CA",
        _ => return None,
    })
}

/// A region written in a language tag (`pt-BR`, `es-419`, `zh-Hant`).
fn region_of_tag(two: &str, region: &str) -> Option<&'static str> {
    let r = region.to_ascii_lowercase();
    Some(match (two, r.as_str()) {
        ("pt", "br") => "BR",
        ("pt", "pt") => "PT",
        ("es", "es") => "ES",
        ("es", _) if r == "419" || r.len() == 2 => "419",
        ("zh", "hans" | "cn" | "sg") => "Hans",
        ("zh", "hant" | "tw" | "hk" | "mo") => "Hant",
        ("fr", "ca") => "CA",
        _ => return None,
    })
}

fn region_name(two: &str, region: &str) -> &'static str {
    match (two, region) {
        ("pt", "BR") => "Brasil",
        ("pt", "PT") => "Portugal",
        ("es", "419") => "Latinoamérica",
        ("es", "ES") => "España",
        ("zh", "Hans") => "简体",
        ("zh", "Hant") => "繁體",
        ("fr", "CA") => "Canada",
        _ => "",
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Word {
    Forced,
    Sdh,
    Commentary,
    Described,
    Signs,
}

/// A detail's name in a language (English for the others).
fn word(two: &str, w: Word) -> &'static str {
    match (w, two) {
        (Word::Sdh, _) => "SDH",
        (Word::Forced, "pt") => "Forçada",
        (Word::Forced, "es") => "Forzada",
        (Word::Forced, "fr") => "Forcés",
        (Word::Forced, "de") => "Erzwungen",
        (Word::Forced, "it") => "Forzati",
        (Word::Forced, _) => "Forced",
        (Word::Commentary, "pt") => "Comentários",
        (Word::Commentary, "es") => "Comentarios",
        (Word::Commentary, "fr") => "Commentaires",
        (Word::Commentary, "de") => "Kommentar",
        (Word::Commentary, "it") => "Commento",
        (Word::Commentary, _) => "Commentary",
        (Word::Described, "pt") => "Audiodescrição",
        (Word::Described, "es") => "Audiodescripción",
        (Word::Described, "fr") => "Audiodescription",
        (Word::Described, "de") => "Audiodeskription",
        (Word::Described, "it") => "Audiodescrizione",
        (Word::Described, _) => "Audio description",
        (Word::Signs, "pt") => "Letreiros e músicas",
        (Word::Signs, "es") => "Carteles y canciones",
        (Word::Signs, "fr") => "Panneaux et chansons",
        (Word::Signs, "de") => "Schilder und Lieder",
        (Word::Signs, "it") => "Cartelli e canzoni",
        (Word::Signs, _) => "Signs & songs",
    }
}

/// Words of an old name that say a detail (folded: lower case, no
/// accents).
fn detail_of(w: &str) -> Option<Word> {
    Some(match w {
        "forced" | "forcada" | "forcadas" | "forcado" | "forcados" | "forzada" | "forzadas" | "forzado" | "forzados"
        | "forces" | "erzwungen" | "forzati" | "foreign" => Word::Forced,
        "sdh" | "cc" | "hi" | "hoh" | "surdos" | "surdo" | "sordos" => Word::Sdh,
        "commentary" | "commentaries" | "comment" | "comments" | "comentario" | "comentarios" | "comentado"
        | "commentaire" | "commentaires" | "kommentar" | "commento" => Word::Commentary,
        "audiodescricao" | "audiodescripcion" | "audiodescription" | "audiodeskription" | "audiodescrizione"
        | "descriptive" | "descritivo" | "descriptiva" => Word::Described,
        "signs" | "letreiros" | "carteles" | "placas" | "panneaux" => Word::Signs,
        _ => return None,
    })
}

/// Phrases of more than one word that say a detail.
const PHRASES: &[(&[&str], Word)] = &[
    (&["hearing", "impaired"], Word::Sdh),
    (&["closed", "captions"], Word::Sdh),
    (&["closed", "caption"], Word::Sdh),
    (&["deficientes", "auditivos"], Word::Sdh),
    (&["audio", "description"], Word::Described),
    (&["audio", "descricao"], Word::Described),
    (&["audio", "descripcion"], Word::Described),
    (&["video", "description"], Word::Described),
];

/// Words that say nothing to tell tracks apart: kinds, codecs, channels,
/// formats.
const PLAIN: &[&str] = &[
    "audio", "sub", "subs", "subtitle", "subtitles", "subtitulo", "subtitulos", "subtitulado", "legenda", "legendas",
    "legendado", "legendada", "legendados", "sous-titres", "untertitel", "sottotitoli", "dublado", "dublada", "dubbed",
    "dub", "doblado", "doblada", "track", "faixa", "pista", "piste", "spur", "full", "completa", "completo", "complete",
    "completas", "default", "padrao", "stereo", "estereo", "mono", "surround", "ch", "channels", "canais", "canales",
    "aac", "ac3", "ac-3", "eac3", "e-ac3", "e-ac-3", "dd", "dd+", "ddp", "dolby", "digital", "plus", "dts", "dts-hd",
    "dts-x", "dts:x", "ma", "hra", "es", "truehd", "true-hd", "atmos", "flac", "opus", "mp3", "mp2", "lpcm", "pcm", "vorbis",
    "srt", "ass", "ssa", "pgs", "sup", "vobsub", "subrip", "webvtt", "vtt", "text", "texto", "utf8", "utf-8", "idx",
    "kbps", "khz", "bit", "hevc", "avc", "x264", "x265", "h264", "h265", "and", "e", "y", "et", "und", "&", "+",
];

/// Words left at either end of what is kept (`por Fulano` → `Fulano`).
const JOINERS: &[&str] = &["by", "por", "de", "from", "da", "do", "von", "par", "-", "–", "e", "and", "y", "et"];

/// A site, address or handle (`site.com`, `www.x`, `@group`).
fn is_address(word: &str) -> bool {
    let w = word.to_ascii_lowercase();
    let w = w.trim_matches(|c: char| !c.is_alphanumeric());
    if w.is_empty() {
        return false;
    }
    if word.contains("://") || w.starts_with("www.") || word.starts_with('@') || w.contains('@') {
        return true;
    }
    const ENDS: &[&str] = &[
        "com", "net", "org", "br", "to", "tv", "me", "io", "cc", "co", "xyz", "info", "biz", "ru", "nu", "se", "ws",
        "in", "uk", "us", "site", "club", "online", "top", "pw", "is", "li", "la", "ch", "su", "lol", "eu",
    ];
    match w.rsplit_once('.') {
        Some((head, end)) => {
            !head.is_empty()
                && head.chars().any(|c| c.is_alphabetic())
                && ENDS.contains(&end)
        }
        None => false,
    }
}

/// What an old name says: language, region, details, and the rest of
/// its words (as written, minus addresses and plain words).
#[derive(Default)]
struct Said {
    lang: Option<&'static Language>,
    regions: Vec<(&'static str, &'static str)>,
    details: Vec<Word>,
    rest: String,
}

fn fold(s: &str) -> String {
    strip_accents(s).to_lowercase()
}

fn read_name(name: &str) -> Said {
    let mut said = Said::default();
    // Words, split at spaces and brackets; addresses go first, with a
    // bracket around one.
    let mut words: Vec<&str> = Vec::new();
    for part in name.split(|c: char| c.is_whitespace() || "()[]{}|,;/_".contains(c)) {
        if !part.is_empty() && !is_address(part) {
            words.push(part);
        }
    }
    let folded: Vec<String> = words
        .iter()
        .map(|w| fold(w.trim_matches(|c: char| !c.is_alphanumeric() && !"+&".contains(c))))
        .collect();
    let mut keep = vec![true; words.len()];
    for (i, f) in folded.iter().enumerate() {
        if f.is_empty() {
            keep[i] = false;
            continue;
        }
        for (phrase, w) in PHRASES {
            if folded[i..].len() >= phrase.len() && folded[i..i + phrase.len()].iter().zip(*phrase).all(|(a, b)| a == b) {
                said.details.push(*w);
                for k in keep.iter_mut().skip(i).take(phrase.len()) {
                    *k = false;
                }
            }
        }
        if let Some(w) = detail_of(f) {
            said.details.push(w);
            keep[i] = false;
            continue;
        }
        // "songs" with signs: part of "signs & songs".
        if ["songs", "musicas", "canciones", "chansons"].contains(&f.as_str()) && folded.iter().any(|x| detail_of(x) == Some(Word::Signs)) {
            keep[i] = false;
            continue;
        }
        let word_lang = LANGUAGES.iter().find(|l| {
            l.words.contains(&f.as_str())
                || l.three == f
                || l.others.contains(&f.as_str())
                // Two letters only in capitals (`PT`, `EN`): in lower
                // case they are words (`de`, `no`).
                || (l.two == f && words[i].chars().all(|c| c.is_ascii_uppercase()))
        });
        if let Some(l) = word_lang {
            said.lang.get_or_insert(l);
            keep[i] = false;
        }
        for l in LANGUAGES {
            if let Some(r) = region_of_word(l.two, f) {
                said.regions.push((l.two, r));
                keep[i] = false;
            }
        }
        if ["br", "pt"].contains(&f.as_str()) && words[i].chars().all(|c| c.is_ascii_uppercase()) {
            said.regions.push(("pt", if f == "br" { "BR" } else { "PT" }));
            keep[i] = false;
        }
        let channels = f.len() == 3 && f.as_bytes()[1] == b'.' && f.as_bytes()[0].is_ascii_digit() && f.as_bytes()[2].is_ascii_digit();
        let digit = f.len() == 1 && f.as_bytes()[0].is_ascii_digit();
        if PLAIN.contains(&f.as_str()) || channels || digit || f.chars().all(|c| !c.is_alphanumeric()) {
            keep[i] = false;
        }
    }
    let mut rest: Vec<&str> = words
        .iter()
        .zip(&keep)
        .filter(|(_, k)| **k)
        .map(|(w, _)| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '!' && c != '.'))
        .filter(|w| !w.is_empty())
        .collect();
    while rest.first().is_some_and(|w| JOINERS.contains(&fold(w).as_str())) {
        rest.remove(0);
    }
    while rest.last().is_some_and(|w| JOINERS.contains(&fold(w).as_str())) {
        rest.pop();
    }
    said.rest = rest.join(" ");
    if !said.rest.chars().any(char::is_alphabetic) {
        said.rest.clear();
    }
    said
}

/// The channels as people say them: `Mono`, `2.0`, `5.1`, `7.1`.
fn channels(n: u32) -> String {
    match n {
        0 => String::new(),
        1 => "Mono".into(),
        2 => "2.0".into(),
        3 => "2.1".into(),
        6 => "5.1".into(),
        7 => "6.1".into(),
        8 => "7.1".into(),
        n => format!("{n}ch"),
    }
}

fn undetermined(code: &str) -> bool {
    ["", "und", "zxx", "mis", "mul", "qaa"].contains(&code.to_ascii_lowercase().as_str())
}

/// The new names (and flags) of `tracks`, in the same order.
pub fn names(tracks: &[TrackInfo]) -> Vec<NewName> {
    struct Work {
        base: String,
        details: Vec<String>,
        rest: String,
        apart: String,
        new: NewName,
        named: bool,
    }
    let mut work: Vec<Work> = tracks
        .iter()
        .map(|t| {
            let mut new = NewName {
                name: t.name.clone(),
                forced: t.forced,
                hearing: t.hearing,
                commentary: t.commentary,
                described: t.described,
                ..Default::default()
            };
            if !matches!(t.kind, TrackKind::Audio | TrackKind::Subtitle) {
                if t.kind == TrackKind::Video {
                    new.name.clear();
                }
                return Work { base: String::new(), details: Vec::new(), rest: String::new(), apart: String::new(), new, named: false };
            }
            let said = read_name(&t.name);
            let (code, tag_region) = t.lang.split_once(['-', '_']).unwrap_or((&t.lang, ""));
            let mut lang = language(code);
            if lang.is_none() && undetermined(code) {
                lang = said.lang;
                new.lang = said.lang.map(|l| l.three);
            }
            let two = lang.map_or("", |l| l.two);
            let region = region_of_tag(two, tag_region)
                .or_else(|| ["pob", "ptb"].contains(&code.to_ascii_lowercase().as_str()).then_some("BR"))
                .or_else(|| (code.eq_ignore_ascii_case("spl")).then_some("419"))
                .or_else(|| said.regions.iter().find(|(l, _)| *l == two).map(|(_, r)| *r));
            let mut base = lang.map(|l| l.own.to_string()).unwrap_or_default();
            if let Some(r) = region {
                let n = region_name(two, r);
                if !n.is_empty() {
                    base.push_str(&format!(" ({n})"));
                }
            }
            let has = |w: Word| said.details.contains(&w);
            let sub = t.kind == TrackKind::Subtitle;
            new.forced |= sub && has(Word::Forced);
            new.hearing |= sub && has(Word::Sdh);
            new.commentary |= has(Word::Commentary);
            new.described |= !sub && has(Word::Described);
            let wl = if two.is_empty() { "en" } else { two };
            let mut details = Vec::new();
            if sub && new.forced {
                details.push(word(wl, Word::Forced).to_string());
            }
            if sub && has(Word::Signs) {
                details.push(word(wl, Word::Signs).to_string());
            }
            if sub && new.hearing {
                details.push(word(wl, Word::Sdh).to_string());
            }
            if new.commentary {
                details.push(word(wl, Word::Commentary).to_string());
            }
            if !sub && new.described {
                details.push(word(wl, Word::Described).to_string());
            }
            // Without a language, what else the name said stays.
            let rest = said.rest;
            if base.is_empty() && !rest.is_empty() {
                details.push(rest.clone());
            }
            let apart = if sub {
                t.codec.clone()
            } else {
                [channels(t.channels), t.codec.clone()]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            Work { base, details, rest: if lang.is_some() { rest } else { String::new() }, apart, new, named: true }
        })
        .collect();

    let key = |w: &Work| format!("{}\u{1}{}", w.base, w.details.join("\u{2}"));
    // Each step tells apart the tracks still sharing a name, when it
    // gives them different words.
    for step in 0..3 {
        let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
        for (i, w) in work.iter().enumerate().filter(|(_, w)| w.named) {
            let k = format!("{:?}{}", tracks[i].kind, key(w));
            match groups.iter_mut().find(|g| g.0 == k) {
                Some(g) => g.1.push(i),
                None => groups.push((k, vec![i])),
            }
        }
        for (_, idx) in groups.into_iter().filter(|g| g.1.len() > 1) {
            let words: Vec<String> = idx
                .iter()
                .map(|&i| match step {
                    0 => work[i].rest.clone(),
                    1 => work[i].apart.clone(),
                    _ => String::new(),
                })
                .collect();
            if step == 2 {
                for (n, &i) in idx.iter().enumerate() {
                    work[i].details.push((n + 1).to_string());
                }
                continue;
            }
            if words.iter().all(|w| *w == words[0]) {
                continue;
            }
            for (&i, w) in idx.iter().zip(words) {
                if !w.is_empty() {
                    work[i].details.push(w);
                }
            }
        }
    }
    work.into_iter()
        .map(|w| {
            let mut new = w.new;
            if w.named {
                new.name = match (w.base.is_empty(), w.details.is_empty()) {
                    (_, true) => w.base,
                    (true, false) => w.details.join(", "),
                    (false, false) => format!("{} – {}", w.base, w.details.join(", ")),
                };
            }
            new
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audio(lang: &str, name: &str, codec: &str, ch: u32) -> TrackInfo {
        TrackInfo { kind: TrackKind::Audio, lang: lang.into(), name: name.into(), codec: codec.into(), channels: ch, ..Default::default() }
    }

    fn sub(lang: &str, name: &str, codec: &str) -> TrackInfo {
        TrackInfo { kind: TrackKind::Subtitle, lang: lang.into(), name: name.into(), codec: codec.into(), ..Default::default() }
    }

    fn got(t: &[TrackInfo]) -> Vec<String> {
        names(t).into_iter().map(|n| n.name).collect()
    }

    #[test]
    fn languages_regions_and_details() {
        let t = [
            TrackInfo { kind: TrackKind::Video, name: "Encoded by GROUP www.site.com".into(), ..Default::default() },
            audio("eng", "English 5.1 - www.site.com", "AC3", 6),
            sub("por", "Portuguese (Brazil) [site.to]", "SRT"),
            sub("pt-BR", "Forced", "SRT"),
            sub("eng", "English SDH", "SRT"),
            sub("spa", "Spanish (Latin America)", "SRT"),
            sub("chi", "Chinese Traditional", "PGS"),
            audio("eng", "Director's Commentary", "AAC", 2),
        ];
        let n = names(&t);
        assert_eq!(n[0].name, "");
        assert_eq!(n[1].name, "English");
        assert_eq!(n[2].name, "Português (Brasil)");
        assert_eq!(n[3].name, "Português (Brasil) – Forçada");
        assert!(n[3].forced);
        assert_eq!(n[4].name, "English – SDH");
        assert!(n[4].hearing);
        assert_eq!(n[5].name, "Español (Latinoamérica)");
        assert_eq!(n[6].name, "中文 (繁體)");
        assert_eq!(n[7].name, "English – Commentary");
        assert!(n[7].commentary);
    }

    #[test]
    fn same_language_told_apart() {
        // Dubs: by what their names say, then by channels and codec.
        let t = [
            audio("por", "Dublagem Clássica - site.com", "AC3", 2),
            audio("por", "Redublagem [@grupo]", "AC3", 2),
            audio("eng", "English", "DTS", 6),
            audio("eng", "English", "AC3", 6),
            audio("spa", "Spanish", "AAC", 2),
            audio("spa", "Spanish", "AAC", 2),
        ];
        assert_eq!(
            got(&t),
            [
                "Português – Dublagem Clássica",
                "Português – Redublagem",
                "English – 5.1 DTS",
                "English – 5.1 AC3",
                "Español – 1",
                "Español – 2",
            ]
        );
        // Subtitles: by who made them, then by format, then a number.
        let t = [
            sub("por", "Legendado por Fulano - www.legendas.com", "SRT"),
            sub("por", "Legendas por Beltrano", "SRT"),
            sub("eng", "English", "ASS"),
            sub("eng", "English", "SRT"),
            sub("fre", "", "SRT"),
            sub("fre", "", "SRT"),
        ];
        assert_eq!(
            got(&t),
            ["Português – Fulano", "Português – Beltrano", "English – ASS", "English – SRT", "Français – 1", "Français – 2"]
        );
    }

    #[test]
    fn names_seen_in_real_files() {
        let t = [
            sub("es", "European Spanish", "SRT"),
            sub("es", "Spanish", "SRT"),
            sub("nb", "Norwegian Bokmål", "SRT"),
            audio("eng", "AC-3 2.0 - 192 kbps (Commentary)", "AC3", 2),
            audio("eng", "DTS-HD MA 5.1 - 2208 kbps", "DTS", 6),
            sub("pt", "FORÇADA PT-BR", "SRT"),
            sub("en", "Inglês SDH", "SRT"),
        ];
        assert_eq!(
            got(&t),
            ["Español (España)", "Español", "Norsk", "English – Commentary", "English", "Português (Brasil) – Forçada", "English – SDH"]
        );
    }

    #[test]
    fn undetermined_told_by_name() {
        let n = names(&[sub("und", "Brazilian Portuguese", "SRT"), sub("", "", "SRT"), audio("und", "Track 1", "AAC", 2)]);
        assert_eq!(n[0].name, "Português (Brasil)");
        assert_eq!(n[0].lang, Some("por"));
        assert_eq!(n[1].name, "");
        assert_eq!(n[2].name, "");
        assert_eq!(n[2].lang, None);
    }

    #[test]
    fn words_kept_without_language() {
        let n = names(&[sub("und", "Signs & Songs", "ASS"), audio("und", "Original Mix", "FLAC", 2)]);
        assert_eq!(n[0].name, "Signs & songs");
        assert_eq!(n[1].name, "Original Mix");
    }

    #[test]
    fn addresses() {
        for w in ["site.com", "www.x.org", "@grupo", "https://a.b/c", "Legendas.TV", "[site.net]"] {
            assert!(is_address(w), "{w}");
        }
        for w in ["5.1", "Vol.2", "Dr.", "Ch.1", "St."] {
            assert!(!is_address(w), "{w}");
        }
    }
}
