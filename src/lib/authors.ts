// One spelling per author across the books of a list, so one author's books
// get one name and one folder.

const fold = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

/** First name's initial and surname: `C. S. Moraes`, `C.S. Moraes` and
 *  `C. S. MORAES` are one author, and so are `Rita Prado` and
 *  `Rita J. Prado`, or `Luís de Quevedo` and `Luís Vaz de Quevedo`. */
export function authorKey(name: string): string {
  let words = fold(name).split(/[^\p{L}\p{N}]+/u).filter(Boolean);
  // `Lindqvist, Otto`
  const comma = name.indexOf(",");
  if (comma > 0) {
    const [last, first] = [name.slice(0, comma), name.slice(comma + 1)].map((p) => fold(p).split(/[^\p{L}\p{N}]+/u).filter(Boolean));
    words = [...first, ...last];
  }
  if (!words.length) return "";
  return `${words[0][0]} ${words[words.length - 1]}`;
}

const shouting = (s: string) => /\p{L}/u.test(s) && s === s.toUpperCase();

/** The spelling each author gets: the most common one in the list, not in
 *  capitals when there is another; accents always composed (`á` as one
 *  character), as two forms of one name look alike but are not. */
export function authorSpellings(names: string[]): Map<string, string> {
  const seen = new Map<string, Map<string, number>>();
  for (const raw of names) {
    const name = raw.normalize("NFC");
    const key = authorKey(name);
    if (!key) continue;
    const counts = seen.get(key) ?? new Map<string, number>();
    counts.set(name, (counts.get(name) ?? 0) + 1);
    seen.set(key, counts);
  }
  const out = new Map<string, string>();
  for (const counts of seen.values()) {
    const best = [...counts].sort((a, b) => Number(shouting(a[0])) - Number(shouting(b[0])) || b[1] - a[1])[0][0];
    for (const name of counts.keys()) out.set(name, best);
  }
  return out;
}
