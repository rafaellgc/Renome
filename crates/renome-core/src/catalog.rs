//! Every rule and every option it takes: name, kind, default, the old
//! name it had, and the keys of its label and hint in the page's texts
//! (`src/lib/i18n.ts`). The window and the command line build rules from
//! here, check values against it and bring saved rules with old option
//! names up to date ([`migrate_step`]).
//!
//! Defaults that depend on the language (templates, subtitle languages)
//! come in the order Portuguese, English, Spanish.

use serde_json::{Map, Value};

/// The languages, in the order of [`Default::Lang`].
pub const LANGS: [&str; 3] = ["pt", "en", "es"];

/// What an option holds.
#[derive(Debug, Clone, Copy)]
pub enum Kind {
    /// Any text.
    Text,
    /// A template with `{keys}`.
    Template,
    /// Text of several lines (one name per line).
    Lines,
    /// Yes or no.
    Bool,
    /// A whole number between the bounds.
    Int { min: i64, max: i64 },
    /// One of these whole numbers.
    IntChoice(&'static [i64]),
    /// A number with decimals, not below 0.
    Float,
    /// One of these words (value saved, word typed on the command line,
    /// key of its label; the word is the value when empty).
    Choice(&'static [Choice]),
}

/// One of an option's choices.
#[derive(Debug, Clone, Copy)]
pub struct Choice {
    /// As saved in the rule.
    pub value: &'static str,
    /// As typed on the command line when the value is not a word (`""`).
    pub word: &'static str,
    /// Key of its label in the page's texts.
    pub label: &'static str,
}

const fn c(value: &'static str, label: &'static str) -> Choice {
    Choice {
        value,
        word: "",
        label,
    }
}
const fn cw(value: &'static str, word: &'static str, label: &'static str) -> Choice {
    Choice { value, word, label }
}

impl Choice {
    /// The word typed for it.
    pub fn word(&self) -> &'static str {
        if self.word.is_empty() {
            self.value
        } else {
            self.word
        }
    }
}

/// An option's value when left out.
#[derive(Debug, Clone, Copy)]
pub enum Default {
    Text(&'static str),
    /// Per language: Portuguese, English, Spanish.
    Lang([&'static str; 3]),
    Bool(bool),
    Int(i64),
    Float(f64),
}

/// One option of a rule.
#[derive(Debug, Clone, Copy)]
pub struct Opt {
    /// Its name in saved rules and on the command line.
    pub name: &'static str,
    /// The name it had before, still read.
    pub old: Option<&'static str>,
    pub kind: Kind,
    pub default: Default,
    /// Keys of its label and hint in the page's texts.
    pub label: &'static str,
    pub hint: Option<&'static str>,
}

const fn o(
    name: &'static str,
    kind: Kind,
    default: Default,
    label: &'static str,
    hint: Option<&'static str>,
) -> Opt {
    Opt {
        name,
        old: None,
        kind,
        default,
        label,
        hint,
    }
}
const fn renamed(
    name: &'static str,
    old: &'static str,
    kind: Kind,
    default: Default,
    label: &'static str,
    hint: Option<&'static str>,
) -> Opt {
    Opt {
        name,
        old: Some(old),
        kind,
        default,
        label,
        hint,
    }
}

/// A rule: its type name and options.
#[derive(Debug, Clone, Copy)]
pub struct RuleSpec {
    /// As in saved rules (`"type"`) and on the command line.
    pub kind: &'static str,
    /// Key of what the rule does, in the page's texts (its name is
    /// `rule.<kind>`).
    pub hint: Option<&'static str>,
    /// Whether it names files from a catalogue or from what they hold
    /// (one at a time in a list).
    pub whole_name: bool,
    pub options: &'static [Opt],
}

impl RuleSpec {
    /// Key of the rule's name in the page's texts.
    pub fn label(&self) -> String {
        format!("rule.{}", self.kind)
    }

    /// The option called so, by its name or the old one, written in
    /// camelCase or kebab-case, case ignored.
    pub fn option(&self, name: &str) -> Option<&'static Opt> {
        let want = squash(name);
        self.options
            .iter()
            .find(|o| squash(o.name) == want || o.old.is_some_and(|old| squash(old) == want))
    }
}

/// A name without `-`, `_` and case: `first-only`, `firstOnly` and
/// `FIRST_ONLY` are one.
pub fn squash(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '-' && *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

const TARGET: Kind = Kind::Choice(&[
    c("name", "targetName"),
    c("ext", "targetExt"),
    c("full", "targetFull"),
]);
const KEEP_DELETE_BOOK: Kind = Kind::Choice(&[
    c("keep", "bookOriginalKeep"),
    c("delete", "comicOriginalDelete"),
]);
const KEEP_DELETE_COMIC: Kind = Kind::Choice(&[
    c("keep", "comicOriginalKeep"),
    c("delete", "comicOriginalDelete"),
]);
const KEEP_DELETE_SUB: Kind = Kind::Choice(&[
    c("keep", "subOriginalKeep"),
    c("delete", "comicOriginalDelete"),
]);
const BOUND: Kind = Kind::Choice(&[
    c("start", "bStart"),
    c("end", "bEnd"),
    c("position", "bPosition"),
    c("text", "bText"),
]);
const NUMBER: Kind = Kind::Int {
    min: i64::MIN,
    max: i64::MAX,
};
const COUNT: Kind = Kind::Int {
    min: 0,
    max: 100_000,
};
const POSITION: Kind = Kind::Int {
    min: 1,
    max: 100_000,
};
const NO: Default = Default::Bool(false);
const YES: Default = Default::Bool(true);
const EMPTY: Default = Default::Text("");
const COLON: Opt = renamed(
    "colonAs",
    "colon",
    Kind::Text,
    Default::Text(" - "),
    "colonLabel",
    None,
);
const SUB_LANGS: Default = Default::Lang(["pt-BR", "en", "es"]);

/// The comic readers pages are fit to.
pub const DEVICES: &[Choice] = &[
    c("original", "comicDevice_original"),
    c("kindle", "comicDevice_kindle"),
    c("kindle-pw", "comicDevice_kindle-pw"),
    c("kindle-pw2024", "comicDevice_kindle-pw2024"),
    c("kindle-oasis", "comicDevice_kindle-oasis"),
    c("kindle-pw-old", "comicDevice_kindle-pw-old"),
    c("kindle-basic", "comicDevice_kindle-basic"),
    c("kindle-dx", "comicDevice_kindle-dx"),
    c("kindle-scribe", "comicDevice_kindle-scribe"),
    c("kindle-scribe2025", "comicDevice_kindle-scribe2025"),
    c("kindle-color", "comicDevice_kindle-color"),
    c("kindle-scribe-color", "comicDevice_kindle-scribe-color"),
    c("kobo", "comicDevice_kobo"),
    c("kobo-clara-color", "comicDevice_kobo-clara-color"),
    c("kobo-libra", "comicDevice_kobo-libra"),
    c("kobo-color", "comicDevice_kobo-color"),
    c("kobo-h2o", "comicDevice_kobo-h2o"),
    c("kobo-sage", "comicDevice_kobo-sage"),
    c("kobo-elipsa", "comicDevice_kobo-elipsa"),
    c("kobo-nia", "comicDevice_kobo-nia"),
    c("remarkable", "comicDevice_remarkable"),
    c("remarkable-pro", "comicDevice_remarkable-pro"),
    c("remarkable-move", "comicDevice_remarkable-move"),
    c("eink480", "comicDevice_eink480"),
    c("eink528", "comicDevice_eink528"),
    c("eink824", "comicDevice_eink824"),
    c("colour824", "comicDevice_colour824"),
    c("colour1404", "comicDevice_colour1404"),
    c("eink1650", "comicDevice_eink1650"),
    c("eink1860", "comicDevice_eink1860"),
    c("colour1860", "comicDevice_colour1860"),
    c("eink1920", "comicDevice_eink1920"),
    c("eink2400", "comicDevice_eink2400"),
    c("colour2400", "comicDevice_colour2400"),
    c("tablet", "comicDevice_tablet"),
    c("ipad", "comicDevice_ipad"),
    c("ipad13", "comicDevice_ipad13"),
    c("phone", "comicDevice_phone"),
    c("custom", "comicDevice_custom"),
];

/// Every rule, in the order of the window's "+ Rule" menu.
pub const RULES: &[RuleSpec] = &[
    RuleSpec {
        kind: "replace",
        hint: None,
        whole_name: false,
        options: &[
            o("find", Kind::Text, EMPTY, "find", None),
            o("replace", Kind::Text, EMPTY, "replaceWith", None),
            o("caseSensitive", Kind::Bool, NO, "caseSensitive", None),
            o(
                "firstOnly",
                Kind::Bool,
                NO,
                "firstOnly",
                Some("firstOnlyHint"),
            ),
            o("target", TARGET, Default::Text("name"), "applyTo", None),
        ],
    },
    RuleSpec {
        kind: "regex",
        hint: None,
        whole_name: false,
        options: &[
            o("pattern", Kind::Text, EMPTY, "expression", None),
            o(
                "replace",
                Kind::Text,
                EMPTY,
                "replaceWith",
                Some("groupsHint"),
            ),
            o("caseSensitive", Kind::Bool, NO, "caseSensitive", None),
            o(
                "firstOnly",
                Kind::Bool,
                NO,
                "firstOnly",
                Some("firstOnlyHint"),
            ),
            o("target", TARGET, Default::Text("name"), "applyTo", None),
        ],
    },
    RuleSpec {
        kind: "rearrange",
        hint: None,
        whole_name: false,
        options: &[
            o(
                "separator",
                Kind::Text,
                Default::Text(" - "),
                "separator",
                None,
            ),
            o("order", Kind::Text, EMPTY, "newOrder", Some("newOrderHint")),
            o("target", TARGET, Default::Text("name"), "applyTo", None),
        ],
    },
    RuleSpec {
        kind: "list",
        hint: Some("listFillHint"),
        whole_name: false,
        options: &[
            o(
                "names",
                Kind::Lines,
                EMPTY,
                "listNames",
                Some("listOrderHint"),
            ),
            o("pairs", Kind::Bool, YES, "pairs", Some("pairsHint")),
        ],
    },
    RuleSpec {
        kind: "case",
        hint: None,
        whole_name: false,
        options: &[
            o(
                "mode",
                Kind::Choice(&[
                    c("lower", "caseLower"),
                    c("upper", "caseUpper"),
                    c("sentence", "caseSentence"),
                    c("title", "caseTitle"),
                    c("titleSmart", "caseTitleSmart"),
                ]),
                Default::Text("titleSmart"),
                "mode",
                None,
            ),
            o("target", TARGET, Default::Text("name"), "applyTo", None),
            renamed(
                "keepWords",
                "keep",
                Kind::Text,
                EMPTY,
                "caseKeep",
                Some("caseKeepHint"),
            ),
        ],
    },
    RuleSpec {
        kind: "insert",
        hint: None,
        whole_name: false,
        options: &[
            o(
                "source",
                Kind::Choice(&[
                    c("text", "srcText"),
                    c("now", "srcNow"),
                    c("modified", "srcModified"),
                    c("taken", "srcTaken"),
                    c("folder", "srcFolder"),
                ]),
                Default::Text("text"),
                "what",
                None,
            ),
            o("text", Kind::Text, EMPTY, "text", None),
            o(
                "dateFormat",
                Kind::Text,
                Default::Text("%Y-%m-%d"),
                "format",
                Some("formatHint"),
            ),
            o(
                "fallback",
                Kind::Bool,
                YES,
                "takenFallback",
                Some("takenHint"),
            ),
            o(
                "at",
                Kind::Choice(&[
                    c("start", "atStart"),
                    c("end", "atEnd"),
                    c("index", "atIndex"),
                    c("fromEnd", "atFromEnd"),
                ]),
                Default::Text("start"),
                "where",
                None,
            ),
            o("position", POSITION, Default::Int(1), "position", None),
        ],
    },
    RuleSpec {
        kind: "number",
        hint: None,
        whole_name: false,
        options: &[
            o(
                "template",
                Kind::Template,
                Default::Lang(["{nome} {n}", "{name} {n}", "{nombre} {n}"]),
                "template",
                Some("templateHint"),
            ),
            o("start", NUMBER, Default::Int(1), "start", None),
            o("step", NUMBER, Default::Int(1), "step", None),
            o(
                "digits",
                Kind::Int { min: 0, max: 12 },
                Default::Int(2),
                "digits",
                None,
            ),
            o("perFolder", Kind::Bool, NO, "perFolder", None),
            o("pairs", Kind::Bool, YES, "pairs", Some("pairsHint")),
        ],
    },
    RuleSpec {
        kind: "remove",
        hint: None,
        whole_name: false,
        options: &[
            o("extraSpaces", Kind::Bool, YES, "rmExtraSpaces", None),
            o("allSpaces", Kind::Bool, NO, "rmAllSpaces", None),
            o("accents", Kind::Bool, NO, "rmAccents", None),
            o("digits", Kind::Bool, NO, "rmDigits", None),
            o(
                "leadingNumber",
                Kind::Bool,
                NO,
                "rmLeadingNumber",
                Some("rmLeadingNumberHint"),
            ),
            o("chars", Kind::Text, EMPTY, "chars", None),
            o("maxLength", COUNT, Default::Int(0), "maxLength", None),
            o("target", TARGET, Default::Text("name"), "applyTo", None),
        ],
    },
    RuleSpec {
        kind: "convert",
        hint: None,
        whole_name: false,
        options: &[
            o(
                "mojibake",
                Kind::Bool,
                YES,
                "cvMojibake",
                Some("cvMojibakeHint"),
            ),
            renamed("transliterate", "latin", Kind::Bool, NO, "cvLatin", None),
            o("splitWords", Kind::Bool, NO, "cvSplit", None),
            renamed(
                "webSafe",
                "web",
                Kind::Choice(&[
                    cw("", "no", "cvWebOff"),
                    cw("-", "dash", "cvWebDash"),
                    cw("_", "underscore", "cvWebUnderscore"),
                ]),
                EMPTY,
                "cvWeb",
                Some("cvWebHint"),
            ),
            o("target", TARGET, Default::Text("name"), "applyTo", None),
        ],
    },
    RuleSpec {
        kind: "removeRange",
        hint: None,
        whole_name: false,
        options: &[
            o("from", BOUND, Default::Text("text"), "from", None),
            o("fromPosition", POSITION, Default::Int(1), "position", None),
            o("fromText", Kind::Text, EMPTY, "text", None),
            o("to", BOUND, Default::Text("text"), "to", None),
            o("toPosition", POSITION, Default::Int(1), "position", None),
            o("toText", Kind::Text, EMPTY, "text", None),
            o("inclusive", Kind::Bool, YES, "inclusive", None),
        ],
    },
    RuleSpec {
        kind: "charAt",
        hint: None,
        whole_name: false,
        options: &[
            o("position", POSITION, Default::Int(1), "position", None),
            o("fromEnd", Kind::Bool, NO, "countFromEnd", None),
            o("onlyIf", Kind::Text, EMPTY, "onlyIf", None),
            o("caseSensitive", Kind::Bool, NO, "caseSensitive", None),
            o("replace", Kind::Text, EMPTY, "replaceBy", None),
        ],
    },
    RuleSpec {
        kind: "extension",
        hint: None,
        whole_name: false,
        options: &[
            o(
                "mode",
                Kind::Choice(&[
                    c("lower", "extLower"),
                    c("upper", "extUpper"),
                    c("set", "extSet"),
                    c("remove", "extRemove"),
                    c("content", "extContent"),
                ]),
                Default::Text("lower"),
                "action",
                Some("extContentHint"),
            ),
            o("value", Kind::Text, EMPTY, "newExt", None),
        ],
    },
    RuleSpec {
        kind: "movie",
        hint: Some("movieHint"),
        whole_name: true,
        options: &[
            o(
                "template",
                Kind::Template,
                Default::Lang([
                    "{título} ({ano}) [tmdbid-{id}]",
                    "{title} ({year}) [tmdbid-{id}]",
                    "{título} ({año}) [tmdbid-{id}]",
                ]),
                "template",
                Some("movieTemplateHint"),
            ),
            COLON,
            o("folder", Kind::Bool, NO, "folderLabel", Some("folderHint")),
            o(
                "trashAds",
                Kind::Bool,
                NO,
                "trashAdsLabel",
                Some("trashAdsHint"),
            ),
            renamed(
                "nfo",
                "sidecar",
                Kind::Bool,
                NO,
                "sidecarLabel",
                Some("sidecarHint"),
            ),
            renamed("backdrop", "fanart", Kind::Bool, NO, "fanartLabel", None),
            o(
                "videoTags",
                Kind::Bool,
                NO,
                "videoTagsLabel",
                Some("videoTagsHint"),
            ),
            o(
                "videoClean",
                Kind::Bool,
                NO,
                "videoCleanLabel",
                Some("videoCleanHint"),
            ),
            o(
                "videoTracks",
                Kind::Bool,
                NO,
                "videoTracksLabel",
                Some("videoTracksHint"),
            ),
            o(
                "missing",
                Kind::Bool,
                YES,
                "movieMissingLabel",
                Some("movieMissingHint"),
            ),
            o("subtitles", Kind::Bool, NO, "subsLabel", Some("subsHint")),
            o("subLangs", Kind::Text, SUB_LANGS, "subsLangs", None),
        ],
    },
    RuleSpec {
        kind: "series",
        hint: Some("seriesHint"),
        whole_name: true,
        options: &[
            o(
                "template",
                Kind::Template,
                Default::Lang([
                    "{série} ({ano}) - S{temporada}E{episódio} - {título}",
                    "{show} ({year}) - S{season}E{episode} - {title}",
                    "{serie} ({año}) - S{temporada}E{episodio} - {título}",
                ]),
                "template",
                Some("seriesTemplateHint"),
            ),
            COLON,
            o(
                "folders",
                Kind::Bool,
                NO,
                "seriesFoldersLabel",
                Some("seriesFoldersHint"),
            ),
            o(
                "folderTemplate",
                Kind::Template,
                Default::Lang([
                    "{série} ({ano}) [tmdbid-{id}]",
                    "{show} ({year}) [tmdbid-{id}]",
                    "{serie} ({año}) [tmdbid-{id}]",
                ]),
                "seriesFolderName",
                None,
            ),
            o(
                "source",
                Kind::Choice(&[
                    c("tmdb", "seriesSourceTmdb"),
                    c("anilist", "seriesSourceAnilist"),
                ]),
                Default::Text("tmdb"),
                "seriesSource",
                Some("animeHint"),
            ),
            o("show", Kind::Text, EMPTY, "seriesShow", None),
            renamed(
                "nfo",
                "sidecar",
                Kind::Bool,
                NO,
                "seriesSidecarLabel",
                Some("seriesSidecarHint"),
            ),
            renamed("backdrop", "fanart", Kind::Bool, NO, "fanartLabel", None),
            o(
                "videoTags",
                Kind::Bool,
                NO,
                "videoTagsLabel",
                Some("videoTagsHintTv"),
            ),
            o(
                "videoClean",
                Kind::Bool,
                NO,
                "videoCleanLabel",
                Some("videoCleanHint"),
            ),
            o(
                "videoTracks",
                Kind::Bool,
                NO,
                "videoTracksLabel",
                Some("videoTracksHint"),
            ),
            o(
                "missing",
                Kind::Bool,
                YES,
                "seriesMissingLabel",
                Some("seriesMissingHint"),
            ),
            o("subtitles", Kind::Bool, NO, "subsLabel", Some("subsHint")),
            o("subLangs", Kind::Text, SUB_LANGS, "subsLangs", None),
        ],
    },
    RuleSpec {
        kind: "music",
        hint: Some("musicHint"),
        whole_name: true,
        options: &[
            o(
                "template",
                Kind::Template,
                Default::Lang([
                    "{faixa} - {título}",
                    "{track} - {title}",
                    "{pista} - {título}",
                ]),
                "template",
                Some("musicTemplateHint"),
            ),
            COLON,
            o(
                "discPrefix",
                Kind::Bool,
                YES,
                "musicDiscPrefix",
                Some("musicDiscPrefixHint"),
            ),
            o(
                "fromName",
                Kind::Bool,
                NO,
                "musicFromName",
                Some("musicFromNameHint"),
            ),
            o(
                "folders",
                Kind::Bool,
                NO,
                "musicFolders",
                Some("musicFoldersHint"),
            ),
            o(
                "folderTemplate",
                Kind::Template,
                Default::Lang([
                    "{artista do álbum}/{álbum} ({ano})",
                    "{album artist}/{album} ({year})",
                    "{artista del álbum}/{álbum} ({año})",
                ]),
                "musicFolderName",
                None,
            ),
            o(
                "playlist",
                Kind::Bool,
                NO,
                "musicPlaylist",
                Some("musicPlaylistHint"),
            ),
            o(
                "cover",
                Kind::Bool,
                NO,
                "musicCover",
                Some("musicCoverHint"),
            ),
            renamed(
                "identifyBySound",
                "listen",
                Kind::Bool,
                YES,
                "musicListen",
                Some("musicListenHint"),
            ),
            o(
                "writeTags",
                Kind::Bool,
                NO,
                "musicWriteTags",
                Some("musicWriteTagsHint"),
            ),
            o(
                "embedCover",
                Kind::Bool,
                YES,
                "musicEmbedCover",
                Some("musicEmbedCoverHint"),
            ),
            o(
                "coverSize",
                Kind::IntChoice(&[1200, 500, 250]),
                Default::Int(1200),
                "musicCoverSize",
                None,
            ),
            o(
                "cleanTags",
                Kind::Bool,
                YES,
                "musicCleanTags",
                Some("musicCleanTagsHint"),
            ),
            o(
                "missing",
                Kind::Bool,
                YES,
                "musicMissingLabel",
                Some("musicMissingHint"),
            ),
        ],
    },
    RuleSpec {
        kind: "book",
        hint: Some("bookHint"),
        whole_name: true,
        options: &[
            o(
                "template",
                Kind::Template,
                Default::Lang([
                    "{autor} - {título} ({ano})",
                    "{author} - {title} ({year})",
                    "{autor} - {título} ({año})",
                ]),
                "template",
                Some("bookTemplateHint"),
            ),
            COLON,
            o(
                "folders",
                Kind::Bool,
                NO,
                "bookFolders",
                Some("bookFoldersHint"),
            ),
            o(
                "folderTemplate",
                Kind::Template,
                Default::Lang(["{autor}/{título}", "{author}/{title}", "{autor}/{título}"]),
                "musicFolderName",
                None,
            ),
            renamed(
                "audiobooks",
                "audio",
                Kind::Bool,
                YES,
                "bookAudio",
                Some("bookAudioHint"),
            ),
            o("cover", Kind::Bool, NO, "bookCover", Some("bookCoverHint")),
            renamed(
                "metadataOpf",
                "opf",
                Kind::Bool,
                NO,
                "bookOpf",
                Some("bookOpfHint"),
            ),
            o(
                "writeEpub",
                Kind::Bool,
                NO,
                "bookWriteEpub",
                Some("bookWriteEpubHint"),
            ),
            o("epub3", Kind::Bool, NO, "bookEpub3", Some("bookEpub3Hint")),
            o(
                "shrinkImages",
                Kind::Bool,
                NO,
                "bookShrink",
                Some("bookShrinkHint"),
            ),
            o(
                "audioTags",
                Kind::Bool,
                NO,
                "bookAudioTags",
                Some("bookAudioTagsHint"),
            ),
            o(
                "audioClean",
                Kind::Bool,
                NO,
                "bookAudioClean",
                Some("bookAudioCleanHint"),
            ),
            o(
                "convertTo",
                Kind::Choice(&[
                    cw("", "no", "bookConvertNone"),
                    c("epub", "comicEbookEpub"),
                    c("azw3", "bookConvertAzw3"),
                    c("fb2", "bookConvert"),
                    c("pdf", "comicEbookPdf"),
                ]),
                EMPTY,
                "bookConvert",
                Some("bookConvertHint"),
            ),
            o(
                "original",
                KEEP_DELETE_BOOK,
                Default::Text("keep"),
                "comicOriginal",
                None,
            ),
            o(
                "model",
                Kind::Text,
                EMPTY,
                "readerFor",
                Some("bookReaderHint"),
            ),
            renamed(
                "otherReaderApps",
                "anyProgram",
                Kind::Bool,
                NO,
                "comicAnyProgram",
                Some("comicAnyProgramHint"),
            ),
            o(
                "missing",
                Kind::Bool,
                YES,
                "bookMissingLabel",
                Some("bookMissingHint"),
            ),
        ],
    },
    RuleSpec {
        kind: "comic",
        hint: Some("comicHint"),
        whole_name: true,
        options: &[
            o(
                "template",
                Kind::Template,
                Default::Lang([
                    "{série} v{volume} #{número} c{capítulo} - {título} ({ano})",
                    "{series} v{volume} #{number} c{chapter} - {title} ({year})",
                    "{serie} v{volumen} #{número} c{capítulo} - {título} ({año})",
                ]),
                "template",
                Some("comicTemplateHint"),
            ),
            COLON,
            o(
                "source",
                Kind::Choice(&[
                    c("gcd", "comicSourceGcd"),
                    c("comicvine", "comicSourceCv"),
                    c("metron", "comicSourceMetron"),
                    c("mangadex", "comicSourceMangadex"),
                    c("anilist", "comicSourceAnilist"),
                    c("mangaupdates", "comicSourceMangaupdates"),
                ]),
                Default::Text("gcd"),
                "comicSource",
                None,
            ),
            o("series", Kind::Text, EMPTY, "comicSeries", None),
            renamed(
                "seriesByFolder",
                "byFolder",
                Kind::Bool,
                NO,
                "comicByFolder",
                Some("comicByFolderHint"),
            ),
            o(
                "folders",
                Kind::Bool,
                NO,
                "comicFolders",
                Some("comicFoldersHint"),
            ),
            o(
                "folderTemplate",
                Kind::Template,
                Default::Lang([
                    "{série} ({ano da série})",
                    "{series} ({series year})",
                    "{serie} ({año de la serie})",
                ]),
                "musicFolderName",
                None,
            ),
            o(
                "barcode",
                Kind::Bool,
                NO,
                "comicBarcodeLabel",
                Some("comicBarcodeHint"),
            ),
            o(
                "skipPublishers",
                Kind::Text,
                EMPTY,
                "comicSkipPublishers",
                Some("comicSkipPublishersHint"),
            ),
            o(
                "writeInfo",
                Kind::Bool,
                NO,
                "comicWriteInfo",
                Some("comicWriteInfoHint"),
            ),
            o(
                "cleanInfo",
                Kind::Bool,
                NO,
                "comicCleanInfo",
                Some("comicCleanInfoHint"),
            ),
            o(
                "convert",
                Kind::Bool,
                NO,
                "comicConvert",
                Some("comicConvertHint"),
            ),
            o(
                "original",
                KEEP_DELETE_COMIC,
                Default::Text("keep"),
                "comicOriginal",
                None,
            ),
            renamed(
                "drawPdfPages",
                "drawPages",
                Kind::Bool,
                YES,
                "comicDrawPages",
                Some("comicDrawPagesHint"),
            ),
            o(
                "pageSize",
                Kind::IntChoice(&[1600, 2400, 3200]),
                Default::Int(2400),
                "comicPageSize",
                Some("comicPageSizeHint"),
            ),
            o(
                "ebook",
                Kind::Choice(&[
                    cw("", "no", "comicEbookNone"),
                    c("epub", "comicEbookEpub"),
                    c("kepub", "comicEbookKepub"),
                    c("azw3", "comicEbookAzw3"),
                    c("cbz", "comicEbookCbz"),
                    c("pdf", "comicEbookPdf"),
                ]),
                EMPTY,
                "comicEbook",
                Some("comicEbookHint"),
            ),
            o(
                "device",
                Kind::Choice(DEVICES),
                Default::Text("kindle-pw"),
                "comicDevice",
                None,
            ),
            o("model", Kind::Text, EMPTY, "comicModel", None),
            renamed(
                "otherReaderApps",
                "anyProgram",
                Kind::Bool,
                NO,
                "comicAnyProgram",
                Some("comicAnyProgramHint"),
            ),
            o(
                "width",
                Kind::Int {
                    min: 100,
                    max: 10_000,
                },
                Default::Int(1264),
                "comicWidth",
                None,
            ),
            o(
                "height",
                Kind::Int {
                    min: 100,
                    max: 10_000,
                },
                Default::Int(1680),
                "comicHeight",
                None,
            ),
            o("crop", Kind::Bool, YES, "comicCrop", Some("comicCropHint")),
            renamed(
                "cropPageNumbers",
                "numbers",
                Kind::Bool,
                NO,
                "comicNumbers",
                Some("comicNumbersHint"),
            ),
            o(
                "spread",
                Kind::Choice(&[
                    c("keep", "comicSpread_keep"),
                    c("split", "comicSpread_split"),
                    c("rotate", "comicSpread_rotate"),
                    c("both", "comicSpread_both"),
                ]),
                Default::Text("split"),
                "comicSpread",
                Some("comicSpreadHint"),
            ),
            o(
                "contrast",
                Kind::Bool,
                YES,
                "comicContrast",
                Some("comicContrastHint"),
            ),
            o(
                "upscale",
                Kind::Bool,
                NO,
                "comicUpscale",
                Some("comicUpscaleHint"),
            ),
            renamed(
                "sixteenGreys",
                "greys16",
                Kind::Bool,
                NO,
                "comicGreys16",
                Some("comicGreys16Hint"),
            ),
            renamed(
                "reduceRainbow",
                "soften",
                Kind::Bool,
                NO,
                "comicSoften",
                Some("comicSoftenHint"),
            ),
            o(
                "webtoon",
                Kind::Bool,
                NO,
                "comicWebtoon",
                Some("comicWebtoonHint"),
            ),
            o("join", Kind::Bool, NO, "comicJoin", Some("comicJoinHint")),
            o(
                "maxMb",
                Kind::IntChoice(&[0, 50, 200]),
                Default::Int(0),
                "comicMaxMb",
                Some("comicMaxMbHint"),
            ),
            o(
                "missing",
                Kind::Bool,
                YES,
                "comicMissingLabel",
                Some("comicMissingHint"),
            ),
        ],
    },
    RuleSpec {
        kind: "photo",
        hint: Some("photoHint"),
        whole_name: true,
        options: &[
            o(
                "template",
                Kind::Template,
                Default::Lang([
                    "{ano}-{mês}-{dia} {hora}.{minuto}.{segundo}",
                    "{year}-{month}-{day} {hour}.{minute}.{second}",
                    "{año}-{mes}-{día} {hora}.{minuto}.{segundo}",
                ]),
                "template",
                Some("photoTemplateHint"),
            ),
            o(
                "fromName",
                Kind::Bool,
                YES,
                "photoFromName",
                Some("photoFromNameHint"),
            ),
            o(
                "modified",
                Kind::Bool,
                NO,
                "photoModified",
                Some("photoModifiedHint"),
            ),
            o(
                "shiftHours",
                Kind::Int { min: -48, max: 48 },
                Default::Int(0),
                "photoShift",
                Some("photoShiftHint"),
            ),
            o(
                "folders",
                Kind::Bool,
                NO,
                "photoFolders",
                Some("photoFoldersHint"),
            ),
            o(
                "folderTemplate",
                Kind::Template,
                Default::Lang([
                    "{ano}/{ano}-{mês}",
                    "{year}/{year}-{month}",
                    "{año}/{año}-{mes}",
                ]),
                "musicFolderName",
                None,
            ),
            o(
                "setModified",
                Kind::Bool,
                NO,
                "photoSetModified",
                Some("photoSetModifiedHint"),
            ),
        ],
    },
    RuleSpec {
        kind: "subtitle",
        hint: Some("subToolsHint"),
        whole_name: false,
        options: &[
            o("toSrt", Kind::Bool, NO, "subToSrt", Some("subToSrtHint")),
            o(
                "original",
                KEEP_DELETE_SUB,
                Default::Text("keep"),
                "comicOriginal",
                None,
            ),
            o("utf8", Kind::Bool, YES, "subUtf8", Some("subUtf8Hint")),
            o("ads", Kind::Bool, NO, "subAds", Some("subAdsHint")),
            o(
                "hearing",
                Kind::Bool,
                NO,
                "subHearing",
                Some("subHearingHint"),
            ),
            o(
                "shiftMs",
                NUMBER,
                Default::Int(0),
                "subShift",
                Some("subShiftHint"),
            ),
            o(
                "fpsFrom",
                Kind::Float,
                Default::Float(0.0),
                "subFps",
                Some("subFpsHint"),
            ),
            o("fpsTo", Kind::Float, Default::Float(0.0), "subFps", None),
            o(
                "extract",
                Kind::Bool,
                NO,
                "subExtract",
                Some("subExtractHint"),
            ),
            o("extractLangs", Kind::Text, EMPTY, "subExtractLangs", None),
        ],
    },
];

/// The rule of this type, by its name (camelCase or kebab-case).
pub fn rule(kind: &str) -> Option<&'static RuleSpec> {
    let want = squash(kind);
    RULES.iter().find(|r| squash(r.kind) == want)
}

/// An option's default in a language (`pt`, `en`, `es`).
pub fn default_value(opt: &Opt, lang: &str) -> Value {
    match opt.default {
        Default::Text(t) => Value::from(t),
        Default::Lang(by) => Value::from(by[LANGS.iter().position(|l| *l == lang).unwrap_or(1)]),
        Default::Bool(b) => Value::from(b),
        Default::Int(n) => Value::from(n),
        Default::Float(f) => Value::from(f),
    }
}

/// A new rule of this type with every option at its default.
pub fn new_rule(spec: &RuleSpec, lang: &str) -> Map<String, Value> {
    let mut rule = Map::new();
    rule.insert("type".into(), Value::from(spec.kind));
    for opt in spec.options {
        rule.insert(opt.name.into(), default_value(opt, lang));
    }
    rule
}

/// Brings a saved rule (the `rule` of a step) up to date: options saved
/// under their old names take the new ones (a new name already there
/// wins), and the comic's old `split` becomes `spread`.
pub fn migrate_rule(rule: &mut Map<String, Value>) {
    let Some(spec) = rule
        .get("type")
        .and_then(Value::as_str)
        .and_then(self::rule)
    else {
        return;
    };
    for opt in spec.options {
        if let Some(old) = opt.old
            && let Some(v) = rule.remove(old)
        {
            rule.entry(opt.name).or_insert(v);
        }
    }
    if spec.kind == "comic"
        && let Some(split) = rule.remove("split")
    {
        let spread = if split == Value::Bool(false) {
            "keep"
        } else {
            "split"
        };
        rule.entry("spread").or_insert(Value::from(spread));
    }
}

/// [`migrate_rule`] for a step (`{id, enabled, rule, only}`) or a bare rule.
pub fn migrate_step(step: &mut Value) {
    let Some(obj) = step.as_object_mut() else {
        return;
    };
    if let Some(Value::Object(rule)) = obj.get_mut("rule") {
        migrate_rule(rule);
    } else if obj.contains_key("type") {
        migrate_rule(obj);
    }
}

/// [`migrate_step`] for every step of a list, of rule sets
/// (`[{name, steps}]`) or of a saved preset file.
pub fn migrate_steps(value: &mut Value) {
    match value {
        Value::Array(items) => {
            for item in items {
                if let Some(steps) = item.get_mut("steps") {
                    migrate_steps(steps);
                } else {
                    migrate_step(item);
                }
            }
        }
        Value::Object(obj) => {
            if let Some(steps) = obj.get_mut("steps") {
                migrate_steps(steps);
            } else if obj.contains_key("rule") || obj.contains_key("type") {
                migrate_step(value);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_found_by_new_old_and_kebab_names() {
        let movie = rule("movie").unwrap();
        assert_eq!(movie.option("nfo").unwrap().name, "nfo");
        assert_eq!(movie.option("sidecar").unwrap().name, "nfo");
        assert_eq!(movie.option("trash-ads").unwrap().name, "trashAds");
        assert_eq!(movie.option("SUB_LANGS").unwrap().name, "subLangs");
        assert!(movie.option("nothing").is_none());
        assert_eq!(rule("remove-range").unwrap().kind, "removeRange");
    }

    #[test]
    fn names_and_old_names_are_unique_in_each_rule() {
        for r in RULES {
            let mut seen = std::collections::HashSet::new();
            for o in r.options {
                assert!(seen.insert(squash(o.name)), "{} {}", r.kind, o.name);
                if let Some(old) = o.old {
                    assert!(seen.insert(squash(old)), "{} {}", r.kind, old);
                }
            }
        }
    }

    #[test]
    fn defaults_fit_their_kinds() {
        for r in RULES {
            for o in r.options {
                for lang in LANGS {
                    let v = default_value(o, lang);
                    match o.kind {
                        Kind::Bool => assert!(v.is_boolean(), "{} {}", r.kind, o.name),
                        Kind::Int { min, max } => {
                            let n = v.as_i64().unwrap();
                            assert!(n >= min && n <= max, "{} {}", r.kind, o.name);
                        }
                        Kind::IntChoice(list) => {
                            assert!(list.contains(&v.as_i64().unwrap()), "{} {}", r.kind, o.name)
                        }
                        Kind::Float => assert!(v.as_f64().is_some()),
                        Kind::Choice(list) => {
                            assert!(
                                list.iter().any(|c| c.value == v.as_str().unwrap()),
                                "{} {}",
                                r.kind,
                                o.name
                            )
                        }
                        Kind::Text | Kind::Template | Kind::Lines => assert!(v.is_string()),
                    }
                }
            }
        }
    }

    #[test]
    fn new_rules_are_read_by_the_core() {
        for r in RULES {
            let v = Value::Object(new_rule(r, "en"));
            let parsed: Result<crate::Rule, _> = serde_json::from_value(v);
            assert!(parsed.is_ok(), "{}: {:?}", r.kind, parsed.err());
        }
    }

    #[test]
    fn old_names_brought_up_to_date() {
        let mut v = serde_json::json!([
            {"name": "set", "steps": [{"id": "a", "enabled": true, "rule": {"type": "movie", "sidecar": true, "fanart": true, "colon": " "}}]},
        ]);
        migrate_steps(&mut v);
        let rule = &v[0]["steps"][0]["rule"];
        assert_eq!(rule["nfo"], true);
        assert_eq!(rule["backdrop"], true);
        assert_eq!(rule["colonAs"], " ");
        assert!(rule.get("sidecar").is_none());

        let mut c = serde_json::json!({"type": "comic", "split": false, "greys16": true});
        migrate_step(&mut c);
        assert_eq!(c["spread"], "keep");
        assert_eq!(c["sixteenGreys"], true);
        // A new name already there wins over the old one.
        let mut both = serde_json::json!({"type": "case", "keep": "old", "keepWords": "new"});
        migrate_step(&mut both);
        assert_eq!(both["keepWords"], "new");
    }
}
