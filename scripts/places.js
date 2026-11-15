// Makes crates/renome-core/data/places.tsv.gz, the places the photo rule
// names a photo's GPS position by: GeoNames' cities500 (places of 500
// people or more, CC BY 4.0, https://www.geonames.org) and its first-level
// divisions and how many people live in them, with the countries' names in Portuguese, English and Spanish
// (CLDR, through Node's Intl). Run by hand when the data should be renewed:
//   node scripts/places.js
// It needs `curl` and `unzip`.
import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { gzipSync } from "node:zlib";

const BASE = "https://download.geonames.org/export/dump/";
const dir = mkdtempSync(join(tmpdir(), "places-"));
try {
  for (const f of ["cities500.zip", "admin1CodesASCII.txt"]) {
    execFileSync("curl", ["-sSfL", "-o", join(dir, f), BASE + f]);
  }
  const cities = execFileSync("unzip", ["-p", join(dir, "cities500.zip"), "cities500.txt"], { maxBuffer: 1 << 30 }).toString("utf8");
  const admin1 = readFileSync(join(dir, "admin1CodesASCII.txt"), "utf8");

  // A part of a city (PPLX, a district of São Paulo) takes the name of the
  // most populous town of its municipality (country, state and the three
  // levels below); without those levels, or without such a town, it is
  // left out. So does a town named after that one and more ("Paris 16
  // Passy"). Places that no longer are
  // (historical, abandoned, destroyed) are left out too.
  const GONE = new Set(["PPLH", "PPLQ", "PPLW", "PPLCH"]);
  // Countries whose second level is the municipality (Brazil's município,
  // Portugal's concelho, a Japanese city or Tokyo's ward): every place in
  // one is named after its main town.
  const WHOLE = new Set(["BR", "PT", "JP"]);
  const all = [];
  for (const line of cities.split("\n")) {
    const c = line.split("\t");
    if (c.length < 15) continue;
    const [name, lat, lon, code, cc, adm] = [c[1], Number(c[4]), Number(c[5]), c[7], c[8], c[10]];
    if (!name || !cc || Number.isNaN(lat) || Number.isNaN(lon) || GONE.has(code)) continue;
    all.push({ name, lat: Math.round(lat * 1e4), lon: Math.round(lon * 1e4), cc, adm, part: code === "PPLX", town: c.slice(8, WHOLE.has(cc) ? 12 : 14).filter((_, i) => i !== 1).join("."), level: !!c[11], people: Number(c[14]) || 0 });
  }
  // A few towns lack their municipality (São Paulo itself, in GeoNames):
  // they take the one of the nearest place of their state that has it.
  for (const r of all) {
    if (r.level || !WHOLE.has(r.cc)) continue;
    let best = null, bestD = Infinity;
    for (const o of all) {
      if (!o.level || o.cc !== r.cc || o.adm !== r.adm) continue;
      const d = (o.lat - r.lat) ** 2 + (o.lon - r.lon) ** 2;
      if (d < bestD) { best = o; bestD = d; }
    }
    if (best) { r.town = best.town; r.level = true; }
  }
  const head = new Map();
  for (const r of all) {
    if ((r.part && !WHOLE.has(r.cc)) || !r.level) continue;
    const h = head.get(r.town);
    if (!h || r.people > h.people) head.set(r.town, r);
  }
  const rows = [];
  const countries = new Set();
  for (const r of all) {
    const h = r.level && head.get(r.town);
    if (r.part && h !== r) {
      if (!h) continue;
      r.name = h.name;
      r.people = h.people;
    } else if (h && h !== r && WHOLE.has(r.cc)) {
      // Where that level is the municipality, its districts are the city.
      r.name = h.name;
      r.people = h.people;
    } else if (h && h !== r && new RegExp(`^${h.name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}[\\s\\d-]`).test(r.name)) {
      // "Paris 16 Passy": a part named after its city.
      r.name = h.name;
      r.people = h.people;
    }
    countries.add(r.cc);
    rows.push(r);
  }
  rows.sort((a, b) => a.cc.localeCompare(b.cc) || a.adm.localeCompare(b.adm) || a.lat - b.lat || a.lon - b.lon);

  const out = [];
  const names = Object.fromEntries(["pt-BR", "en", "es"].map((l) => [l, new Intl.DisplayNames([l], { type: "region", fallback: "code" })]));
  for (const cc of [...countries].sort()) {
    let of = (l) => { try { return names[l].of(cc); } catch { return cc; } };
    out.push(["C", cc, of("pt-BR"), of("en"), of("es")].join("\t"));
  }
  for (const line of admin1.split("\n")) {
    const c = line.split("\t");
    if (c.length >= 2 && countries.has(c[0].split(".")[0])) out.push(["A", c[0], c[1]].join("\t"));
  }
  // People, to two digits (12 000 000, 4 300): enough to weigh by.
  const rough = (n) => {
    if (n < 100) return n;
    const p = 10 ** (Math.floor(Math.log10(n)) - 1);
    return Math.round(n / p) * p;
  };
  // Each country's and division's places after its "G" line, the
  // positions (degrees × 10 000) as differences from the line before.
  let group = "", lat = 0, lon = 0;
  for (const r of rows) {
    const g = `${r.cc}\t${r.adm}`;
    if (g !== group) {
      out.push(`G\t${g}`);
      group = g; lat = 0; lon = 0;
    }
    out.push(`${r.lat - lat}\t${r.lon - lon}\t${r.name}\t${rough(r.people)}`);
    lat = r.lat; lon = r.lon;
  }
  const gz = gzipSync(Buffer.from(out.join("\n") + "\n", "utf8"), { level: 9 });
  writeFileSync(new URL("../crates/renome-core/data/places.tsv.gz", import.meta.url), gz);
  console.log(`${rows.length} places, ${countries.size} countries, ${gz.length} bytes`);
} finally {
  rmSync(dir, { recursive: true, force: true });
}
