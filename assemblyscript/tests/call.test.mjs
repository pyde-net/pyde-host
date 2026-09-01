// Chainless behaviour test for assembly/call.ts.
//
// Compiles tests/call_entry.ts with the local asc and instantiates it in
// Node with the three cross-call host fns stubbed. The stub IS the
// fixture: it returns whatever status code the case under test needs and
// writes whatever payload, which is exactly the seam the wrapper has to
// get right. Everything the engine would do beyond that is out of scope
// here and is covered by the on-devnet crosscall example.
//
// Plain `node --test`, no chain.

import assert from "node:assert/strict";
import { test } from "node:test";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const asDir = fileURLToPath(new URL("..", import.meta.url));
const tmp = mkdtempSync(join(tmpdir(), "pyde-host-call-"));
const wasmPath = join(tmp, "call-test.wasm");

execFileSync(
  "npx",
  ["asc", "tests/call_entry.ts", "--outFile", wasmPath, "--target", "release"],
  { cwd: asDir, stdio: "inherit" },
);
const bytes = await readFile(wasmPath);

// Status codes, mirrored from assembly/status.ts.
const STATUS_OK = 0;
const ERR_CROSS_CALL_FAILED = -10;
const ERR_INVALID_FUNCTION_NAME = -13;

/// Instantiate with a cross-call stub that returns `status` and writes
/// `payload` into the caller's out-buffer. `revert` throws so
/// `execOrRevert` is observable.
/// `reportLen` models a callee whose return is LARGER than the buffer: the
/// host fills what fits and reports the true length, which is the signal
/// the wrapper must treat as an overflow. Defaults to the payload length.
async function instantiate({
  status = STATUS_OK,
  payload = new Uint8Array(0),
  reportLen = null,
  capture = {},
} = {}) {
  let mem;
  const writeOut = (outPtr, outLenPtr) => {
    const u8 = new Uint8Array(mem.buffer);
    // Never write past the buffer — a real host does not corrupt caller
    // memory, it reports the true length through outLenPtr and lets the
    // caller decide.
    u8.set(payload, outPtr);
    new DataView(mem.buffer).setInt32(outLenPtr, reportLen ?? payload.length, true);
    return status;
  };
  const notCalled = (name) => () => {
    throw new Error(`unexpected host fn: ${name}`);
  };

  const pyde = {
    cross_call: (_t, _f, _fl, _c, _cl, valuePtr, gas, outPtr, outLenPtr) => {
      capture.kind = "call";
      capture.gas = gas;
      capture.valuePtr = valuePtr;
      return writeOut(outPtr, outLenPtr);
    },
    cross_call_static: (_t, _f, _fl, _c, _cl, gas, outPtr, outLenPtr) => {
      capture.kind = "static";
      capture.gas = gas;
      return writeOut(outPtr, outLenPtr);
    },
    delegate_call: (_t, _f, _fl, _c, _cl, gas, outPtr, outLenPtr) => {
      capture.kind = "delegate";
      capture.gas = gas;
      return writeOut(outPtr, outLenPtr);
    },
    revert: (ptr, len) => {
      const u8 = new Uint8Array(mem.buffer, ptr, len);
      capture.revertReason = new TextDecoder().decode(u8);
      throw new Error("REVERT");
    },
  };
  // Anything else the module pulls in must not be reached by this path.
  for (const n of ["sload", "sstore", "sload_scalar", "sstore_scalar", "return", "abort"]) {
    if (!(n in pyde)) pyde[n] = notCalled(n);
  }

  const { instance } = await WebAssembly.instantiate(bytes, {
    pyde,
    env: { abort: notCalled("env.abort") },
  });
  mem = instance.exports.memory;
  return instance.exports;
}

const readOut = (ex, len) =>
  new Uint8Array(ex.memory.buffer, ex.outPtr(), len).slice();
const decodeOut = (ex, len) => new TextDecoder().decode(readOut(ex, len));

