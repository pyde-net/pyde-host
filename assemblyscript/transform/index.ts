// `@pyde-net/host/transform` — the function-intent sugar for AssemblyScript
// contracts.
//
// Authors annotate what an entry point *does*:
//
//     @view      export function __get_impl(): u64 { … }
//     @mutating  export function __set_impl(v: u64): void { … }
//     @payable   export function __deposit_impl(): void { … }
//
// The transform reads those markers, cross-checks them against
// `[functions.*]` in `otigen.toml`, and fails the build when the two
// disagree. That is the whole feature.
//
// ## What this deliberately is NOT
//
// It is not a schema source. It never injects an export, never synthesizes a
// signature, never edits the manifest, and never reads `[state]` or
// `[events]`. `otigen build` generates the dispatch shims and typed storage
// from the manifest before `asc` ever runs; this pass sees the finished
// substrate and only checks that the author's stated intent matches it.
//
// Two consequences worth stating plainly, because they are the design:
//
//   1. **Removing this transform from `asconfig.json` still yields a correct,
//      deployable contract.** You lose the cross-check, not the ABI. Every
//      byte of the entry surface comes from `otigen.toml` either way.
//   2. **An `asc` bump can at worst disable the sugar.** If the compiler's
//      version falls outside the range this transform was written against,
//      it prints a warning and stands down rather than guessing at an AST it
//      no longer understands. The substrate is untouched by that decision.

import { createRequire } from "node:module";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { Transform } from "assemblyscript/transform";
import type { Parser, Source } from "assemblyscript";

import {
  assertIntentMatchesManifest,
  findManifest,
  functionNameFor,
  INTENT_MARKERS,
  isIntentMarker,
  loadManifest,
  IntentMismatchError,
  MANIFEST_FILE,
  type DeclaredIntent,
  type IntentMarker,
} from "./reconcile.js";

export * from "./reconcile.js";

/**
 * The `asc` versions this transform is known to read correctly — the range in
 * which the decorator AST shape (`DecoratorNode.name`, `.args`, and a
 * `DeclarationStatement.decorators` array we can filter) is stable.
 *
 * Outside it the transform stands down with a warning instead of running.
 * See the module header: the sugar is expendable, the substrate is not.
 */
export const SUPPORTED_ASC_RANGE = { min: "0.27.30", max: "0.29.0" } as const;

const TAG = "@pyde-net/host/transform";

/** Compare `major.minor.patch`, ignoring any pre-release suffix. */
function compareVersions(a: string, b: string): number {
  const parse = (v: string) =>
    v
      .split("-")[0]
      .split(".")
      .map((p) => Number.parseInt(p, 10) || 0);
  const [x, y] = [parse(a), parse(b)];
  for (let i = 0; i < 3; i++) {
    const d = (x[i] ?? 0) - (y[i] ?? 0);
    if (d !== 0) return d;
  }
  return 0;
}

export function isSupportedAscVersion(version: string): boolean {
  return (
    compareVersions(version, SUPPORTED_ASC_RANGE.min) >= 0 &&
    compareVersions(version, SUPPORTED_ASC_RANGE.max) < 0
  );
}

/**
 * The version of the `asc` actually doing this compile.
 *
 * Resolved from the project first and only then from this package's own
 * location: with a linked or unhoisted SDK the two can be different installs,
 * and the compiler running the build is the one whose AST we have to read.
 */
function ascVersion(baseDir: string): string | null {
  const anchors = [pathToFileURL(join(resolve(baseDir), "-")).href, import.meta.url];
  for (const anchor of anchors) {
    try {
      const pkg = createRequire(anchor)("assemblyscript/package.json") as { version?: string };
      if (typeof pkg.version === "string") return pkg.version;
    } catch {
      // not resolvable from here; try the next anchor
    }
  }
  return null;
}

