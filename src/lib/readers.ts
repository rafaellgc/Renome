// The readers comics can be made for, one entry per model, each pointing
// at the screen it shares with others (`ComicDevice`, what the converter
// knows) and saying which e-book formats it opens as it comes. Grouped by
// brand in alphabetical order; within a brand, by line in alphabetical
// order and, in each line, newest first.

import type { ComicDevice, ComicFormat } from "./types";

/** A format a reader may open: the comic rule's and the book rule's. */
export type Format = Exclude<ComicFormat, ""> | "fb2";

export type Reader = {
  /** Kept in the rule as `model` (to show which model was chosen). */
  id: string;
  /** The model's name (not translated; `@key`: an i18n key). */
  name: string;
  device: ComicDevice;
  /** Formats it opens, when not its brand's. */
  formats?: Format[];
};

/** A screen as the converter fits pages into it: size, and whether it is
 *  grey ink, colour ink or a lit colour screen (tablets, phones). */
type Screen = { size: [number, number]; ink: "grey" | "colour" | "lcd"; ppi: number };

const s = (w: number, h: number, ink: Screen["ink"] = "grey", ppi = 300): Screen => ({ size: [w, h], ink, ppi });

export const SCREENS: Partial<Record<ComicDevice, Screen>> = {
  kindle: s(1072, 1448),
  "kindle-pw": s(1236, 1648),
  "kindle-pw2024": s(1272, 1696),
  "kindle-oasis": s(1264, 1680),
  "kindle-pw-old": s(758, 1024, "grey", 212),
  "kindle-basic": s(600, 800, "grey", 167),
  "kindle-dx": s(824, 1000, "grey", 150),
  "kindle-scribe": s(1860, 2480),
  "kindle-scribe2025": s(1986, 2648),
  "kindle-color": s(1272, 1696, "colour"),
  "kindle-scribe-color": s(1986, 2648, "colour"),
  kobo: s(1072, 1448),
  "kobo-clara-color": s(1072, 1448, "colour"),
  "kobo-libra": s(1264, 1680),
  "kobo-color": s(1264, 1680, "colour"),
  "kobo-h2o": s(1080, 1430, "grey", 265),
  "kobo-sage": s(1440, 1920),
  // 7.8" (300) or 10.3" (227) screens: in between.
  "kobo-elipsa": s(1404, 1872, "grey", 260),
  "kobo-nia": s(758, 1024, "grey", 212),
  remarkable: s(1404, 1872, "grey", 226),
  "remarkable-pro": s(1620, 2160, "colour", 229),
  "remarkable-move": s(954, 1696, "colour", 264),
  eink480: s(480, 800, "grey", 220),
  eink528: s(528, 792, "grey", 259),
  eink824: s(824, 1648),
  colour824: s(824, 1648, "colour"),
  colour1404: s(1404, 1872, "colour", 260),
  eink1650: s(1650, 2200, "grey", 207),
  eink1860: s(1860, 2480),
  colour1860: s(1860, 2480, "colour"),
  eink1920: s(1920, 2560),
  eink2400: s(2400, 3200),
  colour2400: s(2400, 3200, "colour"),
  tablet: s(1600, 2560, "lcd", 270),
  ipad: s(1640, 2360, "lcd", 264),
  ipad13: s(2048, 2732, "lcd", 264),
  phone: s(1080, 2400, "lcd", 400),
};

/** Colour ink screens, where fine dot patterns shimmer in colour. */
export const isColourInk = (d: ComicDevice) => SCREENS[d]?.ink === "colour";
/** Grey ink screens (pages saved in grey). */
export const isGreyInk = (d: ComicDevice) => SCREENS[d]?.ink === "grey";

const r = (id: string, name: string, device: ComicDevice, formats?: Format[]): Reader => ({ id, name, device, formats });

/** What Android readers, tablets and phones open (with a reading app). */
const APPS: Format[] = ["epub", "cbz", "pdf", "azw3", "fb2"];

