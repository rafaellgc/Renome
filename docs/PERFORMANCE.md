# Performance

Measured with `cargo bench -p renome-core` (criterion, release profile) on a
Linux desktop (2026-11-25, v0.38.1), over 10,000 files in 50 folders.

| What | Time |
|---|---|
| Scan 10,000 files | 17.8 ms |
| Preview of 10,000 files, 2 rules (replace + title case) | 176.8 ms |
| Sort 10,000 files by name | 34.1 ms |
| `natural_cmp` of two long names | 48 ns |

The preview is the figure that matters: it runs after every change to a rule
or a typed name. At 10,000 files it is about 18 µs per file; a typical list
(hundreds of files) previews in a few milliseconds.

Any change to the preview, scan or sort should be checked against this table;
a slowdown of more than about 20% needs a reason in the commit message.
