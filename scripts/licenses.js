// @ts-nocheck: build script run by Node, outside the app (no Node types here).
// Vite plugin: writes `licenses.json` with the licenses of the open source
// components built into the app, for the "Licenses" tab: the Rust crates
// linked into the program (from `cargo metadata`) and the npm packages
// that end up in the interface bundle.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const LICENSE_FILE = /^(licen[cs]e|copying|notice)/i;
/** Our own packages: their license is shown on its own. */
const OWN = new Set(["renome", "renome-core"]);

/** License and notice files in a package folder, sorted. */
function licenseFiles(dir) {
  let names = [];
  try {
    names = fs.readdirSync(dir).filter((n) => LICENSE_FILE.test(n)).sort();
  } catch {
    return [];
  }
  return names
    // `LICENSE.spdx` is a description of the package, not a license text.
    .filter((n) => !/\.spdx$/i.test(n))
    .map((n) => path.join(dir, n))
    .filter((p) => fs.statSync(p).isFile())
    .map((p) => ({ name: path.basename(p), text: fs.readFileSync(p, "utf8").replace(/\r\n/g, "\n").trim() }));
}

const isMitText = (f) => /mit/i.test(f.name) || f.text.includes("Permission is hereby granted");

/** For "MIT OR Apache-2.0" and the like, MIT is chosen: only its text (and
 *  any NOTICE) is needed. */
function pickTexts(expression, files) {
  const offersMit = /\bMIT\b/.test(expression) && !/\bAND\b/.test(expression);
  if (offersMit) {
    const mit = files.filter((f) => isMitText(f) || /^notice/i.test(f.name));
    if (mit.some(isMitText)) return mit;
  }
  return files;
}

/** The first license named in an expression ("MIT OR Apache-2.0" → "MIT"). */
const firstLicense = (expression) => (expression.match(/[A-Za-z0-9.+-]+/) ?? [""])[0];

function rustPackages(root, warn) {
  const triple = process.env.TAURI_ENV_TARGET_TRIPLE;
  let target = triple && !triple.includes("universal") ? triple : "";
  try {
    if (!target) target = /host: (\S+)/.exec(execFileSync("rustc", ["-vV"], { encoding: "utf8" }))?.[1] ?? "";
    const meta = JSON.parse(
      execFileSync(
        "cargo",
        ["metadata", "--format-version", "1", "--manifest-path", path.join(root, "src-tauri/Cargo.toml"),
          ...(target ? ["--filter-platform", target] : [])],
        { encoding: "utf8", maxBuffer: 256 * 1024 * 1024 },
      ),
    );
    const byId = new Map(meta.packages.map((p) => [p.id, p]));
    const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
    const app = meta.packages.find((p) => p.name === "renome");
    // Normal dependencies only: build and dev tools are not in the program.
    const seen = new Set();
    const stack = [app.id];
    while (stack.length) {
      const id = stack.pop();
      if (seen.has(id)) continue;
      seen.add(id);
      for (const d of nodes.get(id)?.deps ?? []) {
        if (d.dep_kinds.some((k) => k.kind === null)) stack.push(d.pkg);
      }
    }
    return [...seen]
      .map((id) => byId.get(id))
      .filter((p) => !OWN.has(p.name))
      .map((p) => ({
        name: p.name,
        version: p.version,
        license: p.license ?? (p.license_file ? "see license file" : ""),
        url: p.repository ?? p.homepage ?? `https://crates.io/crates/${p.name}`,
        authors: (p.authors ?? []).map((a) => a.replace(/\s*<[^>]*>/, "")),
        kind: "rust",
        files: licenseFiles(path.dirname(p.manifest_path)),
      }));
  } catch (e) {
    warn(`Rust licenses not collected: ${e.message}`);
    return [];
  }
}

function repoUrl(repo) {
  const url = typeof repo === "string" ? repo : repo?.url;
  return url?.replace(/^git\+/, "").replace(/\.git$/, "") ?? "";
}

