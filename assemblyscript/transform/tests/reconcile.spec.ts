// Tests for the intent transform.
//
// Two layers, and the second is the one that matters most:
//
//   1. Unit tests over the manifest reader and the reconciliation rules —
//      every class of drift the transform is supposed to catch, and every
//      shape it must NOT complain about.
//   2. Integration tests that run the real `asc` over a real project with the
//      real transform loaded. These prove the property the whole design rests
//      on: the transform is **additive**. Annotating a contract does not
//      change one byte of the wasm, and deleting the transform from the build
//      still produces that same deployable wasm — only the cross-check is
//      lost.

import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, describe, test } from "node:test";

import asc from "assemblyscript/asc";
import type { Source } from "assemblyscript";
import type { Transform } from "assemblyscript/transform";

import PydeIntentTransform, {
  collectIntents,
  isSupportedAscVersion,
  SUPPORTED_ASC_RANGE,
} from "../index.js";
import {
  functionNameFor,
  ManifestParseError,
  parseManifest,
  parseStringArray,
  reconcile,
  type DeclaredIntent,
  type IntentMarker,
  type Manifest,
} from "../reconcile.js";

// ─────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────

const tempDirs: string[] = [];

function scratch(prefix = "pyde-intent-"): string {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  tempDirs.push(dir);
  return dir;
}

after(() => {
  for (const dir of tempDirs) rmSync(dir, { recursive: true, force: true });
});

function manifestOf(text: string): Manifest {
  return parseManifest(text, "/proj/otigen.toml");
}

function intent(
  declaredAs: string,
  markers: IntentMarker[],
  line = 10,
): DeclaredIntent {
  return {
    fn: functionNameFor(declaredAs),
    declaredAs,
    markers,
    file: "assembly/contract.ts",
    line,
  };
}

const MANIFEST = `
[contract]
name = "demo"

[state]
schema = [
    { name = "counter", type = "uint64" },
]

[functions.increment]
attributes = ["entry"]
inputs     = []
outputs    = ["uint64"]

[functions.get]
attributes = ["entry", "view"]

[functions.deposit]
attributes = ["entry", "payable"]
`;

// ─────────────────────────────────────────────────────────────────────
// Manifest reader
// ─────────────────────────────────────────────────────────────────────

describe("parseManifest", () => {
  test("reads [functions.*] attributes and ignores everything else", () => {
    const m = manifestOf(MANIFEST);
    assert.deepEqual([...m.functions.keys()].sort(), ["deposit", "get", "increment"]);
    assert.deepEqual(m.functions.get("get")!.attributes, ["entry", "view"]);
    assert.deepEqual(m.functions.get("deposit")!.attributes, ["entry", "payable"]);
    assert.deepEqual(m.functions.get("increment")!.attributes, ["entry"]);
  });

  test("cites the line the attributes are declared on", () => {
    const m = manifestOf('[functions.get]\nattributes = ["entry", "view"]\n');
    assert.equal(m.functions.get("get")!.line, 2);
  });

  test("handles multi-line arrays, trailing commas, and inline comments", () => {
    const m = manifestOf(`
[functions.transfer]
attributes = [
    "entry",     # chain-facing
    "payable",   # accepts value
]
`);
    assert.deepEqual(m.functions.get("transfer")!.attributes, ["entry", "payable"]);
  });

  test("does not mistake # or ] inside a string for syntax", () => {
    const m = manifestOf(`
[contract]
description = "a ] bracket and a # hash walk into a bar"

[functions.get]
attributes = ["entry", "view"]
`);
    assert.deepEqual(m.functions.get("get")!.attributes, ["entry", "view"]);
  });

  test("accepts a quoted table key", () => {
    const m = manifestOf('[functions."odd-name"]\nattributes = ["entry"]\n');
    assert.deepEqual(m.functions.get("odd-name")!.attributes, ["entry"]);
  });

  test("records a function declared without attributes", () => {
    const m = manifestOf("[functions.bare]\ninputs = []\n");
    assert.deepEqual(m.functions.get("bare")!.attributes, []);
  });

  test("ignores [[arrays.of.tables]] and deeper [functions.x.y] tables", () => {
    const m = manifestOf(`
[[tests]]
name = "t"

[functions.get]
attributes = ["entry", "view"]

[functions.get.nested]
attributes = ["not-a-function"]
`);
    assert.equal(m.functions.size, 1);
    assert.deepEqual(m.functions.get("get")!.attributes, ["entry", "view"]);
  });

  // Silently ignoring an attribute list it can't read would turn a real
  // mismatch into a pass — the one failure mode this check must not have.
  test("throws rather than guessing when attributes is not a string list", () => {
    assert.throws(
      () => manifestOf("[functions.get]\nattributes = 42\n"),
      (err: unknown) =>
        err instanceof ManifestParseError && /attributes is not a list of strings/.test(String(err.message)),
    );
  });

  test("parseStringArray rejects non-arrays and nested arrays", () => {
    assert.deepEqual(parseStringArray('["a", "b"]'), ["a", "b"]);
    assert.equal(parseStringArray("42"), null);
    assert.equal(parseStringArray('[["a"]]'), null);
  });
});

