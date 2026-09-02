// The reconciliation half of `@pyde-net/asc-transform`: read the
// `[functions.*]` intent out of `otigen.toml` and cross-check it against the
// `@view` / `@mutating` / `@payable` / `@entry` markers the transform found in
// the AssemblyScript source.
//
// The manifest is authoritative. Nothing here writes to it, derives an ABI
// from it, or feeds anything back into the compile — the only two outcomes
// are "agrees" (build proceeds untouched) and "disagrees" (hard error naming
// both sides).
//
// Deliberately dependency-free. This module runs inside `asc`, on the build
// path of every contract that opts in; a TOML dependency would put a
// third-party package in the trust boundary of a check whose entire job is to
// tell the author the truth about their manifest. The scanner below therefore
// implements the slice of TOML a manifest can express — enough to find
// `[functions.<name>]` tables and read their `attributes` array without ever
// mistaking a `#` inside a string for a comment.

import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

// ─────────────────────────────────────────────────────────────────────
// Intent model
// ─────────────────────────────────────────────────────────────────────

/** The four markers this transform understands. Nothing else is touched. */
export const INTENT_MARKERS = ["entry", "view", "mutating", "payable"] as const;

export type IntentMarker = (typeof INTENT_MARKERS)[number];

export function isIntentMarker(name: string): name is IntentMarker {
  return (INTENT_MARKERS as readonly string[]).includes(name);
}

/** One annotated function, as found in the AS source. */
export interface DeclaredIntent {
  /** The `[functions.<name>]` key this declaration claims to implement. */
  readonly fn: string;
  /** The AS declaration the markers sit on (e.g. `__get_impl`). */
  readonly declaredAs: string;
  /** Markers in source order, deduplicated. */
  readonly markers: readonly IntentMarker[];
  /** Source path, as `asc` normalized it. */
  readonly file: string;
  /** 1-based line of the declaration. */
  readonly line: number;
}

/** One `[functions.<name>]` table. */
export interface ManifestFunction {
  readonly name: string;
  readonly attributes: readonly string[];
  /** 1-based line of the `attributes = [...]` key, or of the table header. */
  readonly line: number;
}

export interface Manifest {
  /** Absolute path the manifest was read from. */
  readonly path: string;
  readonly functions: ReadonlyMap<string, ManifestFunction>;
}

/**
 * Map an AssemblyScript declaration name to the manifest key it implements.
 *
 * otigen's generated dispatch seam puts the author's typed body in
 * `__<fn>_impl` and owns the `() -> ()` export itself, so that is the name the
 * markers land on. Contracts predating the seam export the entry directly, so
 * a bare name maps to itself.
 */
export function functionNameFor(declaredAs: string): string {
  const m = /^__(.+)_impl$/.exec(declaredAs);
  return m ? m[1] : declaredAs;
}

// ─────────────────────────────────────────────────────────────────────
// Manifest discovery + loading
// ─────────────────────────────────────────────────────────────────────

export const MANIFEST_FILE = "otigen.toml";

/** How far up the tree to look for a manifest before giving up. */
const MAX_MANIFEST_WALK = 6;

/**
 * Locate `otigen.toml`, starting at each of `startDirs` in order and walking
 * up to {@link MAX_MANIFEST_WALK} parents from each. `asc` runs with the
 * project directory as its cwd (`npm run build`, or `otigen build` driving
 * `npm` in the project dir), so the first candidate normally hits.
 *
 * `PYDE_OTIGEN_MANIFEST` overrides the search entirely — an escape hatch for
 * layouts where the manifest doesn't sit above the AS sources.
 */
export function findManifest(startDirs: readonly string[]): string | null {
  const override = process.env.PYDE_OTIGEN_MANIFEST;
  if (override && override.length > 0) return resolve(override);

  for (const start of startDirs) {
    let dir = resolve(start);
    for (let i = 0; i < MAX_MANIFEST_WALK; i++) {
      const candidate = join(dir, MANIFEST_FILE);
      try {
        readFileSync(candidate);
        return candidate;
      } catch {
        // not here; keep walking
      }
      const parent = dirname(dir);
      if (parent === dir) break;
      dir = parent;
    }
  }
  return null;
}

