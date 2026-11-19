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
                "trashAds",
                Kind::Bool,
                NO,
                "trashAdsLabel",
                Some("trashAdsHint"),
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

/// The rules that make the whole name for a kind of file (looked up in a
/// catalogue, or from the photo's date; `whole_name`): one at a time,
/// since each would undo the other's names (a photo rule also names
/// videos).
pub fn kind_rules() -> Vec<&'static str> {
    RULES
        .iter()
        .filter(|r| r.whole_name)
        .map(|r| r.kind)
        .collect()
}

fn is_kind(kind: &str) -> bool {
    rule(kind).is_some_and(|r| r.whole_name)
}
/// Rules that make sense once in the list.
const ONCE: &[&str] = &["subtitle"];
/// Kind rules whose files are not videos: the subtitle rule has nothing to
/// do beside them (and they nothing beside it).
const NOT_VIDEO: &[&str] = &["music", "book", "comic", "photo"];

/// Why a rule cannot join the rules there (`types`), the same for the
/// window's menu and the command line.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(untagged)]
pub enum Blocked {
    /// Another kind rule is there (named).
    Kind { kind: String },
    /// The same rule, allowed once, is there.
    Twice { twice: bool },
    /// A rule it does not go with is there (named).
    Clash { clash: String },
}

pub fn blocked_by(types: &[&str], new: &str) -> Option<Blocked> {
    let kind = types.iter().find(|t| is_kind(t));
    if let Some(k) = kind
        && is_kind(new)
    {
        return Some(Blocked::Kind {
            kind: k.to_string(),
        });
    }
    if ONCE.contains(&new) && types.contains(&new) {
        return Some(Blocked::Twice { twice: true });
    }
    if let Some(k) = kind
        && new == "subtitle"
        && NOT_VIDEO.contains(k)
    {
        return Some(Blocked::Clash {
            clash: k.to_string(),
        });
    }
    if NOT_VIDEO.contains(&new) && types.contains(&"subtitle") {
        return Some(Blocked::Clash {
            clash: "subtitle".into(),
        });
    }
    None
}