describe("functionNameFor", () => {
  test("maps the generated-dispatch seam to its manifest key", () => {
    assert.equal(functionNameFor("__get_impl"), "get");
    assert.equal(functionNameFor("__transfer_from_impl"), "transfer_from");
  });

  test("leaves a directly exported entry alone", () => {
    assert.equal(functionNameFor("get"), "get");
    assert.equal(functionNameFor("__weird"), "__weird");
  });
});

// ─────────────────────────────────────────────────────────────────────
// Reconciliation rules
// ─────────────────────────────────────────────────────────────────────

describe("reconcile", () => {
  const manifest = manifestOf(MANIFEST);

  test("agreeing markers produce no problems", () => {
    assert.deepEqual(
      reconcile(
        [
          intent("__get_impl", ["entry", "view"]),
          intent("__increment_impl", ["entry", "mutating"]),
          intent("__deposit_impl", ["payable"]),
        ],
        manifest,
      ),
      [],
    );
  });

  test("an unannotated contract is not checked at all", () => {
    assert.deepEqual(reconcile([], manifest), []);
  });

  test("@view on a function the manifest marks mutating fails, citing both", () => {
    const [problem, ...rest] = reconcile([intent("__increment_impl", ["view"])], manifest);
    assert.equal(rest.length, 0);
    assert.match(problem!.source, /@view/);
    assert.match(problem!.manifest, /attributes = \["entry"\]/);
    assert.match(problem!.manifest, /otigen\.toml:11/);
    assert.match(problem!.fix, /\[functions\.increment\]\.attributes/);
  });

  test("@mutating on a function the manifest marks view fails, citing both", () => {
    const [problem] = reconcile([intent("__get_impl", ["mutating"])], manifest);
    assert.match(problem!.source, /@mutating/);
    assert.match(problem!.manifest, /"view"/);
  });

  test("@payable where the manifest declares none fails", () => {
    const [problem] = reconcile([intent("__increment_impl", ["payable"])], manifest);
    assert.match(problem!.source, /@payable/);
    assert.match(problem!.fix, /add "payable" to \[functions\.increment\]\.attributes/);
  });

  // The asymmetric half of the payable rule, and the reason it exists: an
  // annotated function that stays silent about value reads as "refuses value".
  test("a payable entry annotated without @payable fails", () => {
    const [problem] = reconcile([intent("__deposit_impl", ["view"])], manifest).filter((p) =>
      /no @payable/.test(p.source),
    );
    assert.ok(problem, "expected the missing-@payable problem");
    assert.match(problem!.manifest, /DOES accept value/);
  });

  test("@entry on a function the manifest does not mark entry fails", () => {
    const m = manifestOf('[functions.helper]\nattributes = []\n');
    const [problem] = reconcile([intent("__helper_impl", ["entry"])], m);
    assert.match(problem!.source, /@entry/);
  });

  test("@entry is satisfied by a constructor, matching Rust", () => {
    // `@entry` is STRUCTURAL — "this is chain-facing, generate the shim" —
    // not a claim that the manifest says `entry`. A constructor is
    // chain-facing too; it just runs once, at deploy. Rust puts
    // #[pyde::entry] above a constructor for exactly this reason, and the
    // engine already keeps constructors off the public dispatch surface
    // (HOST_FN_ABI §3.5), so a marker re-litigating that buys nothing.
    const m = manifestOf('[functions.configure]\nattributes = ["constructor"]\n');
    assert.deepEqual(reconcile([intent("configure", ["entry"])], m), []);
  });

  test("@entry still fails on a function that is neither entry nor constructor", () => {
    const m = manifestOf('[functions.helper]\nattributes = []\n');
    const [problem] = reconcile([intent("helper", ["entry"])], m);
    assert.match(problem!.manifest, /neither "entry" nor "constructor"/);
  });

  test("markers naming a function the manifest never declares fail", () => {
    const [problem] = reconcile([intent("__nope_impl", ["view"])], manifest);
    assert.match(problem!.manifest, /no \[functions\.nope\] table/);
    assert.match(problem!.fix, /deposit, get, increment/);
  });

  test("@view and @mutating together fail as one problem", () => {
    const problems = reconcile([intent("__get_impl", ["view", "mutating"])], manifest);
    assert.equal(problems.length, 1);
    assert.match(problems[0]!.source, /@view and @mutating/);
  });

  test("every disagreement is reported in one pass", () => {
    const problems = reconcile(
      [intent("__get_impl", ["mutating"]), intent("__increment_impl", ["view"])],
      manifest,
    );
    assert.equal(problems.length, 2);
  });
});