export function loadManifest(path: string): Manifest {
  const text = readFileSync(path, "utf8");
  return parseManifest(text, path);
}

// ─────────────────────────────────────────────────────────────────────
// The TOML subset scanner
// ─────────────────────────────────────────────────────────────────────

/** Thrown when the manifest can't be read the way this scanner expects. */
export class ManifestParseError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ManifestParseError";
  }
}

interface Cursor {
  readonly text: string;
  i: number;
  line: number;
}

const BARE_KEY = /[A-Za-z0-9_-]/;

function advance(c: Cursor, to: number): void {
  for (let k = c.i; k < to; k++) {
    if (c.text.charCodeAt(k) === 10) c.line++;
  }
  c.i = to;
}

/** Consume a quoted string starting at `c.i`, returning its decoded value. */
function readQuoted(c: Cursor): string {
  const quote = c.text[c.i];
  const triple = c.text.startsWith(quote.repeat(3), c.i);
  const delim = triple ? quote.repeat(3) : quote;
  const literal = quote === "'";

  let i = c.i + delim.length;
  let out = "";
  while (i < c.text.length) {
    if (!literal && c.text[i] === "\\") {
      // Basic strings escape; the manifest's attribute vocabulary never needs
      // more than the simple forms, and anything else passes through as-is so
      // an exotic escape can't be silently mangled into a different value.
      const next = c.text[i + 1];
      switch (next) {
        case "n": out += "\n"; break;
        case "t": out += "\t"; break;
        case "r": out += "\r"; break;
        case '"': out += '"'; break;
        case "\\": out += "\\"; break;
        default: out += "\\" + (next ?? ""); break;
      }
      i += 2;
      continue;
    }
    if (c.text.startsWith(delim, i)) {
      const end = i + delim.length;
      advance(c, end);
      return out;
    }
    out += c.text[i];
    i++;
  }
  throw new ManifestParseError(`unterminated string starting on line ${c.line}`);
}

/**
 * Read a dotted key path (`functions.get`, `functions."my-fn"`), stopping
 * before the terminator (`]` for a table header, `=` for a key/value pair).
 */
function readKeyPath(c: Cursor, terminators: string): string[] {
  const parts: string[] = [];
  for (;;) {
    while (c.i < c.text.length && (c.text[c.i] === " " || c.text[c.i] === "\t")) c.i++;
    const ch = c.text[c.i];
    if (ch === undefined || terminators.includes(ch)) break;

    if (ch === '"' || ch === "'") {
      parts.push(readQuoted(c));
    } else if (BARE_KEY.test(ch)) {
      let j = c.i;
      while (j < c.text.length && BARE_KEY.test(c.text[j])) j++;
      parts.push(c.text.slice(c.i, j));
      c.i = j;
    } else {
      throw new ManifestParseError(`unexpected character ${JSON.stringify(ch)} in key on line ${c.line}`);
    }

    while (c.i < c.text.length && (c.text[c.i] === " " || c.text[c.i] === "\t")) c.i++;
    if (c.text[c.i] === ".") {
      c.i++;
      continue;
    }
    break;
  }
  if (parts.length === 0) {
    throw new ManifestParseError(`empty key on line ${c.line}`);
  }
  return parts;
}

/**
 * Consume one value, returning its raw text. Handles every TOML value shape
 * opaquely: strings (basic / literal / multi-line), arrays and inline tables
 * (balanced, possibly spanning lines, possibly containing comments), and bare
 * scalars up to the end of the line.
 */