/// Steps with only their first kind rule (and one of each rule allowed
/// once); the types of the others, left out (saved before they were one
/// at a time).
pub fn one_lookup(steps: Vec<Value>) -> (Vec<Value>, Vec<String>) {
    let mut kept: Vec<Value> = Vec::new();
    let mut dropped = Vec::new();
    let type_of = |s: &Value| {
        s.get("rule")
            .and_then(|r| r.get("type"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    for s in steps {
        let t = type_of(&s);
        let there: Vec<String> = kept.iter().map(type_of).collect();
        let there: Vec<&str> = there.iter().map(String::as_str).collect();
        if blocked_by(&there, &t).is_some() {
            dropped.push(t);
        } else {
            kept.push(s);
        }
    }
    (kept, dropped)
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
    // Rules from before there was a choice of comic catalogue were on
    // Comic Vine; their default template had no title yet.
    // (Only those: the old template is also Kavita's ready-made one.)
    if spec.kind == "comic" && rule.contains_key("template") && !rule.contains_key("source") {
        rule.insert("source".into(), Value::from("comicvine"));
        let old = rule.get("template").and_then(Value::as_str).and_then(|t| {
            OLD_COMIC_TEMPLATES
                .iter()
                .find(|(was, _)| *was == t)
                .map(|(_, now)| *now)
        });
        if let Some(now) = old {
            rule.insert("template".into(), Value::from(now));
        }
    }
    // Counting a video and its subtitles as one is new rules' default; a
    // rule saved before it counts them apart.
    if matches!(spec.kind, "number" | "list") {
        rule.entry("pairs").or_insert(Value::Bool(false));
    }
}

/// The comic rule's old default templates (unchanged by the person), and
/// the ones that replaced them.
const OLD_COMIC_TEMPLATES: [(&str, &str); 3] = [
    (
        "{série} v{volume} #{número} c{capítulo} ({ano})",
        "{série} v{volume} #{número} c{capítulo} - {título} ({ano})",
    ),
    (
        "{series} v{volume} #{number} c{chapter} ({year})",
        "{series} v{volume} #{number} c{chapter} - {title} ({year})",
    ),
    (
        "{serie} v{volumen} #{número} c{capítulo} ({año})",
        "{serie} v{volumen} #{número} c{capítulo} - {título} ({año})",
    ),
];

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

/// The rules that look files up in a catalogue.
pub const LOOKUPS: [&str; 5] = ["movie", "series", "music", "book", "comic"];

/// Rules that work on a kind of file (the "+ Rule" menu's first group,
/// the manual's): those that make the whole name, and the subtitle rule.
pub fn by_kind(kind: &str) -> bool {
    is_kind(kind) || kind == "subtitle"
}

/// Rule types, those by kind of file first.
pub fn groups() -> (Vec<&'static str>, Vec<&'static str>) {
    let (media, text): (Vec<&RuleSpec>, Vec<&RuleSpec>) =
        RULES.iter().partition(|r| by_kind(r.kind));
    (
        media.iter().map(|r| r.kind).collect(),
        text.iter().map(|r| r.kind).collect(),
    )
}

/// A value of a step's rule as the core reads it: numbers within the
/// option's bounds (an empty field is 0, a whole number has no decimals),
/// a number with decimals not below 0, a choice among its values.
pub fn normalize_rule(rule: &mut Map<String, Value>) {
    let Some(spec) = rule
        .get("type")
        .and_then(Value::as_str)
        .and_then(self::rule)
    else {
        return;
    };
    for opt in spec.options {
        let Some(v) = rule.get_mut(opt.name) else {
            continue;
        };
        let number = v.as_f64().filter(|n| n.is_finite());
        match opt.kind {
            Kind::Int { min, max } => {
                let n = number.map_or(0, |n| n.trunc() as i64);
                *v = Value::from(n.clamp(min, max));
            }
            Kind::IntChoice(list) => {
                let n = number.map(|n| n.trunc() as i64);
                if !n.is_some_and(|n| list.contains(&n)) {
                    *v = default_value(opt, "en");
                } else if let Some(n) = n {
                    *v = Value::from(n);
                }
            }
            Kind::Float => *v = Value::from(number.unwrap_or(0.0).max(0.0)),
            Kind::Bool if v.is_null() => *v = Value::Bool(false),
            Kind::Text | Kind::Template | Kind::Lines if v.is_null() => *v = Value::from(""),
            _ => {}
        }
    }
}

/// [`normalize_rule`] for each step's rule.
pub fn normalize_steps(steps: &mut [Value]) {
    for s in steps {
        if let Some(Value::Object(rule)) = s.get_mut("rule") {
            normalize_rule(rule);
        }
    }
}

/// A saved rule brought up to date and whole: old option names updated
/// ([`migrate_rule`]), the options it leaves out at their defaults in
/// this language.
pub fn complete_rule(mut rule: Map<String, Value>, lang: &str) -> Option<Map<String, Value>> {
    migrate_rule(&mut rule);
    let spec = rule
        .get("type")
        .and_then(Value::as_str)
        .and_then(self::rule)?;
    let mut full = new_rule(spec, lang);
    full.extend(rule);
    normalize_rule(&mut full);
    Some(full)
}

/// Saved steps (remembered from the last session, a rule set, a preset
/// file) made ready to use: with their first kind rule only (the types
/// left out are returned), each rule whole ([`complete_rule`]; unknown
/// types left out too) and an id. `saved`: a set or file whose rules
/// are all off was saved like that by "turn the rules off after
/// renaming", not by choice: they come back on.
pub fn complete_steps(mut steps: Vec<Value>, lang: &str, saved: bool) -> (Vec<Value>, Vec<String>) {
    for s in steps.iter_mut() {
        migrate_step(s);
    }
    let (kept, mut dropped) = one_lookup(steps);
    let mut out = Vec::new();
    for (n, s) in kept.into_iter().enumerate() {
        let Value::Object(mut step) = s else { continue };
        let rule = match step.remove("rule") {
            Some(Value::Object(r)) => r,
            _ => continue,
        };
        let kind = rule
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let Some(rule) = complete_rule(rule, lang) else {
            dropped.push(kind);
            continue;
        };
        step.insert("rule".into(), Value::Object(rule));
        if !step.get("enabled").is_some_and(Value::is_boolean) {
            step.insert("enabled".into(), Value::Bool(true));
        }
        if !step
            .get("id")
            .is_some_and(|v| v.as_str().is_some_and(|s| !s.is_empty()))
        {
            step.insert("id".into(), Value::from(step_id(n)));
        }
        out.push(Value::Object(step));
    }
    if saved && !out.is_empty() && out.iter().all(|s| s["enabled"] == Value::Bool(false)) {
        for s in out.iter_mut() {
            s["enabled"] = Value::Bool(true);
        }
    }
    (out, dropped)
}

/// The steps in a JSON file: a list of steps, a set (`{name, steps}`), or
/// a list of sets (the first).
pub fn steps_of(v: Value) -> Value {
    match v {
        Value::Object(mut o) => o.remove("steps").unwrap_or(Value::Array(vec![])),
        Value::Array(items) if items.first().is_some_and(|i| i.get("steps").is_some()) => items
            .into_iter()
            .next()
            .and_then(|mut s| s.get_mut("steps").map(Value::take))
            .unwrap_or_default(),
        other => other,
    }
}

/// A step id, different for each step made in a run.
pub fn step_id(n: usize) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static RUN: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default();
    format!("s{now:x}{}{n}", RUN.fetch_add(1, Ordering::Relaxed))
}

/// Whether the release's advertisements go to the Trash: asked by the
/// movie or TV show rule that is on.
pub fn trash_ads(steps: &[Value]) -> bool {
    steps.iter().any(|s| {
        s["enabled"] != Value::Bool(false)
            && matches!(s["rule"]["type"].as_str(), Some("movie" | "series"))
            && s["rule"]["trashAds"].as_bool() == Some(true)
    })
}

/// The list's optional columns that make sense with the rules there (on
/// or off): a song's tags with the music rule, a video's picture with the
/// movie and TV show rules, the date taken when a rule inserts it.
pub fn columns_for(steps: &[Value]) -> Vec<&'static str> {
    let types: Vec<&str> = steps
        .iter()
        .filter_map(|s| s["rule"]["type"].as_str())
        .collect();
    let lookup = types.iter().find(|t| is_kind(t)).copied();
    let mut cols = Vec::new();
    if lookup == Some("music") {
        cols.extend([
            "artist", "album", "title", "track", "year", "genre", "length", "quality",
        ]);
    }
    if matches!(lookup, Some("movie" | "series")) {
        cols.extend(["resolution", "codec"]);
    }
    if steps
        .iter()
        .any(|s| s["rule"]["type"] == "insert" && s["rule"]["source"] == "taken")
    {
        cols.push("taken");
    }
    cols
}

/// Every optional column, in the list's order.
pub const COLUMNS: [&str; 11] = [
    "artist",
    "album",
    "title",
    "track",
    "year",
    "genre",
    "length",
    "quality",
    "resolution",
    "codec",
    "taken",
];

/// Frame rates the subtitle rule retimes between: the subtitle's, the
/// video's.
pub const FPS_PAIRS: [(f64, f64); 8] = [
    (23.976, 25.0),
    (25.0, 23.976),
    (24.0, 25.0),
    (25.0, 24.0),
    (23.976, 24.0),
    (24.0, 23.976),
    (29.97, 25.0),
    (25.0, 29.97),
];

/// Ready-made comic templates (name, folder), per language: Kavita reads
/// `Series v01 #001 c001`; a title after " - " is not in its examples,
/// so it is left out. Renome's own are the rule's defaults.
pub const KAVITA_TEMPLATES: [(&str, &str); 3] = [
    (
        "{série} v{volume} #{número} c{capítulo} ({ano})",
        "{série} ({ano da série})",
    ),
    (
        "{series} v{volume} #{number} c{chapter} ({year})",
        "{series} ({series year})",
    ),
    (
        "{serie} v{volumen} #{número} c{capítulo} ({año})",
        "{serie} ({año de la serie})",
    ),
];

/// The photo rule's ready templates per language: names, then folders.
pub const PHOTO_TEMPLATES: [([&str; 4], [&str; 4]); 3] = [
    (
        [
            "{ano}-{mês}-{dia} {hora}.{minuto}.{segundo}",
            "{ano}{mês}{dia}_{hora}{minuto}{segundo}",
            "{ano}-{mês}-{dia} {hora}.{minuto}.{segundo} ({câmera})",
            "{ano}-{mês}-{dia} - {nome}",
        ],
        [
            "{ano}/{ano}-{mês}",
            "{ano}/{mês} - {nome do mês}",
            "{ano}/{ano}-{mês}-{dia}",
            "{ano}",
        ],
    ),
    (
        [
            "{year}-{month}-{day} {hour}.{minute}.{second}",
            "{year}{month}{day}_{hour}{minute}{second}",
            "{year}-{month}-{day} {hour}.{minute}.{second} ({camera})",
            "{year}-{month}-{day} - {name}",
        ],
        [
            "{year}/{year}-{month}",
            "{year}/{month} - {month name}",
            "{year}/{year}-{month}-{day}",
            "{year}",
        ],
    ),
    (
        [
            "{año}-{mes}-{día} {hora}.{minuto}.{segundo}",
            "{año}{mes}{día}_{hora}{minuto}{segundo}",
            "{año}-{mes}-{día} {hora}.{minuto}.{segundo} ({cámara})",
            "{año}-{mes}-{día} - {nombre}",
        ],
        [
            "{año}/{año}-{mes}",
            "{año}/{mes} - {nombre del mes}",
            "{año}/{año}-{mes}-{día}",
            "{año}",
        ],
    ),
];

/// A language's place in the per-language tables (English when unknown).
pub fn lang_index(lang: &str) -> usize {
    LANGS.iter().position(|l| *l == lang).unwrap_or(1)
}

/// Everything the window builds rules and their menus from, in a
/// language: each rule new, the rules by kind of file, the groups of the
/// "+ Rule" menu, the ready-made templates, the frame rates, the columns.
pub fn table(lang: &str) -> Value {
    let defaults: Map<String, Value> = RULES
        .iter()
        .map(|spec| (spec.kind.to_string(), Value::Object(new_rule(spec, lang))))
        .collect();
    let i = lang_index(lang);
    let (media, text) = groups();
    let (kavita, kavita_folder) = KAVITA_TEMPLATES[i];
    serde_json::json!({
        "defaults": defaults,
        "kinds": kind_rules(),
        "lookups": LOOKUPS,
        "media": media,
        "text": text,
        "comicPresets": {
            "renome": {
                "template": defaults["comic"]["template"],
                "folderTemplate": defaults["comic"]["folderTemplate"],
            },
            "kavita": { "template": kavita, "folderTemplate": kavita_folder },
        },
        "photoTemplates": { "names": PHOTO_TEMPLATES[i].0, "folders": PHOTO_TEMPLATES[i].1 },
        "fpsPairs": FPS_PAIRS,
        "columns": COLUMNS,
    })
}

#[cfg(test)]
mod tests {

    #[test]
    fn saved_rules_come_whole_and_as_the_core_reads_them() {
        use serde_json::json;
        let step = |rule: Value, enabled: bool| json!({ "enabled": enabled, "rule": rule });
        let (steps, dropped) = complete_steps(
            vec![
                // A frame rate keeps its decimals; an empty number is 0.
                step(
                    json!({ "type": "subtitle", "fpsFrom": 23.976, "fpsTo": 25, "shiftMs": null }),
                    false,
                ),
                // A list rule saved before "pairs" counts them apart.
                step(json!({ "type": "list", "names": "a" }), false),
                // Subtitle tools go with videos, not with songs.
                step(json!({ "type": "music" }), false),
            ],
            "en",
            true,
        );
        assert_eq!(dropped, vec!["music".to_string()]);
        let r = |i: usize| &steps[i]["rule"];
        assert_eq!(r(0)["fpsFrom"], 23.976);
        assert_eq!(r(0)["shiftMs"], 0);
        assert_eq!(r(1)["pairs"], false);
        // All off when saved (turned off after renaming): back on.
        assert!(steps.iter().all(|s| s["enabled"] == true));
        // A comic rule saved before the catalogue choice was on Comic
        // Vine, its old template updated…
        let (old, _) = complete_steps(
            vec![step(
                json!({ "type": "comic", "template": "{series} v{volume} #{number} c{chapter} ({year})" }),
                true,
            )],
            "en",
            false,
        );
        assert_eq!(old[0]["rule"]["source"], "comicvine");
        assert_eq!(
            old[0]["rule"]["template"],
            "{series} v{volume} #{number} c{chapter} - {title} ({year})"
        );
        // …but the same template chosen later (Kavita's) stays.
        let (kavita, _) = complete_steps(
            vec![step(
                json!({ "type": "comic", "source": "gcd", "template": KAVITA_TEMPLATES[1].0 }),
                true,
            )],
            "en",
            false,
        );
        assert_eq!(kavita[0]["rule"]["template"], KAVITA_TEMPLATES[1].0);
    }

    #[test]
    fn numbers_within_bounds() {
        let mut r: Map<String, Value> =
            serde_json::from_value(serde_json::json!({ "type": "photo", "shiftHours": 99.7 }))
                .unwrap();
        normalize_rule(&mut r);
        assert_eq!(r["shiftHours"], 48);
    }

    #[test]
    fn one_kind_rule_at_a_time() {
        assert_eq!(
            blocked_by(&["replace", "movie"], "music"),
            Some(Blocked::Kind {
                kind: "movie".into()
            })
        );
        assert_eq!(
            blocked_by(&["subtitle"], "subtitle"),
            Some(Blocked::Twice { twice: true })
        );
        assert_eq!(
            blocked_by(&["music"], "subtitle"),
            Some(Blocked::Clash {
                clash: "music".into()
            })
        );
        assert_eq!(blocked_by(&["movie"], "subtitle"), None);
        let step = |t: &str| serde_json::json!({ "rule": { "type": t } });
        let (kept, dropped) = one_lookup(vec![step("movie"), step("replace"), step("series")]);
        assert_eq!(kept.len(), 2);
        assert_eq!(dropped, ["series"]);
    }
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
