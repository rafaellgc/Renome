// Builds renome-cli for the packages (Tauri's externalBin, next to the
// window's program in each one): src-tauri/binaries/renome-cli-<triple>,
// and, when it can run here, its man pages and shell completions in
// target/cli-extra for the .deb and .rpm. Run by the package builds
// (tauri.cli.conf.json), not by `tauri dev`.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const host = /host: (\S+)/.exec(execFileSync("rustc", ["-vV"], { encoding: "utf8" }))[1];
const triple = process.env.TAURI_ENV_TARGET_TRIPLE || host;
const exe = triple.includes("windows") ? ".exe" : "";
const bins = join(root, "src-tauri", "binaries");
mkdirSync(bins, { recursive: true });

const cargo = (args) => execFileSync("cargo", ["build", "--release", "-p", "renome-cli", ...args], { cwd: root, stdio: "inherit" });

/** The built program for a target ("" for the host, as cargo puts it). */
function build(target) {
  cargo(target ? ["--target", target] : []);
  return join(root, "target", target || "", "release", `renome-cli${exe}`);
}

let runnable;
if (triple === "universal-apple-darwin") {
  // Both Macs: each architecture's (tauri-build copies them one by one
  // while it builds each) and the two joined for the bundle.
  const arm = build("aarch64-apple-darwin");
  const intel = build("x86_64-apple-darwin");
  copyFileSync(arm, join(bins, "renome-cli-aarch64-apple-darwin"));
  copyFileSync(intel, join(bins, "renome-cli-x86_64-apple-darwin"));
  execFileSync("lipo", ["-create", "-output", join(bins, "renome-cli-universal-apple-darwin"), arm, intel], { stdio: "inherit" });
  runnable = host.startsWith("aarch64") ? arm : intel;
} else {
  const built = build(triple === host ? "" : triple);
  copyFileSync(built, join(bins, `renome-cli-${triple}${exe}`));
  if (triple === host) runnable = built;
}

// Man pages and completions (installed by the Linux packages).
if (runnable && triple.includes("linux")) {
  const extra = join(root, "target", "cli-extra");
  rmSync(extra, { recursive: true, force: true });
  execFileSync(runnable, ["--lang", "en", "manpages", join(extra, "man")], { stdio: "inherit" });
  for (const [shell, file] of [["bash", "renome-cli"], ["zsh", "_renome-cli"], ["fish", "renome-cli.fish"]]) {
    mkdirSync(join(extra, shell), { recursive: true });
    writeFileSync(join(extra, shell, file), execFileSync(runnable, ["--lang", "en", "completions", shell]));
  }
}