// ─────────────────────────────────────────────────────────────────────
// The AST walk
// ─────────────────────────────────────────────────────────────────────

/**
 * A hand-built AST of plain objects — deliberately NOT `assemblyscript` class
 * instances, and carrying no `kind` ids at all.
 *
 * This is the regression test for the two ways this walk can fail silently:
 * a `NodeKind` comparison inlined at build time against one `asc`'s numeric
 * ids, and an `instanceof` that goes quietly false whenever the project
 * resolves a different `assemblyscript` install than the transform does
 * (linked SDKs, unhoisted trees). Both compile, both pass a same-version
 * integration test, and both wave every mismatch through. If `collectIntents`
 * can read this object, it can read any `asc` that keeps the property shape.
 */
function fakeSource(): Source {
  const decorator = (name: string, start: number) => ({
    name: { text: name },
    args: null,
    range: { start },
  });
  const fn = (name: string, decorators: unknown[], start: number) => ({
    name: { text: name },
    decorators,
    range: { start },
  });

  return {
    normalizedPath: "assembly/contract.ts",
    isLibrary: false,
    lineAt: (pos: number) => pos,
    statements: [
      fn("__get_impl", [decorator("view", 12)], 12),
      fn("__plain_impl", [], 20),
      fn("__inlined_impl", [decorator("inline", 30)], 30),
      { members: [fn("__nested_impl", [decorator("payable", 44)], 44)] },
    ],
  } as unknown as Source;
}