// ─────────────────────────────────────────────────────────────────────
// AST walk
// ─────────────────────────────────────────────────────────────────────
//
// The walk is deliberately STRUCTURAL: it matches on the shape of the nodes
// `asc` hands us, never on `NodeKind` and never on class identity. Both of the
// obvious alternatives fail silently, which is the one failure mode a drift
// checker must not have:
//
//   • `stmt.kind === NodeKind.FunctionDeclaration` — `NodeKind` is a `const
//     enum`, so TypeScript inlines the numeric id of whichever `asc` this
//     package was compiled against. The ids move between minors
//     (`FunctionDeclaration`: 55 in 0.27, 56 in 0.28); a stale literal matches
//     nothing and every mismatch sails through unchecked.
//   • `stmt instanceof FunctionDeclaration` — correct only while there is
//     exactly one `assemblyscript` in the tree. A `file:`/linked SDK (what
//     this repo's own examples use) or an unhoisted install gives the
//     transform a different module instance than the compiler is running, and
//     every `instanceof` is quietly false.
//
// Property shapes, by contrast, are the part of the AST that has been stable
// across every `asc` this package supports.

/** The subset of an `asc` AST node this transform reads. */
interface DecoratedNode {
  readonly name?: { text?: unknown };
  readonly range: { start: number };
  decorators?: DecoratorLike[] | null;
  readonly members?: readonly unknown[];
}

interface DecoratorLike {
  readonly name?: { text?: unknown };
  readonly args?: unknown;
  readonly range: { start: number };
}

function isUserSource(source: Source): boolean {
  return !source.isLibrary && !source.normalizedPath.startsWith("~lib/");
}

function decoratorName(node: DecoratorLike): string | null {
  // A bare identifier (`@view`) carries `name.text`. A property access
  // (`@pyde.view`) does not — that is somebody else's decorator vocabulary and
  // is left alone. `asc`'s own built-ins (`@inline`, `@external`, …) share the
  // identifier shape, so it is the NAME that separates ours from theirs; none
  // of the four markers collides with a built-in.
  const text = node.name?.text;
  return typeof text === "string" ? text : null;
}

// Optimal string alignment distance (Levenshtein plus adjacent
// transposition) between two short strings. Used only to catch a typo'd
// marker; the operands are marker names, so the full table is negligible.
function osaDistance(a: string, b: string): number {
  const m = a.length;
  const n = b.length;
  const d: number[][] = [];
  for (let i = 0; i <= m; i++) {
    d[i] = new Array<number>(n + 1).fill(0);
    d[i][0] = i;
  }
  for (let j = 0; j <= n; j++) d[0][j] = j;
  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      d[i][j] = Math.min(d[i - 1][j] + 1, d[i][j - 1] + 1, d[i - 1][j - 1] + cost);
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) {
        d[i][j] = Math.min(d[i][j], d[i - 2][j - 2] + 1);
      }
    }
  }
  return d[m][n];
}

// The intent marker within one edit (transposition included, so `@veiw` is
// caught) of `name`, or null. This is the guard on the feature's whole
// promise: without it a typo'd marker is silently left on the AST and the
// function it was meant to constrain compiles unchecked.
function nearestMarker(name: string): IntentMarker | null {
  for (const marker of INTENT_MARKERS) {
    if (osaDistance(name, marker) <= 1) return marker;
  }
  return null;
}

/**
 * Split a declaration's decorators into the intent markers we own and
 * everything else, and reject the shapes that are almost certainly a mistake.
 *
 * `warn`, when given, fires for a decorator that is not a marker but is one
 * edit away from one — the near-miss a silent no-op would otherwise hide.
 */
function partitionDecorators(
  node: DecoratedNode,
  source: Source,
  warn?: (message: string) => void,
): { markers: IntentMarker[]; keep: DecoratorLike[] } {
  const markers: IntentMarker[] = [];
  const keep: DecoratorLike[] = [];

  for (const dec of node.decorators ?? []) {
    const name = decoratorName(dec);
    if (name === null || !isIntentMarker(name)) {
      if (warn && name !== null) {
        const near = nearestMarker(name);
        if (near !== null) {
          warn(
            `\`@${name}\` on \`${node.name?.text ?? "?"}\` ` +
              `(${source.normalizedPath}:${source.lineAt(dec.range.start)}) ` +
              `is not an intent marker and is being left UNCHECKED — did you mean ` +
              `\`@${near}\`? The markers are @${INTENT_MARKERS.join(", @")}.`,
          );
        }
      }
      keep.push(dec);
      continue;
    }
    // `@view` parses with `args === null`; `@view()` with an empty array.
    if (dec.args !== null && dec.args !== undefined) {
      throw new IntentMismatchError(
        `${TAG}: \`@${name}\` takes no arguments\n\n` +
          `  ${source.normalizedPath}:${source.lineAt(dec.range.start)}\n\n` +
          `Write \`@${name}\` — the marker states intent; the ABI it is checked ` +
          `against lives in ${MANIFEST_FILE}.`,
        [],
      );
    }
    if (!markers.includes(name)) markers.push(name);
  }

  return { markers, keep };
}

