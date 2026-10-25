// Diff between the current and the new name, to highlight what a rename
// changes: whole words are compared, so a part that only moved
// (`Title - Author` → `Author - Title`) is shown as moved, not as letters
// taken here and there.

export interface Seg {
  text: string;
  changed: boolean;
  /** Changed only in place: the same words are on the other side. */
  moved?: boolean;
}

type State = "same" | "changed" | "moved";
interface Piece {
  text: string;
  state: State;
  /** A word (not a space or a sign). */
  word: boolean;
}

const graphemes = new Intl.Segmenter(undefined, { granularity: "grapheme" });
/** Whole letters, accents included: `á` written as `a` plus an accent is
 *  one letter, not an `a` kept and an accent added. */
const letters = (s: string) => [...graphemes.segment(s.normalize("NFC"))].map((g) => g.segment);
const isWord = (c: string) => /[\p{L}\p{N}]/u.test(c);
const fold = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

/** Words (letters and digits in a row) and each other character alone. */
function tokens(s: string): string[] {
  const out: string[] = [];
  for (const c of letters(s)) {
    const last = out.length - 1;
    if (isWord(c) && last >= 0 && isWord(out[last])) out[last] += c;
    else out.push(c);
  }
  return out;
}

/** Long words count more than spaces and signs: what is kept is the
 *  most text, not the most pieces. */
const weight = (t: string) => (isWord(t) ? 2 * t.length : 1);

/** Longest common subsequence: which items of each side are kept. The
 *  common start and end are kept first (the extension stays the
 *  extension, not a dot found earlier). */
function lcs(x: string[], y: string[]): [boolean[], boolean[]] {
  const kx = new Array<boolean>(x.length).fill(false);
  const ky = new Array<boolean>(y.length).fill(false);
  let pre = 0;
  while (pre < x.length && pre < y.length && x[pre] === y[pre]) kx[pre] = ky[pre++] = true;
  let suf = 0;
  while (suf < x.length - pre && suf < y.length - pre && x[x.length - 1 - suf] === y[y.length - 1 - suf]) {
    kx[x.length - 1 - suf] = ky[y.length - 1 - suf] = true;
    suf++;
  }
  const n = x.length - pre - suf;
  const m = y.length - pre - suf;
  const w = m + 1;
  const dp = new Uint32Array((n + 1) * w);
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      dp[i * w + j] =
        x[pre + i] === y[pre + j]
          ? dp[(i + 1) * w + j + 1] + weight(x[pre + i])
          : Math.max(dp[(i + 1) * w + j], dp[i * w + j + 1]);
    }
  }
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (x[pre + i] === y[pre + j] && dp[i * w + j] === dp[(i + 1) * w + j + 1] + weight(x[pre + i])) {
      kx[pre + i++] = ky[pre + j++] = true;
    }
    else if (dp[(i + 1) * w + j] >= dp[i * w + j + 1]) i++;
    else j++;
  }
  return [kx, ky];
}

/** A word that changed only a little (`Ceu` → `Céu`, `Mario` → `Mário`):
 *  its letters, the changed ones marked. */
function refine(a: string, b: string): [Piece[], Piece[]] | null {
  if (fold(a) !== fold(b)) return null;
  const x = letters(a);
  const y = letters(b);
  const [kx, ky] = lcs(x, y);
  const pieces = (l: string[], k: boolean[]) => l.map((t, i): Piece => ({ text: t, state: k[i] ? "same" : "changed", word: false }));
  return [pieces(x, kx), pieces(y, ky)];
}

/** Changed words found changed on the other side too moved; so do the
 *  spaces and signs between two such words (`C. S.`). */
function markMoved(a: Piece[], b: Piece[]) {
  const count = (ps: Piece[]) => {
    const c = new Map<string, number>();
    for (const p of ps) if (p.word && p.state === "changed") c.set(fold(p.text), (c.get(fold(p.text)) ?? 0) + 1);
    return c;
  };
  const inA = count(a);
  const inB = count(b);
  for (const [ps, there] of [[a, inB], [b, inA]] as const) {
    const left = new Map(there);
    for (const p of ps) {
      const k = fold(p.text);
      if (p.word && p.state === "changed" && (left.get(k) ?? 0) > 0) {
        p.state = "moved";
        left.set(k, left.get(k)! - 1);
      }
    }
    for (let i = 1; i < ps.length - 1; i++) {
      if (ps[i].state !== "changed" || ps[i].word) continue;
      let k = i;
      while (k < ps.length && ps[k].state === "changed" && !ps[k].word) k++;
      const prev = ps[i - 1];
      const next = ps[k];
      if (prev?.state === "moved" && next?.state === "moved" && ps.slice(i, k).every((p) => p.text !== "-" && p.text !== "–")) {
        for (let q = i; q < k; q++) ps[q].state = "moved";
      }
      i = k - 1;
    }
  }
}

/** Pieces in runs. A space or sign left alone between two changes joins
 *  them, so the highlight does not break into bits. */
function segments(ps: Piece[]): Seg[] {
  const runs: { text: string; state: State; word: boolean }[] = [];
  for (const p of ps) {
    const last = runs[runs.length - 1];
    if (last && last.state === p.state) {
      last.text += p.text;
      last.word ||= p.word || isWord(p.text);
    } else runs.push({ text: p.text, state: p.state, word: p.word || isWord(p.text) });
  }
  for (let i = 1; i < runs.length - 1; i++) {
    const [before, mid, after] = [runs[i - 1], runs[i], runs[i + 1]];
    if (mid.state === "same" && !mid.word && before.state === "changed" && after.state === "changed") {
      runs.splice(i - 1, 3, { text: before.text + mid.text + after.text, state: "changed", word: true });
      i--;
    }
  }
  return runs.map((r) => ({ text: r.text, changed: r.state !== "same", ...(r.state === "moved" ? { moved: true } : {}) }));
}

/** Segments of `a` (removed) and of `b` (added). */
export function diff(a: string, b: string): [Seg[], Seg[]] {
  const x = tokens(a);
  const y = tokens(b);
  const [kx, ky] = lcs(x, y);
  const pa: Piece[] = [];
  const pb: Piece[] = [];
  const add = (to: Piece[], t: string, state: State) => to.push({ text: t, state, word: isWord(t) });
  let i = 0;
  let j = 0;
  while (i < x.length || j < y.length) {
    if (i < x.length && j < y.length && kx[i] && ky[j]) {
      add(pa, x[i++], "same");
      add(pb, y[j++], "same");
      continue;
    }
    const ra: string[] = [];
    const rb: string[] = [];
    while (i < x.length && !kx[i]) ra.push(x[i++]);
    while (j < y.length && !ky[j]) rb.push(y[j++]);
    const fine = ra.length === 1 && rb.length === 1 ? refine(ra[0], rb[0]) : null;
    if (fine) {
      pa.push(...fine[0]);
      pb.push(...fine[1]);
    } else {
      for (const t of ra) add(pa, t, "changed");
      for (const t of rb) add(pb, t, "changed");
    }
  }
  markMoved(pa, pb);
  return [segments(pa), segments(pb)];
}
