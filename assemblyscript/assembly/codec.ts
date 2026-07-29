// Little-endian integer codecs — the byte-level substrate every higher
// layer (u128, borsh, ctx) builds on.
//
// ─── Why these exist ──────────────────────────────────────────────────
//
// The Pyde host ABI moves every multi-byte integer across the WASM ⇄ host
// boundary in LITTLE-ENDIAN order (HOST_FN_ABI_SPEC §3.2), which is also
// WASM linear memory's native byte order. The Rust reference produces the
// same bytes with `u32::to_le_bytes` / `u32::from_le_bytes`, etc.
//
// AssemblyScript's `store<T>(ptr, value)` / `load<T>(ptr)` intrinsics
// already write/read in that exact little-endian layout — WASM has no
// big-endian store. So each function here is a one-liner. We still name
// them explicitly (one per width + signedness) because the borsh layer
// reads far more clearly as `writeU32LE(p, len)` than a bare `store<u32>`,
// and because naming the byte layout is the whole point of "own the
// wire format" — a future non-WASM target can re-implement these without
// touching a single call site.
//
// ─── Pointer convention ───────────────────────────────────────────────
//
// Every `ptr` is a `usize` byte-offset into linear memory (wasm32 →
// 32-bit). Callers own the buffer and guarantee `[ptr, ptr + width)` is
// in bounds; a stray pointer traps in the host, it does not corrupt it.
// Get a pointer from any AS buffer with `changetype<usize>(obj)` (works
// on StaticArray / ArrayBuffer / typed-array dataStart).
//
// WASM permits unaligned loads/stores, so these are safe at any offset —
// borsh packs fields back-to-back with no padding.

// ─────────────────────────────────────────────────────────────────────
// Unsigned
// ─────────────────────────────────────────────────────────────────────

// @ts-ignore: decorator
@inline export function writeU8LE(ptr: usize, value: u8): void { store<u8>(ptr, value); }
// @ts-ignore: decorator
@inline export function readU8LE(ptr: usize): u8 { return load<u8>(ptr); }

// @ts-ignore: decorator
@inline export function writeU16LE(ptr: usize, value: u16): void { store<u16>(ptr, value); }
// @ts-ignore: decorator
@inline export function readU16LE(ptr: usize): u16 { return load<u16>(ptr); }

// @ts-ignore: decorator
@inline export function writeU32LE(ptr: usize, value: u32): void { store<u32>(ptr, value); }
// @ts-ignore: decorator
@inline export function readU32LE(ptr: usize): u32 { return load<u32>(ptr); }

// @ts-ignore: decorator
@inline export function writeU64LE(ptr: usize, value: u64): void { store<u64>(ptr, value); }
// @ts-ignore: decorator
@inline export function readU64LE(ptr: usize): u64 { return load<u64>(ptr); }

// ─────────────────────────────────────────────────────────────────────
// Signed — two's-complement, same bytes as the unsigned width. Borsh
// encodes signed integers as plain little-endian two's-complement, so
// `writeI32LE(p, -1)` lays down `ff ff ff ff`, identical to Rust's
// `(-1i32).to_le_bytes()`.
// ─────────────────────────────────────────────────────────────────────

// @ts-ignore: decorator
@inline export function writeI8LE(ptr: usize, value: i8): void { store<i8>(ptr, value); }
// @ts-ignore: decorator
@inline export function readI8LE(ptr: usize): i8 { return load<i8>(ptr); }

// @ts-ignore: decorator
@inline export function writeI16LE(ptr: usize, value: i16): void { store<i16>(ptr, value); }
// @ts-ignore: decorator
@inline export function readI16LE(ptr: usize): i16 { return load<i16>(ptr); }

// @ts-ignore: decorator
@inline export function writeI32LE(ptr: usize, value: i32): void { store<i32>(ptr, value); }
// @ts-ignore: decorator
@inline export function readI32LE(ptr: usize): i32 { return load<i32>(ptr); }

// @ts-ignore: decorator
@inline export function writeI64LE(ptr: usize, value: i64): void { store<i64>(ptr, value); }
// @ts-ignore: decorator
@inline export function readI64LE(ptr: usize): i64 { return load<i64>(ptr); }