/** npm packages whose code is in the bundle. */
function npmPackages(moduleIds) {
  const dirs = new Set();
  for (const id of moduleIds) {
    const clean = id.replace(/^\0/, "").split("?")[0].replace(/\\/g, "/");
    const i = clean.lastIndexOf("/node_modules/");
    if (i < 0) continue;
    const rest = clean.slice(i + "/node_modules/".length).split("/");
    const name = rest[0].startsWith("@") ? `${rest[0]}/${rest[1]}` : rest[0];
    dirs.add(`${clean.slice(0, i)}/node_modules/${name}`);
  }
  return [...dirs].flatMap((dir) => {
    try {
      const pkg = JSON.parse(fs.readFileSync(path.join(dir, "package.json"), "utf8"));
      return [{
        name: pkg.name,
        version: pkg.version,
        license: typeof pkg.license === "string" ? pkg.license : pkg.license?.type ?? "",
        url: repoUrl(pkg.repository) || pkg.homepage || `https://www.npmjs.com/package/${pkg.name}`,
        kind: "npm",
        files: licenseFiles(dir),
      }];
    } catch {
      return [];
    }
  });
}

/** The MIT license with a package's own copyright line, for packages
 *  published without their license file. */
function mitText(p) {
  const who = p.authors?.length ? p.authors.join(", ") : `the ${p.name} authors`;
  const where = p.url && !p.url.includes("crates.io") ? ` (${p.url})` : "";
  return `MIT License

Copyright (c) ${who}${where}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.`;
}

/** Builds the list; identical texts are stored once and referred to by index. */
function collect(root, moduleIds, warn) {
  const all = [...rustPackages(root, warn), ...npmPackages(moduleIds)];
  // A package without license files takes those of another package from
  // the same repository under the same license (the npm side of a Rust
  // plugin): same authors.
  const repo = (u) => (u ?? "").toLowerCase().replace(/^https?:\/\/(www\.)?/, "").replace(/\/(tree|blob)\/.*$/, "").replace(/\/$/, "");
  const byRepo = new Map();
  // "MIT OR Apache-2.0" and "Apache-2.0 OR MIT" are one license.
  const terms = (l) => [...new Set(l.match(/[A-Za-z0-9.+-]+/g) ?? [])].sort().join(" ");
  const key = (p) => `${repo(p.url)} ${terms(p.license)}`;
  for (const p of all) if (p.files.length && repo(p.url) && !byRepo.has(key(p))) byRepo.set(key(p), p.files);
  const generic = [];
  for (const p of all) {
    if (p.files.length) continue;
    const same = byRepo.get(key(p));
    if (same) p.files = same;
    // MIT names its authors: written with this package's own.
    else if (/\bMIT\b/.test(p.license) && !/\bAND\b/.test(p.license)) p.files = [{ name: "LICENSE-MIT", text: mitText(p) }];
    else generic.push(p.name);
  }
  // Licenses without a copyright line of their own (MPL, Apache) are the
  // same text for everyone.
  if (generic.length) warn(`standard license texts used for: ${generic.join(", ")}`);
  // Standard texts for the few packages without license files, taken from
  // packages that have them.
  const standard = new Map();
  for (const p of all) {
    const only = firstLicense(p.license);
    const files = pickTexts(p.license, p.files);
    if (files.length === 1 && only && !standard.has(only) && p.license.trim() === only) standard.set(only, files);
  }
  const texts = [];
  const index = new Map();
  const ref = (text) => {
    if (!index.has(text)) {
      index.set(text, texts.length);
      texts.push(text);
    }
    return index.get(text);
  };
  const packages = all
    .map((p) => {
      let files = pickTexts(p.license, p.files);
      if (!files.length) files = standard.get(firstLicense(p.license)) ?? [];
      return { name: p.name, version: p.version, license: p.license, url: p.url, kind: p.kind, texts: files.map((f) => ref(f.text)) };
    })
    .sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));
  return { packages, texts };
}

export function licenses() {
  const root = process.cwd();
  return {
    name: "renome-licenses",
    apply: "build",
    generateBundle(_options, bundle) {
      // SvelteKit also builds for the server; only the app's bundle counts.
      if (this.environment && this.environment.name !== "client") return;
      const ids = Object.values(bundle).flatMap((c) =>
        c.type === "chunk" ? (c.moduleIds ?? Object.keys(c.modules ?? {})) : [],
      );
      const data = collect(root, ids, (m) => this.warn(m));
      this.emitFile({ type: "asset", fileName: "licenses.json", source: JSON.stringify(data) });
    },
  };
}