function readValue(c: Cursor): string {
  while (c.i < c.text.length && (c.text[c.i] === " " || c.text[c.i] === "\t")) c.i++;
  const start = c.i;
  let depth = 0;

  while (c.i < c.text.length) {
    const ch = c.text[c.i];

    if (ch === '"' || ch === "'") {
      readQuoted(c);
      continue;
    }
    if (ch === "#") {
      if (depth === 0) break;
      // Comment inside a multi-line array / inline table: skip to EOL.
      while (c.i < c.text.length && c.text[c.i] !== "\n") c.i++;
      continue;
    }
    if (ch === "[" || ch === "{") {
      depth++;
      c.i++;
      continue;
    }
    if (ch === "]" || ch === "}") {
      depth--;
      c.i++;
      if (depth <= 0) break;
      continue;
    }
    if (ch === "\n") {
      if (depth === 0) break;
      c.line++;
      c.i++;
      continue;
    }
    c.i++;
  }

  if (depth > 0) {
    throw new ManifestParseError(`unterminated value starting on line ${c.line}`);
  }
  return c.text.slice(start, c.i).trim();
}

/**
 * Interpret a raw value as a flat array of strings.
 *
 * Returns `null` when the value isn't that shape. Callers decide whether that
 * is an error — for `attributes` it is, because quietly ignoring an
 * unreadable attribute list would turn a real intent mismatch into a pass.
 */
export function parseStringArray(raw: string): string[] | null {
  if (!raw.startsWith("[") || !raw.endsWith("]")) return null;
  const c: Cursor = { text: raw.slice(1, -1), i: 0, line: 1 };
  const out: string[] = [];

  for (;;) {
    while (c.i < c.text.length && /[\s,]/.test(c.text[c.i])) c.i++;
    if (c.i >= c.text.length) break;
    if (c.text[c.i] === "#") {
      while (c.i < c.text.length && c.text[c.i] !== "\n") c.i++;
      continue;
    }
    if (c.text[c.i] !== '"' && c.text[c.i] !== "'") return null;
    try {
      out.push(readQuoted(c));
    } catch {
      return null;
    }
  }
  return out;
}

/**
 * Read `[functions.*]` out of a manifest.
 *
 * Everything else in the document is scanned (so headers are found reliably)
 * but not interpreted. `[state]`, `[events]`, `[metadata]` and friends stay
 * exclusively the business of `otigen` — this transform never becomes a second
 * reader of the schema.
 */
export function parseManifest(text: string, path: string): Manifest {
  const c: Cursor = { text, i: 0, line: 1 };
  const functions = new Map<string, ManifestFunction>();

  // The table we're currently inside, as a dotted path.
  let table: string[] = [];

  while (c.i < text.length) {
    const ch = text[c.i];

    if (ch === "\n") {
      c.line++;
      c.i++;
      continue;
    }
    if (ch === " " || ch === "\t" || ch === "\r") {
      c.i++;
      continue;
    }
    if (ch === "#") {
      while (c.i < text.length && text[c.i] !== "\n") c.i++;
      continue;
    }

    if (ch === "[") {
      const headerLine = c.line;
      c.i++;
      const arrayOfTables = text[c.i] === "[";
      if (arrayOfTables) c.i++;
      table = readKeyPath(c, "]");
      while (c.i < text.length && text[c.i] === "]") c.i++;

      if (!arrayOfTables && table.length === 2 && table[0] === "functions") {
        const name = table[1];
        // Record the table even when it declares no attributes: a function
        // that exists with an empty attribute list is a different thing from
        // one the manifest never mentions.
        if (!functions.has(name)) {
          functions.set(name, { name, attributes: [], line: headerLine });
        }
      }
      continue;
    }

    // key = value
    const keyLine = c.line;
    const key = readKeyPath(c, "=");
    while (c.i < text.length && (text[c.i] === " " || text[c.i] === "\t")) c.i++;
    if (text[c.i] !== "=") {
      throw new ManifestParseError(
        `${path}:${keyLine}: expected \`=\` after key \`${key.join(".")}\``,
      );
    }
    c.i++;
    const raw = readValue(c);

    const inFunctionTable = table.length === 2 && table[0] === "functions";
    if (inFunctionTable && key.length === 1 && key[0] === "attributes") {
      const name = table[1];
      const attributes = parseStringArray(raw);
      if (attributes === null) {
        throw new ManifestParseError(
          `${path}:${keyLine}: [functions.${name}].attributes is not a list of strings ` +
            `(got \`${raw}\`) — the intent transform cannot cross-check this entry`,
        );
      }
      functions.set(name, { name, attributes, line: keyLine });
    }
  }

  return { path, functions };
}

