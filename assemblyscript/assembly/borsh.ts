// Borsh v1 encode / decode for AssemblyScript — byte-identical to the
// Rust reference's `borsh::to_vec` (rust/pyde-host + the `borsh` crate).
//
// ─── The rules (borsh v1, the subset Pyde's ScalarType vocab uses) ────
//
//   uN / iN          fixed-width little-endian (u8..u128, i8..i128)
//   bool             1 byte, 0x00 = false, 0x01 = true
//   address / hash32 32 RAW bytes, NO length prefix (a fixed `[u8; 32]`)
//   bytes            u32_le(len) ‖ raw bytes
//   string           u32_le(byte_len) ‖ UTF-8 bytes  (no NUL)
//   vec<T>           u32_le(len) ‖ element[0] ‖ element[1] ‖ ...
//   tuple / struct   field[0] ‖ field[1] ‖ ...   (NO framing)
//
// The last rule is why there is no `tuple()` method: encoding a struct is
// just calling one field method per field, in declaration order. That
// frameless concatenation is exactly `#[pyde::entry]`'s calldata layout,
// so an encoder built field-by-field produces calldata a Rust contract
// decodes verbatim.
//
// This layer OWNS nothing about byte order itself — it composes the
// slice-1 `codec` (LE ints) and slice-2 `u128` codec. Fix a byte there,
// fix it everywhere.

import {
  writeU8LE, writeU16LE, writeU32LE, writeU64LE,
  writeI8LE, writeI16LE, writeI32LE, writeI64LE,
  readU8LE, readU16LE, readU32LE, readU64LE,
  readI8LE, readI16LE, readI32LE, readI64LE,
} from "./codec";
import {
  u128, i128,
  writeU128LE, writeI128LE, readU128LE, readI128LE,
} from "./u128";
import { Address, Bytes32, bytes32FromPtr, BYTES32_LEN } from "./types";

// ─────────────────────────────────────────────────────────────────────
// Encoder
// ─────────────────────────────────────────────────────────────────────
//
// Backed by an ArrayBuffer we grow ourselves (doubling). `reserve(n)`
// grows if needed, then returns a pointer to the freshly-claimed tail —
// valid until the NEXT reserve. Every writer calls reserve immediately
// before writing, so the pointer is never stale. `this.data` is an
// ArrayBuffer reference (a GC root), so the backing bytes stay alive.

export class BorshEncoder {
  private data: ArrayBuffer;
  private len: i32;

  constructor(initialCapacity: i32 = 64) {
    this.data = new ArrayBuffer(initialCapacity > 0 ? initialCapacity : 1);
    this.len = 0;
  }

  // Grow to fit `n` more bytes, return a pointer to the tail, advance len.
  private reserve(n: i32): usize {
    const need = this.len + n;
    const cap = this.data.byteLength;
    if (need > cap) {
      let newCap = cap;
      while (newCap < need) newCap <<= 1;
      const grown = new ArrayBuffer(newCap);
      memory.copy(changetype<usize>(grown), changetype<usize>(this.data), this.len);
      this.data = grown;
    }
    const p = changetype<usize>(this.data) + <usize>this.len;
    this.len += n;
    return p;
  }

  // ── fixed-width integers ────────────────────────────────────────────
  u8(v: u8): BorshEncoder { writeU8LE(this.reserve(1), v); return this; }
  u16(v: u16): BorshEncoder { writeU16LE(this.reserve(2), v); return this; }
  u32(v: u32): BorshEncoder { writeU32LE(this.reserve(4), v); return this; }
  u64(v: u64): BorshEncoder { writeU64LE(this.reserve(8), v); return this; }
  u128(v: u128): BorshEncoder { writeU128LE(this.reserve(16), v); return this; }

  i8(v: i8): BorshEncoder { writeI8LE(this.reserve(1), v); return this; }
  i16(v: i16): BorshEncoder { writeI16LE(this.reserve(2), v); return this; }
  i32(v: i32): BorshEncoder { writeI32LE(this.reserve(4), v); return this; }
  i64(v: i64): BorshEncoder { writeI64LE(this.reserve(8), v); return this; }
  i128(v: i128): BorshEncoder { writeI128LE(this.reserve(16), v); return this; }

  // ── bool ────────────────────────────────────────────────────────────
  bool(v: bool): BorshEncoder { writeU8LE(this.reserve(1), v ? 1 : 0); return this; }

  // ── fixed 32-byte values: address / hash32 (raw, no prefix) ─────────
  private fixed32(b: StaticArray<u8>): BorshEncoder {
    assert(b.length == BYTES32_LEN, "borsh: address/hash32 must be 32 bytes");
    memory.copy(this.reserve(BYTES32_LEN), changetype<usize>(b), BYTES32_LEN);
    return this;
  }
  address(a: Address): BorshEncoder { return this.fixed32(a); }
  hash32(h: Bytes32): BorshEncoder { return this.fixed32(h); }