/** Brand (an i18n key), the formats its readers open as they come, and
 *  its models. */
export const READERS: [string, Format[], Reader[]][] = [
  [
    "comicDevices_boox",
    APPS,
    [
      r("boox-go-10-gen2", "Boox Go 10.3 Gen II (2026)", "eink1860"),
      r("boox-go-10", "Boox Go 10.3 (2024)", "eink1860"),
      r("boox-go-6-gen2", "Boox Go 6 Gen II (2026)", "kobo"),
      r("boox-go-6", "Boox Go 6 (2024)", "kobo"),
      r("boox-go-7", "Boox Go 7 (2025)", "kobo-libra"),
      r("boox-go-color-7-gen2", "Boox Go Color 7 Gen II (2025)", "kobo-color"),
      r("boox-go-color-7", "Boox Go Color 7 (2024)", "kobo-color"),
      r("boox-leaf-2", "Boox Leaf 2 (2022)", "kobo-libra"),
      r("boox-leaf", "Boox Leaf (2021)", "kobo-libra"),
      r("boox-max-lumi-2", "Boox Max Lumi 2 (2021)", "eink1650"),
      r("boox-max-lumi", "Boox Max Lumi (2020)", "eink1650"),
      r("boox-note-air-6c", "Boox Note Air 6 C (2026)", "colour1860"),
      r("boox-note-air-5c", "Boox Note Air 5 C (2025)", "colour1860"),
      r("boox-note-air-4c", "Boox Note Air 4 C (2024)", "colour1860"),
      r("boox-note-air-3c", "Boox Note Air 3 C (2023)", "colour1860"),
      r("boox-note-air-3", "Boox Note Air 3 (2023)", "kobo-elipsa"),
      r("boox-note-air-2-plus", "Boox Note Air 2 Plus (2022)", "kobo-elipsa"),
      r("boox-note-max", "Boox Note Max (2025)", "eink2400"),
      r("boox-nova-air-c", "Boox Nova Air C (2022)", "colour1404"),
      r("boox-nova-air-2", "Boox Nova Air 2 (2022)", "kobo-elipsa"),
      r("boox-page", "Boox Page (2023)", "kobo-libra"),
      r("boox-palma-3", "Boox Palma 3 (2026)", "eink824"),
      r("boox-palma-2-pro", "Boox Palma 2 Pro (2025)", "colour824"),
      r("boox-palma-2", "Boox Palma 2 (2024)", "eink824"),
      r("boox-palma", "Boox Palma (2023)", "eink824"),
      r("boox-picco", "Boox Picco (2026)", "eink480"),
      r("boox-poke-5", "Boox Poke 5 (2023)", "kobo"),
      r("boox-tab-mini-c", "Boox Tab Mini C (2023)", "colour1404"),
      r("boox-tab-ultra-c-pro", "Boox Tab Ultra C Pro (2023)", "colour1860"),
      r("boox-tab-ultra-c", "Boox Tab Ultra C (2023)", "colour1860"),
      r("boox-tab-ultra", "Boox Tab Ultra (2022)", "kobo-elipsa"),
      r("boox-tab-x-c", "Boox Tab X C (2025)", "colour2400"),
      r("boox-tab-x", "Boox Tab X (2023)", "eink1650"),
    ],
  ],
  [
    "comicDevices_kindle",
    ["azw3", "pdf"],
    [
      r("kindle-2026", "Kindle 2026", "kindle"),
      r("kindle-2024", "Kindle 2024", "kindle"),
      r("kindle-2022", "Kindle 2022", "kindle"),
      r("kindle-10", "Kindle 10 (2019)", "kindle-basic"),
      r("kindle-8", "Kindle 8 (2016)", "kindle-basic"),
      r("kindle-7", "Kindle 7 (2014)", "kindle-basic"),
      r("kindle-5", "Kindle 5 (2012)", "kindle-basic"),
      r("kindle-4", "Kindle 4 (2011)", "kindle-basic"),
      r("kindle-colorsoft-2026", "Kindle Colorsoft 2026", "kindle-color"),
      r("kindle-colorsoft", "Kindle Colorsoft (2024)", "kindle-color"),
      r("kindle-dx", "Kindle DX (2009)", "kindle-dx"),
      r("kindle-oasis-3", "Kindle Oasis 3 (2019)", "kindle-oasis"),
      r("kindle-oasis-2", "Kindle Oasis 2 (2017)", "kindle-oasis"),
      r("kindle-oasis-1", "Kindle Oasis 1 (2016)", "kindle"),
      r("kindle-pw-2026", "Kindle Paperwhite 2026", "kindle-pw2024"),
      r("kindle-pw-2024", "Kindle Paperwhite 2024", "kindle-pw2024"),
      r("kindle-pw-2021", "Kindle Paperwhite 2021", "kindle-pw"),
      r("kindle-pw-4", "Kindle Paperwhite 4 (2018)", "kindle"),
      r("kindle-pw-3", "Kindle Paperwhite 3 (2015)", "kindle"),
      r("kindle-pw-2", "Kindle Paperwhite 2 (2013)", "kindle-pw-old"),
      r("kindle-pw-1", "Kindle Paperwhite 1 (2012)", "kindle-pw-old"),
      r("kindle-scribe-colorsoft", "Kindle Scribe Colorsoft (2025)", "kindle-scribe-color"),
      r("kindle-scribe-2025", "Kindle Scribe 2025", "kindle-scribe2025"),
      r("kindle-scribe-2024", "Kindle Scribe 2024", "kindle-scribe"),
      r("kindle-scribe-2022", "Kindle Scribe 2022", "kindle-scribe"),
      r("kindle-touch", "Kindle Touch (2011)", "kindle-basic"),
      r("kindle-voyage", "Kindle Voyage (2014)", "kindle"),
    ],
  ],
  [
    "comicDevices_kobo",
    ["kepub", "epub", "cbz", "pdf"],
    [
      r("kobo-aura-one", "Kobo Aura One (2016)", "kobo-elipsa"),
      r("kobo-aura-h2o", "Kobo Aura H2O (2014, 2017)", "kobo-h2o"),
      r("kobo-aura", "Kobo Aura (2013)", "kobo-nia"),
      r("kobo-clara-colour", "Kobo Clara Colour (2024)", "kobo-clara-color"),
      r("kobo-clara-bw", "Kobo Clara BW (2024)", "kobo"),
      r("kobo-clara-2e", "Kobo Clara 2E (2022)", "kobo"),
      r("kobo-clara-hd", "Kobo Clara HD (2018)", "kobo"),
      r("kobo-elipsa-2e", "Kobo Elipsa 2E (2023)", "kobo-elipsa"),
      r("kobo-elipsa", "Kobo Elipsa (2021)", "kobo-elipsa"),
      r("kobo-forma", "Kobo Forma (2018)", "kobo-sage"),
      r("kobo-glo-hd", "Kobo Glo HD (2015)", "kobo"),
      r("kobo-libra-colour", "Kobo Libra Colour (2024)", "kobo-color"),
      r("kobo-libra-2", "Kobo Libra 2 (2021)", "kobo-libra"),
      r("kobo-libra-h2o", "Kobo Libra H2O (2019)", "kobo-h2o"),
      r("kobo-nia", "Kobo Nia (2020)", "kobo-nia"),
      r("kobo-sage", "Kobo Sage (2021)", "kobo-sage"),
    ],
  ],
  [
    "comicDevices_barnes",
    ["epub", "pdf"],
    [
      r("nook-gl4-plus", "Nook GlowLight 4 Plus (2023)", "kobo-elipsa"),
      r("nook-gl4e", "Nook GlowLight 4e (2022)", "kobo-nia"),
      r("nook-gl4", "Nook GlowLight 4 (2021)", "kobo"),
      r("nook-gl-plus-78", "Nook GlowLight Plus 7.8 (2019)", "kobo-elipsa"),
      r("nook-gl3", "Nook GlowLight 3 (2017)", "kobo"),
    ],
  ],
  [
    "comicDevices_pocketbook",
    ["epub", "cbz", "pdf", "azw3", "fb2"],
    [
      r("pb-basic-lux-4", "PocketBook Basic Lux 4 (2023)", "kobo-nia"),
      r("pb-color", "PocketBook Color (2020)", "kobo-clara-color"),
      r("pb-color-note", "PocketBook Color Note (2024)", "colour1404"),
      r("pb-era-color", "PocketBook Era Color (2024)", "kobo-color"),
      r("pb-era-lite", "PocketBook Era Lite (2026)", "kobo-libra"),
      r("pb-era", "PocketBook Era (2022)", "kobo-libra"),
      r("pb-inkpad-eo-2", "PocketBook InkPad Eo 2 (2026)", "colour1860"),
      r("pb-inkpad-eo", "PocketBook InkPad Eo (2024)", "colour1860"),
      r("pb-inkpad-one", "PocketBook InkPad One (2026)", "kobo-elipsa"),
      r("pb-inkpad-x-pro", "PocketBook InkPad X Pro (2023)", "kobo-elipsa"),
      r("pb-inkpad-x", "PocketBook InkPad X (2019)", "kobo-elipsa"),
      r("pb-inkpad-color-3", "PocketBook InkPad Color 3 (2023)", "colour1404"),
      r("pb-inkpad-color-2", "PocketBook InkPad Color 2 (2023)", "colour1404"),
      r("pb-inkpad-color", "PocketBook InkPad Color (2021)", "colour1404"),
      r("pb-inkpad-4", "PocketBook InkPad 4 (2023)", "kobo-elipsa"),
      r("pb-inkpad-3-pro", "PocketBook InkPad 3 Pro (2019)", "kobo-elipsa"),
      r("pb-inkpad-3", "PocketBook InkPad 3 (2018)", "kobo-elipsa"),
      r("pb-touch-hd-3", "PocketBook Touch HD 3 (2019)", "kobo"),
      r("pb-touch-lux-5", "PocketBook Touch Lux 5 (2020)", "kobo-nia"),
      r("pb-verse-pro-color", "PocketBook Verse Pro Color (2024)", "kobo-clara-color"),
      r("pb-verse-pro", "PocketBook Verse Pro (2023)", "kobo"),
      r("pb-verse-lite", "PocketBook Verse Lite (2025)", "kobo-nia"),
      r("pb-verse", "PocketBook Verse (2023)", "kobo-nia"),
      r("pb-verta-color", "PocketBook Verta Color (2026)", "kobo-clara-color"),
      r("pb-verta", "PocketBook Verta (2026)", "kobo"),
    ],
  ],
  [
    "comicDevices_remarkable",
    ["pdf", "epub"],
    [
      r("remarkable-pure", "reMarkable Paper Pure (2026)", "remarkable"),
      r("remarkable-move", "reMarkable Paper Pro Move (2025)", "remarkable-move"),
      r("remarkable-pro", "reMarkable Paper Pro (2024)", "remarkable-pro"),
      r("remarkable-2", "reMarkable 2 (2020)", "remarkable"),
      r("remarkable-1", "reMarkable 1 (2017)", "remarkable"),
    ],
  ],
  [
    "comicDevices_supernote",
    ["epub", "cbz", "pdf", "fb2"],
    [
      r("supernote-a5x", "Supernote A5 X (2020)", "kobo-elipsa"),
      r("supernote-a6x", "Supernote A6 X (2021)", "kobo-elipsa"),
      r("supernote-manta", "Supernote Manta (2024)", "eink1920"),
      r("supernote-nomad", "Supernote Nomad (2023)", "kobo-elipsa"),
    ],
  ],
  [
    "comicDevices_tolino",
    // The 2024 ones (made with Kobo) open CBZ; the older ones do not.
    ["epub", "cbz", "pdf"],
    [
      r("tolino-epos-3", "tolino epos 3 (2022)", "kobo-sage", ["epub", "pdf"]),
      r("tolino-epos-2", "tolino epos 2 (2019)", "kobo-sage", ["epub", "pdf"]),
      r("tolino-shine-color", "tolino shine color (2024)", "kobo-clara-color"),
      r("tolino-shine-5", "tolino shine 5 (2024)", "kobo"),
      r("tolino-shine-4", "tolino shine 4 (2022)", "kobo", ["epub", "pdf"]),
      r("tolino-vision-color", "tolino vision color (2024)", "kobo-color"),
      r("tolino-vision-6", "tolino vision 6 (2022)", "kobo-libra", ["epub", "pdf"]),
      r("tolino-vision-5", "tolino vision 5 (2019)", "kobo-libra", ["epub", "pdf"]),
    ],
  ],
  [
    "comicDevices_xteink",
    ["epub"],
    [
      r("xteink-s4", "Xteink S4 (2026, Android)", "eink480", APPS),
      r("xteink-x4-pro", "Xteink X4 Pro (2026)", "eink480"),
      r("xteink-x4", "Xteink X4 (2025)", "eink480"),
      r("xteink-x3", "Xteink X3 (2026)", "eink528"),
    ],
  ],
  [
    "comicDevices_tablets",
    APPS,
    [
      r("ipad", "iPad, iPad Air, iPad Pro 11″", "ipad"),
      r("ipad-13", "iPad Air, iPad Pro 13″", "ipad13"),
      r("tablet", "@comicModel_tablet", "tablet"),
      r("phone", "@comicModel_phone", "phone"),
    ],
  ],
];