describe("collectIntents", () => {
  test("reads markers off a structurally-shaped AST", () => {
    const source = fakeSource();
    const intents = collectIntents(source);

    assert.deepEqual(
      intents.map((i) => [i.fn, i.markers, i.line]),
      [
        ["get", ["view"], 12],
        ["nested", ["payable"], 44],
      ],
    );
  });

  test("strips only our markers, leaving asc's own decorators in place", () => {
    const source = fakeSource();
    collectIntents(source);

    const statements = source.statements as unknown as {
      name?: { text: string };
      decorators?: { name: { text: string } }[] | null;
    }[];
    const byName = (n: string) => statements.find((s) => s.name?.text === n)!;

    assert.equal(byName("__get_impl").decorators, null, "@view must be removed");
    assert.deepEqual(
      byName("__inlined_impl").decorators?.map((d) => d.name.text),
      ["inline"],
      "@inline is asc's, not ours",
    );
  });

  test("warns on a near-miss marker instead of silently ignoring the typo", () => {
    const decorator = (name: string, start: number) => ({
      name: { text: name },
      args: null,
      range: { start },
    });
    const source = {
      normalizedPath: "assembly/contract.ts",
      isLibrary: false,
      lineAt: (pos: number) => pos,
      statements: [
        {
          name: { text: "__get_impl" },
          decorators: [decorator("veiw", 12)], // transposed @view
          range: { start: 12 },
        },
      ],
    } as unknown as Source;

    const warnings: string[] = [];
    const intents = collectIntents(source, (m) => warnings.push(m));

    // `@veiw` is not a marker, so nothing is collected or checked...
    assert.deepEqual(intents, []);
    // ...but the author is told, so the typo doesn't pass as "unannotated".
    assert.equal(warnings.length, 1);
    assert.match(warnings[0]!, /@veiw/);
    assert.match(warnings[0]!, /did you mean `@view`/);
  });

  test("does not warn on real markers or asc's own decorators", () => {
    // fakeSource carries @view (a marker), @payable (a marker), and @inline
    // (asc's) — none should trip the near-miss warning.
    const warnings: string[] = [];
    collectIntents(fakeSource(), (m) => warnings.push(m));
    assert.deepEqual(warnings, []);
  });
});

describe("asc version gate", () => {
  test("accepts the supported range and rejects outside it", () => {
    assert.ok(isSupportedAscVersion(SUPPORTED_ASC_RANGE.min));
    assert.ok(isSupportedAscVersion("0.28.20"));
    assert.ok(!isSupportedAscVersion(SUPPORTED_ASC_RANGE.max));
    assert.ok(!isSupportedAscVersion("0.27.0"));
    assert.ok(!isSupportedAscVersion("1.0.0"));
  });
});

// ─────────────────────────────────────────────────────────────────────
// Integration: the real compiler, the real transform
// ─────────────────────────────────────────────────────────────────────

const FIXTURE_MANIFEST = `
[contract]
name = "intent-fixture"

[contract.lang]
language = "as"
output   = "build/contract.wasm"

[functions.increment]
attributes = ["entry"]
outputs    = ["uint64"]

[functions.get]
attributes = ["entry", "view"]
outputs    = ["uint64"]
`;

/** The contract body, with `MARK` substituted for the annotation under test. */
function fixtureSource(annotated: boolean): string {
  const mark = (m: string) => (annotated ? `@${m}\n` : "");
  return `let slot: u64 = 0;

${mark("mutating")}export function __increment_impl(): u64 {
  slot += 1;
  return slot;
}

${mark("view")}export function __get_impl(): u64 {
  return slot;
}
`;
}

interface CompileResult {
  error: Error | null;
  stderr: string;
  wasm: Uint8Array | null;
}

async function compile(dir: string, opts: { transform: boolean }): Promise<CompileResult> {
  const out = join(dir, "contract.wasm");
  const { error, stderr } = await asc.main(
    [
      "contract.ts",
      "--outFile",
      out,
      "--runtime",
      "stub",
      "--optimizeLevel",
      "3",
      "--shrinkLevel",
      "2",
      "--noAssert",
      "--baseDir",
      dir,
    ],
    // `asc` accepts either a constructor or an instance here and news up the
    // former after injecting `baseDir` / `stderr` / … onto its prototype —
    // which is the only way the transform gets those, so pass the class.
    opts.transform
      ? { transforms: [PydeIntentTransform as unknown as Transform] }
      : {},
  );
  let wasm: Uint8Array | null = null;
  try {
    wasm = readFileSync(out);
  } catch {
    /* compile failed; leave null */
  }
  return { error, stderr: stderr.toString(), wasm };
}