// ─────────────────────────────────────────────────────────────────────
// Reconciliation
// ─────────────────────────────────────────────────────────────────────

/** One disagreement between a decorator and the manifest. */
export interface Problem {
  readonly intent: DeclaredIntent;
  /** What the source claims. */
  readonly source: string;
  /** What the manifest says (with the line it says it on). */
  readonly manifest: string;
  /** The two ways out, in the author's words. */
  readonly fix: string;
}

export class IntentMismatchError extends Error {
  readonly problems: readonly Problem[];

  constructor(message: string, problems: readonly Problem[]) {
    super(message);
    this.name = "IntentMismatch";
    this.problems = problems;
    // `asc` prints `error.stack` verbatim on a transform failure. A JS stack
    // trace through the compiler's internals tells the contract author
    // nothing, so the message IS the stack.
    this.stack = message;
  }
}

function attrList(fn: ManifestFunction): string {
  return `attributes = [${fn.attributes.map((a) => `"${a}"`).join(", ")}]`;
}

/**
 * Compare every annotated function against the manifest. Returns the full list
 * of disagreements (never throws on a mismatch — the caller decides how to
 * surface them) so one build reports every problem rather than one per run.
 *
 * ## The rules
 *
 * Annotation is opt-in **per function**. A function with no markers is not
 * checked at all — that is what keeps the Phase 2–4 generated substrate, and
 * every contract written before this package existed, building untouched.
 *
 * Once a function carries any marker:
 *
 * - `@view` requires `view` in the manifest; `@mutating` requires its absence.
 *   Declaring neither asserts nothing and is checked as nothing.
 * - `@payable` is symmetric: the marker and the manifest attribute must both
 *   be present or both absent. Value acceptance is the one axis where a silent
 *   omission reads as a safety claim ("this function refuses value") the
 *   manifest may not be making.
 * - `@entry` requires `entry` in the manifest. Its absence asserts nothing —
 *   almost every function is an entry, and forcing `@entry` alongside `@view`
 *   would be noise.
 * - The named function must exist in `[functions.*]`. A marker on something the
 *   manifest never declares is a typo, not an ABI.
 */