/**
 * Recursively yield every decorated, named declaration reachable from
 * `nodes`, descending through namespaces.
 *
 * It does not ask whether a node is a function: the markers are checked
 * against `[functions.*]` by NAME, so a marker on something that isn't an
 * entry point resolves to "no such function in the manifest" — a better error
 * than the one a node-type test could give, and one less thing to keep in step
 * with `asc`'s AST.
 */
function* decoratedDeclarations(nodes: readonly unknown[]): Generator<DecoratedNode> {
  for (const node of nodes) {
    if (typeof node !== "object" || node === null) continue;
    const candidate = node as DecoratedNode;

    if (Array.isArray(candidate.decorators) && candidate.decorators.length > 0) {
      if (typeof candidate.name?.text === "string") yield candidate;
    }
    if (Array.isArray(candidate.members)) {
      yield* decoratedDeclarations(candidate.members);
    }
  }
}

/**
 * Collect the intent markers in `source` and strip them from the AST.
 *
 * Stripping keeps the markers from ever reaching the compiler proper. Current
 * `asc` ignores unknown custom decorators, but "ignored today" is not a
 * contract — and the promise this package makes is that annotating a function
 * changes nothing about the wasm it compiles to.
 */
export function collectIntents(
  source: Source,
  warn?: (message: string) => void,
): DeclaredIntent[] {
  const intents: DeclaredIntent[] = [];

  for (const decl of decoratedDeclarations(source.statements)) {
    const { markers, keep } = partitionDecorators(decl, source, warn);
    if (markers.length === 0) continue;

    decl.decorators = keep.length > 0 ? keep : null;

    const declaredAs = decl.name!.text as string;
    intents.push({
      fn: functionNameFor(declaredAs),
      declaredAs,
      markers,
      file: source.normalizedPath,
      line: source.lineAt(decl.range.start),
    });
  }

  return intents;
}

// ─────────────────────────────────────────────────────────────────────
// The transform
// ─────────────────────────────────────────────────────────────────────

export default class PydeIntentTransform extends Transform {
  // `asc` assigns these onto the transform's prototype when it loads the
  // module — they are not constructor-initialized, so `declare` states their
  // shape without emitting a field that would shadow the injected value.
  // Declared here rather than leaned on from the base class because the base
  // class ships as a stub whose typed surface has moved between `asc` minors.
  declare readonly baseDir: string;
  declare readonly stderr: { write(chunk: string): void };

  afterParse(parser: Parser): void {
    const version = ascVersion(this.baseDir);
    if (version !== null && !isSupportedAscVersion(version)) {
      this.warn(
        `asc ${version} is outside the supported range ` +
          `(>=${SUPPORTED_ASC_RANGE.min} <${SUPPORTED_ASC_RANGE.max}); ` +
          `@view/@mutating/@payable are NOT being checked against ${MANIFEST_FILE} ` +
          `on this build. The contract's ABI is unaffected — it comes from ` +
          `[functions.*], not from these markers.`,
      );
      return;
    }

    const intents: DeclaredIntent[] = [];
    for (const source of parser.sources) {
      if (!isUserSource(source)) continue;
      intents.push(...collectIntents(source, (m) => this.warn(m)));
    }

    // No annotations, no manifest read, no cost. This is the path every
    // contract written before the sugar existed takes.
    if (intents.length === 0) return;

    const manifestPath = findManifest([this.baseDir, process.cwd()]);
    if (manifestPath === null) {
      const where = intents
        .map((i) => `  ${i.file}:${i.line} — @${i.markers.join(" @")} on \`${i.declaredAs}\``)
        .join("\n");
      throw new IntentMismatchError(
        `${TAG}: found intent markers but no ${MANIFEST_FILE} to check them against\n\n` +
          `${where}\n\n` +
          `Searched upward from ${this.baseDir} and ${process.cwd()}. Set ` +
          `PYDE_OTIGEN_MANIFEST to point at the manifest, or remove the markers.`,
        [],
      );
    }

    assertIntentMatchesManifest(intents, loadManifest(manifestPath));
  }

  private warn(message: string): void {
    this.stderr.write(`WARNING ${TAG}: ${message}\n`);
  }
}
