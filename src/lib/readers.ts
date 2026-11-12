// The readers comics can be made for, one entry per model, each pointing
// at the screen it shares with others (`ComicDevice`, what the converter
// knows). Grouped by brand in alphabetical order; within a brand, by line
// in alphabetical order and, in each line, newest first.

import type { ComicDevice } from "./types";

export type Reader = {
  /** Kept in the rule as `model` (to show which model was chosen). */
  id: string;
  /** The model's name (not translated). */
  name: string;
  device: ComicDevice;
};

/** Screen sizes, as the converter fits pages into them. */
export const SIZES: Partial<Record<ComicDevice, [number, number]>> = {
  kindle: [1072, 1448],
  "kindle-pw": [1236, 1648],
  "kindle-pw2024": [1272, 1696],
  "kindle-oasis": [1264, 1680],
  "kindle-pw-old": [758, 1024],
  "kindle-basic": [600, 800],
  "kindle-dx": [824, 1000],
  "kindle-scribe": [1860, 2480],
  "kindle-scribe2025": [1986, 2648],
  "kindle-color": [1272, 1696],
  "kindle-scribe-color": [1986, 2648],
  kobo: [1072, 1448],
  "kobo-clara-color": [1072, 1448],
  "kobo-libra": [1264, 1680],
  "kobo-color": [1264, 1680],
  "kobo-h2o": [1080, 1430],
  "kobo-sage": [1440, 1920],
  "kobo-elipsa": [1404, 1872],
  "kobo-nia": [758, 1024],
  remarkable: [1404, 1872],
  "remarkable-pro": [1620, 2160],
  "remarkable-move": [954, 1696],
  tablet: [1600, 2560],
};

const r = (id: string, name: string, device: ComicDevice): Reader => ({ id, name, device });

/** Brand (an i18n key) and its models. "Others" last. */
export const READERS: [string, Reader[]][] = [
  [
    "comicDevices_kindle",
    [
      r("kindle-2024", "Kindle 2024", "kindle"),
      r("kindle-2022", "Kindle 2022", "kindle"),
      r("kindle-10", "Kindle 10 (2019)", "kindle-basic"),
      r("kindle-8", "Kindle 8 (2016)", "kindle-basic"),
      r("kindle-7", "Kindle 7 (2014)", "kindle-basic"),
      r("kindle-5", "Kindle 5 (2012)", "kindle-basic"),
      r("kindle-4", "Kindle 4 (2011)", "kindle-basic"),
      r("kindle-colorsoft", "Kindle Colorsoft (2024)", "kindle-color"),
      r("kindle-dx", "Kindle DX (2009)", "kindle-dx"),
      r("kindle-oasis-3", "Kindle Oasis 3 (2019)", "kindle-oasis"),
      r("kindle-oasis-2", "Kindle Oasis 2 (2017)", "kindle-oasis"),
      r("kindle-oasis-1", "Kindle Oasis 1 (2016)", "kindle"),
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
    "comicDevices_remarkable",
    [
      r("remarkable-move", "reMarkable Paper Pro Move (2025)", "remarkable-move"),
      r("remarkable-pro", "reMarkable Paper Pro (2024)", "remarkable-pro"),
      r("remarkable-2", "reMarkable 2 (2020)", "remarkable"),
      r("remarkable-1", "reMarkable 1 (2017)", "remarkable"),
    ],
  ],
];

/** The others, labelled by i18n key `comicDevice_<device>`. */
export const OTHER_DEVICES: ComicDevice[] = ["tablet", "custom", "original"];

const ALL = READERS.flatMap(([, list]) => list);

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
