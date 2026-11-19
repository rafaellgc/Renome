// "{n} arquivo(s)" made "1 arquivo" or "3 arquivos" (and "fica(m)" "fica"
// or "ficam"; "[tem|têm]" "tem" or "têm", for words whose plural is not
// an ending): each word with its plural ending in brackets follows the
// number before it (the first one in the text when none comes before).
// Without numbers, left as is. The command line does the same in Rust
// (renome-cli `text::plurals`): both run the cases of plurals.json.

import table from "./plurals.json" with { type: "json" };

/** Words whose plural is not the singular plus the letters in brackets. */
const IRREGULAR: Record<string, string> = table.irregular;

export function plurals(text: string, vars: Record<string, string | number>): string {
  const count = (name: string) => {
    const v = vars[name];
    if (typeof v === "number") return v;
    return typeof v === "string" && /^\d+$/.test(v) ? Number(v) : null;
  };
  const numbers = [...text.matchAll(/\{(\w+)\}/g)].map((m) => ({ at: m.index ?? 0, n: count(m[1]) })).filter((x) => x.n !== null);
  if (!numbers.length) return text;
  return text.replace(
    /\[([^[\]|]*)\|([^[\]|]*)\]|([\p{L}]*)\((s|es|ns|ões|eis|ies|m|n)\)/gu,
    (whole, one: string | undefined, many: string | undefined, word: string | undefined, _end, at: number) => {
      const before = numbers.filter((x) => x.at < at).pop() ?? numbers[0];
      if (one !== undefined) return before.n === 1 ? one : (many ?? "");
      if (before.n === 1) return word ?? "";
      return IRREGULAR[whole.toLowerCase()] ?? (word ?? "") + whole.slice((word ?? "").length + 1, -1);
    },
  );
}
