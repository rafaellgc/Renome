// Character diff between the current and the new name, to highlight what
// a rename changes.

const MIN_RUN = 3;

export interface Seg {
  text: string;
  changed: boolean;
}

/** Splits `chars` into runs of kept/changed characters. A single kept
 *  character between two changes is merged into them, so the highlight
 *  does not break into confetti. */
function segments(chars: string[], kept: boolean[]): Seg[] {
  const segs: Seg[] = [];
  chars.forEach((c, i) => {
    const changed = !kept[i];
    const last = segs[segs.length - 1];
    if (last && last.changed === changed) last.text += c;
    else segs.push({ text: c, changed });
  });
  for (let i = 1; i < segs.length - 1; i++) {
    if (!segs[i].changed && [...segs[i].text].length === 1 && segs[i - 1].changed && segs[i + 1].changed) {
      segs.splice(i - 1, 3, { text: segs[i - 1].text + segs[i].text + segs[i + 1].text, changed: true });
      i--;
    }
  }
  return segs;
}

const isDigit = (c: string | undefined) => c !== undefined && c >= "0" && c <= "9";

/** Longest common subsequence: marks which characters of each side are
 *  kept. Returns the segments of `a` (removed) and of `b` (added). */
export function diff(a: string, b: string): [Seg[], Seg[]] {
  const x = [...a];
  const y = [...b];

  // Common prefix and suffix need no table.
  let pre = 0;
  while (pre < x.length && pre < y.length && x[pre] === y[pre]) pre++;
  let suf = 0;
  while (suf < x.length - pre && suf < y.length - pre && x[x.length - 1 - suf] === y[y.length - 1 - suf]) suf++;

  // Matched runs [start in x, start in y, length]; middle ones flagged.
  const runs: [number, number, number, boolean][] = [[0, 0, pre, false]];
  const n = x.length - pre - suf;
  const m = y.length - pre - suf;
  if (n && m) {
    // dp[i][j] = LCS length of x[pre+i..] and y[pre+j..].
    const w = m + 1;
    const dp = new Uint16Array((n + 1) * w);
    for (let i = n - 1; i >= 0; i--) {
      for (let j = m - 1; j >= 0; j--) {
        dp[i * w + j] =
          x[pre + i] === y[pre + j] ? dp[(i + 1) * w + j + 1] + 1 : Math.max(dp[(i + 1) * w + j], dp[i * w + j + 1]);
      }
    }
    let i = 0;
    let j = 0;
    while (i < n && j < m) {
      if (x[pre + i] === y[pre + j]) {
        const last = runs[runs.length - 1];
        if (last[3] && last[0] + last[2] === pre + i && last[1] + last[2] === pre + j) last[2]++;
        else runs.push([pre + i, pre + j, 1, true]);
        i++;
        j++;
      } else if (dp[(i + 1) * w + j] >= dp[i * w + j + 1]) i++;
      else j++;
    }
  }
  runs.push([x.length - suf, y.length - suf, suf, false]);

  const keptX = new Array<boolean>(x.length).fill(false);
  const keptY = new Array<boolean>(y.length).fill(false);
  for (let [ri, rj, len, middle] of runs) {
    // A number is one unit: digits that only match part of a number on
    // either side (the "0" of "01" → "02") count as changed.
    while (len && isDigit(x[ri]) && (isDigit(x[ri - 1]) || isDigit(y[rj - 1]))) {
      ri++;
      rj++;
      len--;
    }
    while (len && isDigit(x[ri + len - 1]) && (isDigit(x[ri + len]) || isDigit(y[rj + len]))) len--;
    // Short runs in the middle are usually coincidences.
    if (middle && len < MIN_RUN) continue;
    for (let k = 0; k < len; k++) keptX[ri + k] = keptY[rj + k] = true;
  }
  return [segments(x, keptX), segments(y, keptY)];
}
