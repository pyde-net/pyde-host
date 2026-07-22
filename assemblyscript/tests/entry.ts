// Test-only compile entry for tests/child.test.mjs — NOT part of the
// published package (package.json `files` whitelists assembly/ only).
//
// Wraps the StaticArray-taking child.ts helpers behind fixed
// linear-memory scratch slots + plain pointer/length exports, so the
// Node test can drive them through raw WebAssembly exports — no AS
// loader, no __new/__pin dance. The slots are module-scope globals
// (GC roots; the AS GC is non-moving), so their pointers are stable
// for the life of the instance.

import {
  childPreimage,
  unorderedPairEncoding,
  childAddress,
  CHILD_PREIMAGE_LEN,
} from "../assembly/child";

const A = new StaticArray<u8>(32); // parent | pair arg a
const B = new StaticArray<u8>(32); // template | pair arg b
const C = new StaticArray<u8>(32); // salt
const OUT = new StaticArray<u8>(CHILD_PREIMAGE_LEN); // fits 107 ≥ 64 ≥ 32

export function aPtr(): usize {
  return changetype<usize>(A);
}
export function bPtr(): usize {
  return changetype<usize>(B);
}
export function cPtr(): usize {
  return changetype<usize>(C);
}
export function outPtr(): usize {
  return changetype<usize>(OUT);
}

// OUT ← childPreimage(A, B, C); returns bytes written (107).
export function runChildPreimage(): i32 {
  const p = childPreimage(A, B, C);
  memory.copy(changetype<usize>(OUT), changetype<usize>(p), p.length);
  return p.length;
}

// OUT ← unorderedPairEncoding(A, B); returns bytes written (64).
export function runUnorderedPairEncoding(): i32 {
  const e = unorderedPairEncoding(A, B);
  memory.copy(changetype<usize>(OUT), changetype<usize>(e), e.length);
  return e.length;
}

// OUT ← childAddress(A, B, C); returns bytes written (32). Routes
// through the imported hash_poseidon2 — the test stubs it with a
// vector-lookup oracle, pinning the pointer/length plumbing.
export function runChildAddress(): i32 {
  const addr = childAddress(A, B, C);
  memory.copy(changetype<usize>(OUT), changetype<usize>(addr), addr.length);
  return addr.length;
}
