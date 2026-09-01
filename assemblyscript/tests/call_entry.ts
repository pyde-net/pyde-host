// Test-only compile entry for tests/call.test.mjs — NOT part of the
// published package (package.json `files` whitelists assembly/ only).
//
// Drives the cross-call builder through plain pointer/length exports so
// the Node test can stub `cross_call` / `cross_call_static` /
// `delegate_call` and assert what the wrapper does with each status code
// and payload. No chain, no engine — the host fn IS the fixture.

import { Call, StaticCall, DelegateCall, CallResult } from "../assembly/call";
import { statusName } from "../assembly/status";
import { u128 } from "../assembly/u128";

const TARGET = new StaticArray<u8>(32);
const ARGS = new StaticArray<u8>(64);
// Holds the bytes the last call returned, so the test can read them back.
const OUT = new StaticArray<u8>(1024);

let lastStatus: i32 = 0;
let lastLen: i32 = 0;
let lastReverted: bool = false;
let lastMessage: string = "";

export function targetPtr(): usize {
  return changetype<usize>(TARGET);
}
export function argsPtr(): usize {
  return changetype<usize>(ARGS);
}
export function outPtr(): usize {
  return changetype<usize>(OUT);
}

/// Copy a result into the readable slots so the test can inspect it
/// without an AS loader.
function record(r: CallResult): void {
  lastStatus = r.status;
  lastLen = r.data.length;
  lastReverted = r.reverted;
  lastMessage = r.revertMessage;
  const n = r.data.length < OUT.length ? r.data.length : OUT.length;
  if (n > 0) {
    memory.copy(changetype<usize>(OUT), changetype<usize>(r.data), <usize>n);
  }
}

/// `Call(...).args(...).exec()` with `argsLen` bytes of calldata.
export function doCall(argsLen: i32): void {
  const cd = new StaticArray<u8>(argsLen);
  if (argsLen > 0) {
    memory.copy(changetype<usize>(cd), changetype<usize>(ARGS), <usize>argsLen);
  }
  record(Call(TARGET, "transfer").args(cd).exec());
}

/// Same, but with a return cap, to exercise the oversized-return guard.
export function doCallWithCap(cap: i32): void {
  record(Call(TARGET, "transfer").returnCap(cap).exec());
}

/// Attach value — routes through `cross_call` with a value pointer.
export function doCallWithValue(lo: u64): void {
  record(Call(TARGET, "transfer").value(u128.fromU64(lo)).exec());
}

export function doStaticCall(): void {
  record(StaticCall(TARGET, "balance_of").exec());
}

export function doDelegateCall(): void {
  record(DelegateCall(TARGET, "upgrade").exec());
}

/// `execOrRevert` — traps through the stubbed `revert` on any non-OK
/// status, so the test asserts on the revert reason the host receives.
export function doCallOrRevert(): i32 {
  return Call(TARGET, "transfer").execOrRevert().length;
}

export function lastStatusCode(): i32 {
  return lastStatus;
}
export function lastDataLen(): i32 {
  return lastLen;
}
export function lastWasReverted(): bool {
  return lastReverted;
}
/// Write the last revert message into OUT and return its byte length, so
/// the test can compare the decoded text.
export function lastMessageInto(): i32 {
  const b = String.UTF8.encode(lastMessage);
  const n = b.byteLength < OUT.length ? b.byteLength : OUT.length;
  if (n > 0) {
    memory.copy(changetype<usize>(OUT), changetype<usize>(b), <usize>n);
  }
  return n;
}

/// Write `statusName(code)` into OUT; returns its byte length.
export function statusNameInto(code: i32): i32 {
  const b = String.UTF8.encode(statusName(code));
  const n = b.byteLength < OUT.length ? b.byteLength : OUT.length;
  if (n > 0) {
    memory.copy(changetype<usize>(OUT), changetype<usize>(b), <usize>n);
  }
  return n;
}