/** The others, labelled by i18n key `comicDevice_<device>`: they take any
 *  format. */
export const OTHER_DEVICES: ComicDevice[] = ["custom", "original"];

/** The models most people have, shown first. */
const POPULAR_IDS = [
  "kindle-pw-2024", "kindle-2024", "kindle-colorsoft", "kindle-scribe-2025",
  "kobo-clara-bw", "kobo-clara-colour", "kobo-libra-colour",
  "pb-verse-pro", "pb-era", "boox-go-7", "boox-palma-2", "remarkable-2", "remarkable-pro",
  "ipad", "tablet", "phone",
];

const ALL = READERS.flatMap(([, brand, list]) => list.map((m) => ({ ...m, formats: m.formats ?? brand })));

export const POPULAR: Reader[] = POPULAR_IDS.map((id) => ALL.find((m) => m.id === id)!).filter(Boolean);

/** The option chosen in a rule: its model, else (rules made before
 *  models) the first model of its device, else the device itself. */
export function readerOf(device: ComicDevice, model: string | undefined): string {
  if (model && ALL.some((m) => m.id === model && m.device === device)) return model;
  return ALL.find((m) => m.device === device)?.id ?? device;
}

/** What an option stands for: the device, and the model when it is one. */
export function chooseReader(value: string): { device: ComicDevice; model?: string } {
  const m = ALL.find((x) => x.id === value);
  return m ? { device: m.device, model: m.id } : { device: value as ComicDevice };
}

/** A model by its id. */
export const modelOf = (id: string | undefined) => (id ? ALL.find((m) => m.id === id) : undefined);

/** Whether a reader opens a format as it comes (the others, any). */
export function opens(device: ComicDevice, model: string | undefined, format: Format): boolean {
  const m = modelOf(readerOf(device, model));
  return !m || m.formats.includes(format);
}

/** The formats a reader opens as it comes, best first (none listed: any). */
export const formatsOf = (device: ComicDevice, model: string | undefined): Format[] | null => modelOf(readerOf(device, model))?.formats ?? null;

/** A screen's size in mm (for pages printed to fit it). */
export function screenMm(device: ComicDevice): [number, number] | null {
  const sc = SCREENS[device];
  if (!sc) return null;
  const mm = (px: number) => Math.round((px / sc.ppi) * 254) / 10;
  return [mm(sc.size[0]), mm(sc.size[1])];
}
