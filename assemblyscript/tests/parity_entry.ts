// Parity entry for tests/codec_parity.spec.mjs — NOT part of the
// published package. Exposes generic, per-type borsh encoders that the
// Node harness drives from vectors/codec_borsh.json (the Rust goldens),
// so the fixture VALUES live only on the Rust side (single source of
// truth) while AS proves it frames them byte-identically.
//
// Two directions are exercised:
//   • encode  — build the value from the golden `input`, encode, compare
//               to the golden `borsh` hex   (proves BorshEncoder)
//   • reencode — decode the golden `borsh` bytes, re-encode, compare back
//                (proves BorshDecoder against real Rust bytes)

import { BorshEncoder, BorshDecoder } from "../assembly/borsh";
import { u128, i128 } from "../assembly/u128";
import { readU64LE } from "../assembly/codec";

// ── scratch slots (module globals → stable GC-rooted pointers) ──────────
const IN = new StaticArray<u8>(8192);   // caller-written input bytes
const TYPE = new StaticArray<u8>(32);   // type tag for reencode dispatch
let LAST: StaticArray<u8> = new StaticArray<u8>(0); // last encoder output

export function inPtr(): usize { return changetype<usize>(IN); }
export function typePtr(): usize { return changetype<usize>(TYPE); }
export function outPtr(): usize { return changetype<usize>(LAST); }
export function outLen(): i32 { return LAST.length; }

// ── encoders: value comes in via scalar params or the IN buffer ─────────

export function encU8(v: u32): void { LAST = new BorshEncoder().u8(<u8>v).toBytes(); }
export function encU16(v: u32): void { LAST = new BorshEncoder().u16(<u16>v).toBytes(); }
export function encU32(v: u32): void { LAST = new BorshEncoder().u32(v).toBytes(); }
export function encU64(v: u64): void { LAST = new BorshEncoder().u64(v).toBytes(); }

export function encI8(v: i32): void { LAST = new BorshEncoder().i8(<i8>v).toBytes(); }
export function encI16(v: i32): void { LAST = new BorshEncoder().i16(<i16>v).toBytes(); }
export function encI32(v: i32): void { LAST = new BorshEncoder().i32(v).toBytes(); }
export function encI64(v: i64): void { LAST = new BorshEncoder().i64(v).toBytes(); }

// u128/i128 arrive as their two 64-bit limbs (JS splits the decimal).
export function encU128(lo: u64, hi: u64): void {
  LAST = new BorshEncoder().u128(new u128(lo, hi)).toBytes();
}
export function encI128(lo: u64, hi: i64): void {
  LAST = new BorshEncoder().i128(new i128(lo, hi)).toBytes();
}

export function encBool(v: u32): void { LAST = new BorshEncoder().bool(v != 0).toBytes(); }

// address / hash32: 32 raw bytes sitting at IN[0..32].
export function encAddress(): void {
  const a = new StaticArray<u8>(32);
  memory.copy(changetype<usize>(a), inPtr(), 32);
  LAST = new BorshEncoder().address(a).toBytes();
}
export function encHash32(): void {
  const h = new StaticArray<u8>(32);
  memory.copy(changetype<usize>(h), inPtr(), 32);
  LAST = new BorshEncoder().hash32(h).toBytes();
}

// bytes: `len` raw bytes at IN[0..len].
export function encBytes(len: i32): void {
  const b = new StaticArray<u8>(len);
  if (len > 0) memory.copy(changetype<usize>(b), inPtr(), len);
  LAST = new BorshEncoder().bytes(b).toBytes();
}

// string: `len` UTF-8 bytes at IN[0..len] → AS string → encode.
export function encString(len: i32): void {
  const s = String.UTF8.decodeUnsafe(inPtr(), len);
  LAST = new BorshEncoder().string(s).toBytes();
}

// vec<u64>: `count` little-endian u64 elements packed at IN.
export function encVecU64(count: i32): void {
  const v = new Array<u64>(count);
  for (let i = 0; i < count; i++) unchecked(v[i] = readU64LE(inPtr() + <usize>(i * 8)));
  LAST = new BorshEncoder().vecU64(v).toBytes();
}

// ── reencode: decode `dataLen` golden bytes from IN, re-encode ──────────
//
// Dispatched on the type tag written to TYPE[0..typeLen]. Returns the
// decoder's leftover byte count — a clean decode of a whole value leaves 0.
export function reencodeByType(typeLen: i32, dataLen: i32): i32 {
  const ty = String.UTF8.decodeUnsafe(typePtr(), typeLen);
  const data = new StaticArray<u8>(dataLen);
  if (dataLen > 0) memory.copy(changetype<usize>(data), inPtr(), dataLen);
  const dec = new BorshDecoder(data);
  const enc = new BorshEncoder();

  if (ty == "u8") enc.u8(dec.u8());
  else if (ty == "u16") enc.u16(dec.u16());
  else if (ty == "u32") enc.u32(dec.u32());
  else if (ty == "u64") enc.u64(dec.u64());
  else if (ty == "u128") enc.u128(dec.u128());
  else if (ty == "i8") enc.i8(dec.i8());
  else if (ty == "i16") enc.i16(dec.i16());
  else if (ty == "i32") enc.i32(dec.i32());
  else if (ty == "i64") enc.i64(dec.i64());
  else if (ty == "i128") enc.i128(dec.i128());
  else if (ty == "bool") enc.bool(dec.bool());
  else if (ty == "address") enc.address(dec.address());
  else if (ty == "hash32") enc.hash32(dec.hash32());
  else if (ty == "bytes") enc.bytes(dec.bytes());
  else if (ty == "string") enc.string(dec.string());
  else if (ty == "vec_u64") enc.vecU64(dec.vecU64());
  else { assert(false, "reencode: unknown type tag"); }

  LAST = enc.toBytes();
  return dec.remaining;
}
