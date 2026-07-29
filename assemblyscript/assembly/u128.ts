// 128-bit integers — the value type AssemblyScript lacks natively.
//
// ─── Why we wrap as-bignum instead of hand-rolling ────────────────────
//
// WASM (and AS) top out at 64-bit integers. Pyde balances, transfer
// amounts, and `tx_value` are all `u128` (spec §3.2 → 16 bytes). Rather
// than re-derive carry/borrow/compare on two u64 limbs, we adopt
// `as-bignum`'s battle-tested `u128` / `i128` — they ship the arithmetic
// as OPERATOR OVERLOADS, so contract code writes `a + b`, `a < b`, `a -
// fee` directly on these values. as-bignum is pure computation: it adds
// ZERO host imports, so it never shows up in the `pyde.*`-only import
// audit.
//
// ─── What we OWN: the wire layout ─────────────────────────────────────
//
// as-bignum has its own to/from-bytes helpers, but the issue is explicit
// that the SDK must own the canonical 16-byte layout. We do that here by
// composing our slice-1 `codec` over the two 64-bit limbs:
//
//   u128 → [ lo: u64 LE (8 bytes) ][ hi: u64 LE (8 bytes) ]
//   i128 → [ lo: u64 LE (8 bytes) ][ hi: i64 LE (8 bytes) ]   (two's-comp)
//
// This is byte-identical to Rust's `u128::to_le_bytes` /
// `i128::to_le_bytes`: little-endian means the least-significant 64-bit
// limb comes first. Owning it here means the layout is defined by OUR
// code, provable against the Rust goldens, and independent of any
// as-bignum internal representation change.

import { u128, i128 } from "as-bignum/assembly";
import { readU64LE, writeU64LE, readI64LE, writeI64LE } from "./codec";

// Re-export the value types so contracts get the arithmetic operators
// (+, -, *, /, ==, <, <=, >, >=, ...) without importing as-bignum
// directly. `import { u128 } from "@pyde-net/host"` is the intended path.
export { u128, i128 };

// Byte width of a u128 / i128 on the wire (spec §3.2).
export const U128_LEN: i32 = 16;
export const I128_LEN: i32 = 16;

// ─────────────────────────────────────────────────────────────────────
// u128 — canonical 16-byte little-endian codec
// ─────────────────────────────────────────────────────────────────────

// Write `value` as 16 little-endian bytes at `ptr`. Low limb first.
// @ts-ignore: decorator
@inline export function writeU128LE(ptr: usize, value: u128): void {
  writeU64LE(ptr, value.lo);
  writeU64LE(ptr + 8, value.hi);
}

// Read a 16-byte little-endian u128 from `ptr`.
// @ts-ignore: decorator
@inline export function readU128LE(ptr: usize): u128 {
  return new u128(readU64LE(ptr), readU64LE(ptr + 8));
}

// Encode `value` to a fresh 16-byte StaticArray (little-endian).
export function u128ToBytesLE(value: u128): StaticArray<u8> {
  const out = new StaticArray<u8>(U128_LEN);
  writeU128LE(changetype<usize>(out), value);
  return out;
}

// Decode a u128 from a 16-byte little-endian StaticArray.
export function u128FromBytesLE(bytes: StaticArray<u8>): u128 {
  assert(bytes.length == U128_LEN, "u128FromBytesLE: need exactly 16 bytes");
  return readU128LE(changetype<usize>(bytes));
}

// The maximum u128 (2^128 - 1) — both limbs all-ones. Handy for the
// parity fixtures and for saturating checks.
// @ts-ignore: decorator
@inline export function u128Max(): u128 {
  return new u128(u64.MAX_VALUE, u64.MAX_VALUE);
}

// ─────────────────────────────────────────────────────────────────────
// i128 — canonical 16-byte little-endian two's-complement codec
// ─────────────────────────────────────────────────────────────────────

// Write `value` as 16 little-endian two's-complement bytes at `ptr`.
// The high limb is signed (i64), so a negative i128 lays down the
// sign-extended high bits exactly as Rust's `i128::to_le_bytes` does.
// @ts-ignore: decorator
@inline export function writeI128LE(ptr: usize, value: i128): void {
  writeU64LE(ptr, value.lo);
  writeI64LE(ptr + 8, value.hi);
}

// Read a 16-byte little-endian two's-complement i128 from `ptr`.
// @ts-ignore: decorator
@inline export function readI128LE(ptr: usize): i128 {
  return new i128(readU64LE(ptr), readI64LE(ptr + 8));
}

// Encode `value` to a fresh 16-byte StaticArray (little-endian two's-comp).
export function i128ToBytesLE(value: i128): StaticArray<u8> {
  const out = new StaticArray<u8>(I128_LEN);
  writeI128LE(changetype<usize>(out), value);
  return out;
}

// Decode an i128 from a 16-byte little-endian two's-complement StaticArray.
export function i128FromBytesLE(bytes: StaticArray<u8>): i128 {
  assert(bytes.length == I128_LEN, "i128FromBytesLE: need exactly 16 bytes");
  return readI128LE(changetype<usize>(bytes));
}