  // ── length-prefixed byte string: u32_le(len) ‖ raw bytes ────────────
  bytes(b: StaticArray<u8>): BorshEncoder {
    this.u32(<u32>b.length);
    if (b.length > 0) memory.copy(this.reserve(b.length), changetype<usize>(b), b.length);
    return this;
  }

  // ── UTF-8 string: u32_le(byte_len) ‖ UTF-8 bytes (no NUL) ────────────
  string(s: string): BorshEncoder {
    const utf8 = String.UTF8.encode(s); // ArrayBuffer, not NUL-terminated
    const n = utf8.byteLength;
    this.u32(<u32>n);
    if (n > 0) memory.copy(this.reserve(n), changetype<usize>(utf8), n);
    return this;
  }

  // ── vec<u64>: u32_le(len) ‖ each element as 8-byte LE ────────────────
  // The representative vec for this slice; generic `vec<T>` arrives with
  // codegen (Phases 2-4). A borsh `Vec<u8>` is identical to `bytes()`.
  vecU64(v: u64[]): BorshEncoder {
    this.u32(<u32>v.length);
    for (let i = 0, n = v.length; i < n; i++) this.u64(unchecked(v[i]));
    return this;
  }

  // Number of bytes encoded so far.
  // @ts-ignore: decorator
  @inline get length(): i32 { return this.len; }

  // Copy the accumulated bytes into a fresh, exactly-sized StaticArray.
  toBytes(): StaticArray<u8> {
    const out = new StaticArray<u8>(this.len);
    if (this.len > 0) memory.copy(changetype<usize>(out), changetype<usize>(this.data), this.len);
    return out;
  }
}

// ─────────────────────────────────────────────────────────────────────
// Decoder
// ─────────────────────────────────────────────────────────────────────
//
// A forward cursor over a StaticArray<u8>. Stores the ARRAY REFERENCE
// (not a raw pointer): a bare `usize` is invisible to the GC, so caching
// only the pointer could let the buffer be collected mid-decode. Every
// read bounds-checks against the input length and traps on underrun.

export class BorshDecoder {
  private data: StaticArray<u8>;
  private pos: i32;

  constructor(data: StaticArray<u8>) {
    this.data = data;
    this.pos = 0;
  }

  // Claim `n` bytes, return a pointer to them, advance the cursor.
  private take(n: i32): usize {
    assert(this.pos + n <= this.data.length, "borsh: unexpected end of input");
    const p = changetype<usize>(this.data) + <usize>this.pos;
    this.pos += n;
    return p;
  }

  // ── fixed-width integers ────────────────────────────────────────────
  u8(): u8 { return readU8LE(this.take(1)); }
  u16(): u16 { return readU16LE(this.take(2)); }
  u32(): u32 { return readU32LE(this.take(4)); }
  u64(): u64 { return readU64LE(this.take(8)); }
  u128(): u128 { return readU128LE(this.take(16)); }

  i8(): i8 { return readI8LE(this.take(1)); }
  i16(): i16 { return readI16LE(this.take(2)); }
  i32(): i32 { return readI32LE(this.take(4)); }
  i64(): i64 { return readI64LE(this.take(8)); }
  i128(): i128 { return readI128LE(this.take(16)); }

  // ── bool ────────────────────────────────────────────────────────────
  bool(): bool { return readU8LE(this.take(1)) != 0; }

  // ── fixed 32-byte values ────────────────────────────────────────────
  address(): Address { return bytes32FromPtr(this.take(BYTES32_LEN)); }
  hash32(): Bytes32 { return bytes32FromPtr(this.take(BYTES32_LEN)); }

  // ── length-prefixed byte string ─────────────────────────────────────
  bytes(): StaticArray<u8> {
    const n = <i32>this.u32();
    const out = new StaticArray<u8>(n);
    if (n > 0) memory.copy(changetype<usize>(out), this.take(n), n);
    return out;
  }

  // ── UTF-8 string ────────────────────────────────────────────────────
  string(): string {
    const n = <i32>this.u32();
    const p = this.take(n);
    return String.UTF8.decodeUnsafe(p, n);
  }

  // ── vec<u64> ────────────────────────────────────────────────────────
  vecU64(): u64[] {
    const n = <i32>this.u32();
    const out = new Array<u64>(n);
    for (let i = 0; i < n; i++) unchecked(out[i] = this.u64());
    return out;
  }

  // Bytes not yet consumed. A well-formed decode of a complete value
  // ends with `remaining() == 0`.
  // @ts-ignore: decorator
  @inline get remaining(): i32 { return this.data.length - this.pos; }
}
