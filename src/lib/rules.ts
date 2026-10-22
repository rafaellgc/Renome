import { settings } from "./settings.svelte";
import type { Rule, RuleType, Step } from "./types";

export const RULE_TYPES: RuleType[] = [
  "replace", "regex", "rearrange", "case", "insert", "number", "remove", "removeRange", "charAt", "extension",
];

const NAME_VAR = { pt: "{nome}", en: "{name}", es: "{nombre}" } as const;

export function newRule(type: RuleType): Rule {
  switch (type) {
    case "replace":
      return { type, find: "", replace: "", caseSensitive: false, firstOnly: false, target: "name" };
    case "regex":
      return { type, pattern: "", replace: "", caseSensitive: false, firstOnly: false, target: "name" };
    case "case":
      return { type, mode: "titleSmart", target: "name" };
    case "insert":
      return { type, source: "text", text: "", dateFormat: "%Y-%m-%d", fallback: true, at: "start", position: 1 };
    case "number":
      return { type, template: `${NAME_VAR[settings.lang]} {n}`, start: 1, step: 1, digits: 2, perFolder: false };
    case "remove":
      return { type, allSpaces: false, extraSpaces: true, accents: false, digits: false, chars: "", target: "name" };
    case "removeRange":
      return { type, from: "text", fromPosition: 1, fromText: "", to: "text", toPosition: 1, toText: "", inclusive: true };
    case "charAt":
      return { type, position: 1, fromEnd: false, onlyIf: "", caseSensitive: false, replace: "" };
    case "extension":
      return { type, mode: "lower", value: "" };
    case "rearrange":
      return { type, separator: " - ", order: "", target: "name" };
  }
}

/** Same split as the Rust rule: trimmed parts, empty ones dropped. */
export function splitParts(name: string, separator: string): string[] {
  if (!separator) return [];
  return name.split(separator).map((p) => p.trim()).filter(Boolean);
}

/** Splits `name.ext` like the Rust core (hidden files have no extension). */
export function splitExt(name: string): [string, string] {
  const dot = name.lastIndexOf(".");
  return dot > 0 && dot < name.length - 1 ? [name.slice(0, dot), name.slice(dot + 1)] : [name, ""];
}

let counter = 0;
export function newStep(rule: Rule, enabled = true): Step {
  return { id: `s${Date.now().toString(36)}${counter++}`, enabled, rule };
}
