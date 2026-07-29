// Fixed 32-byte value types — Address and Bytes32.
//
// Both are exactly 32 raw bytes with NO length prefix (borsh treats a
// `[u8; 32]` as a bare fixed array — HOST_FN_ABI_SPEC §3.2 fixed sizes).
// They mirror the Rust reference, where `Address` and `Hash32` are both
// `pub type ... = [u8; 32]` (rust/pyde-host/src/lib.rs L107/L113): two
// names for the same shape, so an address and a hash are byte-assignable.
//
// We back them with `StaticArray<u8>` (not a class) for the same reasons
// the host package's child.ts does: a StaticArray is a flat, GC-managed
// byte buffer whose backing pointer is `changetype<usize>(arr)` — exactly
// what every host fn wants. No boxing, no wrapper object, no length word
// to strip before handing the pointer across the boundary.

// Byte width of an Address / Bytes32. Both are 32 (spec §3.2).
export const ADDRESS_LEN: i32 = 32;
export const BYTES32_LEN: i32 = 32;

// A 32-byte Pyde account address. Same layout as the chain's address
// type; kept as a thin StaticArray alias so contracts never pull heavier
// machinery in.
export type Address = StaticArray<u8>;

// A 32-byte hash / opaque identifier (Blake3, Poseidon2, Keccak256
// output; content hashes; pubkey-derived ids). `bytes32` in otigen.toml
// resolves here. Byte-compatible with Address by design.
export type Bytes32 = StaticArray<u8>;

// Allocate a zeroed 32-byte Address. AS zero-initializes StaticArray, so
// this is the canonical "empty address" (all-zero) too.
// @ts-ignore: decorator
@inline export function newAddress(): Address {
  return new StaticArray<u8>(ADDRESS_LEN);
}

// Allocate a zeroed 32-byte Bytes32.
// @ts-ignore: decorator
@inline export function newBytes32(): Bytes32 {
  return new StaticArray<u8>(BYTES32_LEN);
}

// Copy exactly 32 bytes from an arbitrary source buffer into a fresh
// Address. `src` must be at least 32 bytes; callers that read from
// untrusted lengths should check first (borsh decode does).
// @ts-ignore: decorator
@inline export function bytes32FromPtr(src: usize): Bytes32 {
  const out = new StaticArray<u8>(BYTES32_LEN);
  memory.copy(changetype<usize>(out), src, BYTES32_LEN);
  return out;
}

// Constant-time-ish byte equality for two 32-byte values. Not a
// side-channel-hardened compare (contracts rarely need that on public
// chain data); it exists so authorization checks read as `equals32(a, b)`
// instead of a hand-rolled loop at every call site.
export function equals32(a: StaticArray<u8>, b: StaticArray<u8>): bool {
  if (a.length != BYTES32_LEN || b.length != BYTES32_LEN) return false;
  for (let i = 0; i < BYTES32_LEN; i++) {
    if (unchecked(a[i]) != unchecked(b[i])) return false;
  }
  return true;
}