export function reconcile(
  intents: readonly DeclaredIntent[],
  manifest: Manifest,
): Problem[] {
  const problems: Problem[] = [];

  for (const intent of intents) {
    const markers = new Set(intent.markers);

    if (markers.has("view") && markers.has("mutating")) {
      problems.push({
        intent,
        source: "@view and @mutating on the same function",
        manifest: `${manifest.path}: a function either reads state or writes it`,
        fix: "keep exactly one of @view / @mutating",
      });
      continue;
    }

    const fn = manifest.functions.get(intent.fn);
    if (!fn) {
      const known = [...manifest.functions.keys()].sort();
      problems.push({
        intent,
        source: `intent markers on \`${intent.declaredAs}\``,
        manifest: `${manifest.path}: no [functions.${intent.fn}] table`,
        fix:
          known.length > 0
            ? `declare [functions.${intent.fn}], or rename to one of: ${known.join(", ")}`
            : `declare [functions.${intent.fn}] in ${MANIFEST_FILE}`,
      });
      continue;
    }

    const manifestView = fn.attributes.includes("view");
    const manifestPayable = fn.attributes.includes("payable");
    // `@entry` is STRUCTURAL, not an intent claim: it means "this is
    // chain-facing, generate the dispatch shim for it". A constructor is
    // chain-facing too — it just runs once, at deploy — so it satisfies the
    // marker. This mirrors Rust, where `#[pyde::entry]` sits above a
    // constructor without complaint.
    //
    // Requiring a separate `@constructor` marker would diverge from Rust for
    // no safety gain: the engine already excludes constructors from the
    // public dispatch surface (HOST_FN_ABI §3.5), so "callable once, at
    // deploy" is enforced where it belongs rather than by an annotation.
    //
    // `@view` / `@mutating` / `@payable` stay genuine intent claims, because
    // asserting "this does not mutate" against a manifest that says it does
    // is a real contradiction. "This is an entry point" is not.
    const manifestEntry =
      fn.attributes.includes("entry") || fn.attributes.includes("constructor");

    if (markers.has("view") && !manifestView) {
      problems.push({
        intent,
        source: "@view — declares this entry never writes state",
        manifest: `${manifest.path}:${fn.line}: ${attrList(fn)} — no "view", so the ABI marks it mutating`,
        fix: `drop @view, or add "view" to [functions.${fn.name}].attributes`,
      });
    }
    if (markers.has("mutating") && manifestView) {
      problems.push({
        intent,
        source: "@mutating — declares this entry writes state",
        manifest: `${manifest.path}:${fn.line}: ${attrList(fn)} — "view", so the ABI marks it read-only`,
        fix: `drop @mutating, or remove "view" from [functions.${fn.name}].attributes`,
      });
    }

    if (markers.has("payable") && !manifestPayable) {
      problems.push({
        intent,
        source: "@payable — declares this entry accepts attached value",
        manifest: `${manifest.path}:${fn.line}: ${attrList(fn)} — no "payable", so the engine rejects any value sent to it`,
        fix: `drop @payable, or add "payable" to [functions.${fn.name}].attributes`,
      });
    }
    if (!markers.has("payable") && manifestPayable) {
      problems.push({
        intent,
        source: `no @payable on an annotated function (markers: ${[...markers].map((m) => "@" + m).join(" ")})`,
        manifest: `${manifest.path}:${fn.line}: ${attrList(fn)} — "payable", so this entry DOES accept value`,
        fix: `add @payable, or remove "payable" from [functions.${fn.name}].attributes`,
      });
    }

    if (markers.has("entry") && !manifestEntry) {
      problems.push({
        intent,
        source: "@entry — declares this a chain-facing entry point",
        manifest: `${manifest.path}:${fn.line}: ${attrList(fn)} — neither "entry" nor "constructor"`,
        fix: `drop @entry, or add "entry" (or "constructor") to [functions.${fn.name}].attributes`,
      });
    }
  }

  return problems;
}

export function formatProblems(problems: readonly Problem[], manifest: Manifest): string {
  const lines: string[] = [
    "IntentMismatch: function intent in AssemblyScript disagrees with otigen.toml",
    "",
  ];

  problems.forEach((p, idx) => {
    lines.push(
      `  ${idx + 1}. ${p.intent.file}:${p.intent.line} — \`${p.intent.declaredAs}\`` +
        (p.intent.declaredAs === p.intent.fn ? "" : ` (function \`${p.intent.fn}\`)`),
    );
    lines.push(`       source      : ${p.source}`);
    lines.push(`       otigen.toml : ${p.manifest}`);
    lines.push(`       fix         : ${p.fix}`);
    lines.push("");
  });

  lines.push(
    `[functions.*] in ${manifest.path} is the single source of truth for the ABI.`,
    "@view / @mutating / @payable / @entry are checked against it and never merged",
    "into it — no marker can add, remove, or reshape an entry point.",
  );

  return lines.join("\n");
}

/** Reconcile and throw {@link IntentMismatchError} if anything disagrees. */
export function assertIntentMatchesManifest(
  intents: readonly DeclaredIntent[],
  manifest: Manifest,
): void {
  const problems = reconcile(intents, manifest);
  if (problems.length > 0) {
    throw new IntentMismatchError(formatProblems(problems, manifest), problems);
  }
}
