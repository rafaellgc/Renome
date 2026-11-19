// Runs the cases of src/lib/plurals.json on the window's plurals (the
// command line's Rust runs the same in its tests).
import table from "../src/lib/plurals.json" with { type: "json" };
import { plurals } from "../src/lib/plurals.ts";

let failed = 0;
for (const c of table.cases) {
  const got = plurals(c.text, c.vars as Record<string, string | number>);
  if (got !== c.out) {
    failed++;
    console.error(`plurals: ${JSON.stringify(c.text)} ${JSON.stringify(c.vars)} gave ${JSON.stringify(got)}, not ${JSON.stringify(c.out)}`);
  }
}
if (failed) process.exit(1);
console.log(`plurals: ${table.cases.length} cases ok`);
