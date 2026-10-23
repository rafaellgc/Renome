// Manual content: one list of sections per language. The HTML is written
// here (not user input), so it is rendered as is.

export interface Section {
  id: string;
  title: string;
  /** Shown indented in the index (a rule inside "Rules"). */
  sub?: boolean;
  html: string;
}

/** Example of a rename: current name → new name. */
export const ex = (from: string, to: string) =>
  `<div class="ex"><code>${from}</code><span>→</span><code>${to}</code></div>`;
