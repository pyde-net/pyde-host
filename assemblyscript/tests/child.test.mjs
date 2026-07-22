// Chainless conformance test for assembly/child.ts against the shared
// golden vectors (vectors/child_address.json, repo root).
//
// Compiles tests/entry.ts (pointer-friendly wrappers over the
// StaticArray helpers) with the local asc, instantiates the .wasm in
// Node with every "pyde" import stubbed (throwing — the pure helpers
// must never reach a host fn; hash_poseidon2 alone is a vector-lookup
// oracle so childAddress's pointer/length plumbing is exercised too),
// then replays EVERY vector. Plain `node`, no test framework, no chain.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const asDir = fileURLToPath(new URL("..", import.meta.url));
const vectorsPath = fileURLToPath(
  new URL("../../vectors/child_address.json", import.meta.url),
);

const golden = JSON.parse(readFileSync(vectorsPath, "utf8"));
const vectors = golden.vectors;
assert.ok(Array.isArray(vectors) && vectors.length > 0, "golden vectors empty");

// ── compile ──────────────────────────────────────────────────────────

const tmp = mkdtempSync(join(tmpdir(), "pyde-host-as-"));
const wasmPath = join(tmp, "host-test.wasm");
try {
  execFileSync(
    "npx",
    ["asc", "tests/entry.ts", "--outFile", wasmPath, "--target", "release"],
    { cwd: asDir, stdio: "inherit" },
  );

  // ── instantiate with stubbed pyde imports ──────────────────────────

  // Wire names mirrored from assembly/host_fns.ts (§7.1–§7.12).
  const wireNames = [
    "sload", "sstore", "sdelete",
    "sstore_scalar", "sload_scalar", "sdelete_scalar",
    "sstore_map1", "sload_map1", "sdelete_map1",
    "sstore_map2", "sload_map2", "sdelete_map2",
    "sstore_map3", "sload_map3", "sdelete_map3",
    "balance", "transfer",
    "caller", "origin", "self_address",
    "wave_id", "wave_timestamp", "chain_id",
    "tx_hash", "tx_value", "tx_gas_remaining",
    "calldata_size", "calldata_copy",
    "emit_event",
    "hash_blake3", "hash_poseidon2", "hash_keccak256",
    "falcon_verify",
    "cross_call", "cross_call_static", "delegate_call",
    "return", "revert",
    "consume_gas",
    "beacon_get",
    "instantiate",
  ];

  let mem; // assigned after instantiation
  const bytesAt = (ptr, len) => new Uint8Array(mem.buffer, ptr, len);
  const hex = (u8) => Buffer.from(u8).toString("hex");
  const unhex = (h) => Uint8Array.from(Buffer.from(h, "hex"));

  // preimage-hex → child_address-hex, straight from the golden set.
  const oracle = new Map(vectors.map((v) => [v.preimage, v.child_address]));

  const pyde = {};
  for (const name of wireNames) {
    pyde[name] = () => {
      throw new Error(`pyde.${name} reached — this conformance test is chainless`);
    };
  }
  pyde.hash_poseidon2 = (inPtr, inLen, outPtr) => {
    const preimage = hex(bytesAt(inPtr, inLen));
    const child = oracle.get(preimage);
    if (!child) {
      throw new Error(`hash_poseidon2 called with unpinned preimage ${preimage}`);
    }
    bytesAt(outPtr, 32).set(unhex(child));
  };

  const env = {
    abort(msgPtr, filePtr, line, col) {
      const asStr = (p) => {
        try {
          const n = new DataView(mem.buffer).getUint32(p - 4, true);
          return Buffer.from(mem.buffer, p, n).toString("utf16le");
        } catch {
          return "<unreadable>";
        }
      };
      throw new Error(`AS abort: ${asStr(msgPtr)} (${asStr(filePtr)}:${line}:${col})`);
    },
  };

  const { instance } = await WebAssembly.instantiate(readFileSync(wasmPath), {
    pyde,
    env,
  });
  const x = instance.exports;
  mem = x.memory;

  const setIn = (ptr, hexStr) => bytesAt(ptr, 32).set(unhex(hexStr));

  const childPreimage = (parent, template, salt) => {
    setIn(x.aPtr(), parent);
    setIn(x.bPtr(), template);
    setIn(x.cPtr(), salt);
    return hex(bytesAt(x.outPtr(), x.runChildPreimage()));
  };
  const childAddress = (parent, template, salt) => {
    setIn(x.aPtr(), parent);
    setIn(x.bPtr(), template);
    setIn(x.cPtr(), salt);
    return hex(bytesAt(x.outPtr(), x.runChildAddress()));
  };
  const unorderedPairEncoding = (a, b) => {
    setIn(x.aPtr(), a);
    setIn(x.bPtr(), b);
    return hex(bytesAt(x.outPtr(), x.runUnorderedPairEncoding()));
  };

  // ── replay every vector ────────────────────────────────────────────

  for (const v of vectors) {
    const preimage = childPreimage(v.parent, v.template, v.salt);
    assert.equal(preimage.length, 107 * 2, `${v.name}: preimage not 107 bytes`);
    assert.equal(preimage, v.preimage, `${v.name}: preimage mismatch`);

    // Full derivation through the stubbed host fn: proves childAddress
    // hashes EXACTLY the canonical preimage and copies out 32 bytes.
    const addr = childAddress(v.parent, v.template, v.salt);
    assert.equal(addr, v.child_address, `${v.name}: child_address mismatch`);
  }

  // ── unordered-pair canonical encoding ──────────────────────────────
  // EVERY pair vector (incl. the 0x7f/0x80 sign-boundary one, which a
  // signed byte comparator would sort the other way).

  const pairs = vectors.filter((v) => v.salt_source_pair_args);
  assert.ok(pairs.length >= 2, "pair vectors missing from golden set");
  for (const pair of pairs) {
    const [a, b] = pair.salt_source_pair_args; // recorded DELIBERATELY unsorted

    const sorted = unorderedPairEncoding(a, b);
    assert.equal(sorted.length, 64 * 2, `${pair.name}: encoding not 64 bytes`);
    assert.equal(
      sorted,
      pair.salt_source_borsh,
      `${pair.name}: sorted encoding != salt_source_borsh`,
    );
    assert.equal(
      unorderedPairEncoding(b, a),
      sorted,
      `${pair.name}: argument order leaked into the encoding`,
    );
    assert.notEqual(
      a + b,
      sorted,
      `${pair.name}: naive unsorted concat must differ (pair_args are unsorted on purpose)`,
    );
  }

  console.log(
    `child.test: ${vectors.length}/${vectors.length} vectors OK ` +
      "(preimage + child_address via hash_poseidon2 oracle); " +
      `unordered-pair sort pinned (${pairs.length} pairs incl. sign boundary)`,
  );
} finally {
  rmSync(tmp, { recursive: true, force: true });
}