test("a successful call returns exactly the bytes the host wrote", async () => {
  const payload = new Uint8Array([1, 2, 3, 4, 5]);
  const ex = await instantiate({ status: STATUS_OK, payload });
  ex.doCall(0);
  assert.equal(ex.lastStatusCode(), STATUS_OK);
  // Trimmed to the actual length, not the 16 KiB buffer it was read into.
  assert.equal(ex.lastDataLen(), 5);
  assert.deepEqual([...readOut(ex, 5)], [1, 2, 3, 4, 5]);
  assert.equal(ex.lastWasReverted(), 0);
});

test("a revert surfaces the callee's payload, not a generic failure", async () => {
  const payload = new TextEncoder().encode("insufficient balance");
  const ex = await instantiate({ status: ERR_CROSS_CALL_FAILED, payload });
  ex.doCall(0);
  assert.equal(ex.lastStatusCode(), ERR_CROSS_CALL_FAILED);
  assert.equal(ex.lastWasReverted(), 1);
  assert.equal(decodeOut(ex, ex.lastMessageInto()), "insufficient balance");
});

test("a failure that never ran is not reported as a revert", async () => {
  // -13 means the call was rejected before the callee executed, so there
  // is no callee payload to forward. Treating it as a revert would invent
  // a message the callee never raised.
  const ex = await instantiate({ status: ERR_INVALID_FUNCTION_NAME });
  ex.doCall(0);
  assert.equal(ex.lastStatusCode(), ERR_INVALID_FUNCTION_NAME);
  assert.equal(ex.lastWasReverted(), 0);
  assert.equal(ex.lastDataLen(), 0);
});

test("an oversized return reverts rather than truncating", async () => {
  // The callee returned 64 bytes into an 8-byte buffer: the host fills
  // what fits and reports 64. Silently trimming would hand back bytes
  // that borsh-decode to a different value than the callee returned.
  const ex = await instantiate({
    status: STATUS_OK,
    payload: new Uint8Array(8),
    reportLen: 64,
  });
  assert.throws(() => ex.doCallWithCap(8), /REVERT/);
});

test("execOrRevert forwards the callee's message verbatim", async () => {
  const capture = {};
  const payload = new TextEncoder().encode("ERC20: transfer amount exceeds balance");
  const ex = await instantiate({ status: ERR_CROSS_CALL_FAILED, payload, capture });
  assert.throws(() => ex.doCallOrRevert(), /REVERT/);
  assert.equal(capture.revertReason, "ERC20: transfer amount exceeds balance");
});

test("execOrRevert names the status when there is no payload", async () => {
  const capture = {};
  const ex = await instantiate({ status: ERR_INVALID_FUNCTION_NAME, capture });
  assert.throws(() => ex.doCallOrRevert(), /REVERT/);
  // The code must not be lost just because the callee sent nothing.
  assert.match(capture.revertReason, /invalid-function-name/);
});

test("static and delegate route to their own host fns", async () => {
  for (const [drive, kind] of [
    ["doStaticCall", "static"],
    ["doDelegateCall", "delegate"],
    ["doCall", "call"],
  ]) {
    const capture = {};
    const ex = await instantiate({ capture });
    kind === "call" ? ex.doCall(0) : ex[drive]();
    assert.equal(capture.kind, kind);
  }
});

test("the default gas budget forwards everything remaining", async () => {
  const capture = {};
  const ex = await instantiate({ capture });
  ex.doCall(0);
  // 1 << 62 — far above any real budget, so the engine's
  // min(gas, remaining) forwards all of it.
  assert.equal(capture.gas, 1n << 62n);
});

test("statusName never swallows an unrecognised code", async () => {
  const ex = await instantiate();
  assert.equal(decodeOut(ex, ex.statusNameInto(-10)), "cross-call-failed");
  assert.equal(decodeOut(ex, ex.statusNameInto(0)), "ok");
  assert.equal(decodeOut(ex, ex.statusNameInto(-999)), "status(-999)");
});

process.on("exit", () => rmSync(tmp, { recursive: true, force: true }));