describe("integration: asc + the transform", () => {
  // Acceptance criterion 3, stated as a byte comparison: the sugar is
  // additive. The same contract, annotated and compiled WITH the transform,
  // must produce the identical wasm it produces un-annotated and WITHOUT it.
  test("annotating a contract changes nothing about the wasm", async () => {
    const annotated = scratch("pyde-intent-annotated-");
    writeFileSync(join(annotated, "otigen.toml"), FIXTURE_MANIFEST);
    writeFileSync(join(annotated, "contract.ts"), fixtureSource(true));

    const plain = scratch("pyde-intent-plain-");
    writeFileSync(join(plain, "otigen.toml"), FIXTURE_MANIFEST);
    writeFileSync(join(plain, "contract.ts"), fixtureSource(false));

    const withTransform = await compile(annotated, { transform: true });
    const without = await compile(plain, { transform: false });

    assert.equal(withTransform.error, null, withTransform.stderr);
    assert.equal(without.error, null, without.stderr);
    assert.ok(withTransform.wasm && without.wasm);
    assert.deepEqual(
      Buffer.from(withTransform.wasm!),
      Buffer.from(without.wasm!),
      "the transform must not influence codegen",
    );
  });

  // Acceptance criterion 3, the other half: pull the transform out of the
  // build and the annotated contract still compiles to that same wasm. Only
  // the cross-check is lost.
  test("removing the transform still yields the same deployable wasm", async () => {
    const dir = scratch("pyde-intent-sugarless-");
    writeFileSync(join(dir, "otigen.toml"), FIXTURE_MANIFEST);
    writeFileSync(join(dir, "contract.ts"), fixtureSource(true));

    const sugarless = await compile(dir, { transform: false });
    assert.equal(sugarless.error, null, sugarless.stderr);

    const reference = scratch("pyde-intent-reference-");
    writeFileSync(join(reference, "otigen.toml"), FIXTURE_MANIFEST);
    writeFileSync(join(reference, "contract.ts"), fixtureSource(false));
    const withTransform = await compile(reference, { transform: true });

    assert.deepEqual(Buffer.from(sugarless.wasm!), Buffer.from(withTransform.wasm!));
  });

  test("a mismatched marker fails the compile, citing decorator and manifest", async () => {
    const dir = scratch("pyde-intent-mismatch-");
    // The manifest says `get` is a view; the source claims it mutates.
    writeFileSync(join(dir, "otigen.toml"), FIXTURE_MANIFEST);
    writeFileSync(
      join(dir, "contract.ts"),
      `let slot: u64 = 0;

@mutating
export function __get_impl(): u64 {
  return slot;
}
`,
    );

    const result = await compile(dir, { transform: true });
    assert.notEqual(result.error, null, "expected the build to fail");
    const message = String(result.error?.message);
    assert.match(message, /IntentMismatch/);
    assert.match(message, /@mutating/);
    assert.match(message, /otigen\.toml/);
    assert.match(message, /\[functions\.get\]\.attributes/);
    assert.match(message, /contract\.ts:3/);
  });

  test("markers with no manifest to check against fail rather than pass quietly", async () => {
    const dir = scratch("pyde-intent-nomanifest-");
    writeFileSync(join(dir, "contract.ts"), fixtureSource(true));

    const result = await compile(dir, { transform: true });
    assert.notEqual(result.error, null, "expected the build to fail");
    assert.match(String(result.error?.message), /no otigen\.toml to check them against/);
  });

  test("`@view()` with arguments is rejected as the typo it is", async () => {
    const dir = scratch("pyde-intent-args-");
    writeFileSync(join(dir, "otigen.toml"), FIXTURE_MANIFEST);
    writeFileSync(
      join(dir, "contract.ts"),
      `@view()
export function __get_impl(): u64 {
  return 0;
}
`,
    );

    const result = await compile(dir, { transform: true });
    assert.notEqual(result.error, null);
    assert.match(String(result.error?.message), /`@view` takes no arguments/);
  });
});
