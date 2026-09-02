// Chainless behaviour test for assembly/factory.ts and assembly/account.ts.
//
// Compiles tests/factory_entry.ts with the local asc and instantiates it in
// Node with `instantiate`, `transfer`, and `balance` stubbed. Everything the
// engine does beyond the host-fn boundary is out of scope here and belongs
// to an on-devnet example.

import assert from "node:assert/strict";
import { test } from "node:test";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const asDir = fileURLToPath(new URL("..", import.meta.url));
const tmp = mkdtempSync(join(tmpdir(), "pyde-host-factory-"));
const wasmPath = join(tmp, "factory-test.wasm");

execFileSync(
  "npx",
  ["asc", "tests/factory_entry.ts", "--outFile", wasmPath, "--target", "release"],
  { cwd: asDir, stdio: "inherit" },
);
const bytes = await readFile(wasmPath);

const STATUS_OK = 0;
const ERR_CTOR_REVERTED = -40;
const ERR_CHILD_ADDRESS_TAKEN = -44;

async function instantiate({
  status = STATUS_OK,
  payload = new Uint8Array(0),
  childFill = 0xcd,
  balanceLo = 0n,
  transferStatus = STATUS_OK,
  capture = {},
} = {}) {
  let mem;
  const grab = (p, n) => new Uint8Array(mem.buffer, p, n).slice();

  const pyde = {
    instantiate: (tPtr, sPtr, cdPtr, cdLen, valPtr, gas, childPtr, outPtr, outLenPtr) => {
      const u8 = new Uint8Array(mem.buffer);
      capture.template = grab(tPtr, 32);
      capture.salt = grab(sPtr, 32);
      capture.calldata = grab(cdPtr, cdLen);
      capture.value = grab(valPtr, 16);
      capture.gas = gas;
      // The host writes the child address on every path past the early
      // bounds checks — including failures.
      u8.fill(childFill, childPtr, childPtr + 32);
      u8.set(payload, outPtr);
      new DataView(mem.buffer).setInt32(outLenPtr, payload.length, true);
      return status;
    },
    balance: (_addrPtr, outPtr) => {
      const dv = new DataView(mem.buffer);
      dv.setBigUint64(outPtr, balanceLo, true);
      dv.setBigUint64(outPtr + 8, 0n, true);
    },
    transfer: (toPtr, amtPtr) => {
      capture.to = grab(toPtr, 32);
      capture.amount = grab(amtPtr, 16);
      return transferStatus;
    },
    revert: (ptr, len) => {
      capture.revertReason = new TextDecoder().decode(new Uint8Array(mem.buffer, ptr, len));
      throw new Error("REVERT");
    },
  };
  const notCalled = (n) => () => { throw new Error(`unexpected host fn: ${n}`); };
  for (const n of ["sload", "sstore", "return", "cross_call"]) {
    if (!(n in pyde)) pyde[n] = notCalled(n);
  }

  const { instance } = await WebAssembly.instantiate(bytes, {
    pyde,
    env: { abort: notCalled("env.abort") },
  });
  mem = instance.exports.memory;
  return instance.exports;
}

const readOut = (ex, n) => new Uint8Array(ex.memory.buffer, ex.outPtr(), n).slice();

test("a successful instantiate returns the child the host wrote", async () => {
  const ex = await instantiate({ childFill: 0xab });
  ex.doInstantiate();
  assert.equal(ex.lastStatusCode(), STATUS_OK);
  assert.deepEqual([...readOut(ex, ex.lastChildInto())], new Array(32).fill(0xab));
});

test("constructor args reach the host verbatim", async () => {
  const capture = {};
  const ex = await instantiate({ capture });
  new Uint8Array(ex.memory.buffer, ex.saltPtr(), 32).fill(0x11);
  ex.doInstantiateWithArgs(12);
  assert.equal(capture.calldata.length, 12);
  assert.deepEqual([...capture.salt], new Array(32).fill(0x11));
});

test("an endowment is passed as 16 little-endian bytes", async () => {
  const capture = {};
  const ex = await instantiate({ capture });
  ex.doInstantiateWithValue(258n); // 0x0102
  assert.deepEqual([...capture.value], [0x02, 0x01, ...new Array(14).fill(0)]);
});

test("the default gas budget is the forward-all sentinel", async () => {
  const capture = {};
  const ex = await instantiate({ capture });
  ex.doInstantiate();
  // -1, NOT a large positive number. instantiate reads a negative limit as
  // "forward everything"; a big positive value would CAP the constructor.
  assert.equal(capture.gas, -1n);
});

test("a constructor revert is distinguished from a refusal", async () => {
  const payload = new TextEncoder().encode("token: bad decimals");
  const ex = await instantiate({ status: ERR_CTOR_REVERTED, payload });
  ex.doInstantiate();
  assert.equal(ex.lastWasCtorRevert(), 1);
  assert.equal(new TextDecoder().decode(readOut(ex, ex.lastMessageInto())), "token: bad decimals");

  // -44 means it never got as far as running a constructor, so there is no
  // child payload and reporting one would invent a message.
  const ex2 = await instantiate({ status: ERR_CHILD_ADDRESS_TAKEN });
  ex2.doInstantiate();
  assert.equal(ex2.lastWasCtorRevert(), 0);
});

test("instantiateOrRevert forwards the constructor's message verbatim", async () => {
  const capture = {};
  const payload = new TextEncoder().encode("token: initial supply is zero");
  const ex = await instantiate({ status: ERR_CTOR_REVERTED, payload, capture });
  assert.throws(() => ex.doInstantiateOrRevert(), /REVERT/);
  assert.equal(capture.revertReason, "token: initial supply is zero");
});

test("instantiateOrRevert names the status when there is no payload", async () => {
  const capture = {};
  const ex = await instantiate({ status: ERR_CHILD_ADDRESS_TAKEN, capture });
  assert.throws(() => ex.doInstantiateOrRevert(), /REVERT/);
  assert.match(capture.revertReason, /child-address-taken/);
});

// ── account ──────────────────────────────────────────────────────────

test("balanceOf decodes the 16 little-endian bytes the host writes", async () => {
  const ex = await instantiate({ balanceLo: 1_000_000_000n });
  assert.equal(ex.doBalanceLo(), 1_000_000_000n);
});

test("transfer reverts on a non-OK status; tryTransfer reports it", async () => {
  const capture = {};
  const ok = await instantiate({ capture });
  ok.doTransfer(500n);
  assert.deepEqual([...capture.amount.slice(0, 2)], [0xf4, 0x01]); // 500 LE

  const bad = await instantiate({ transferStatus: -3, capture });
  assert.throws(() => bad.doTransfer(500n), /REVERT/);
  assert.match(capture.revertReason, /insufficient balance/);

  // Same failure, but the caller wants to handle it rather than abort.
  const soft = await instantiate({ transferStatus: -3 });
  assert.equal(soft.doTryTransfer(500n), 0);
});

process.on("exit", () => rmSync(tmp, { recursive: true, force: true }));
