// Hashing primitives — pointer-free wrappers over the §7.6 host fns.
// Mirrors the Rust reference `pyde::hash::{blake3, poseidon2, keccak256}`
// (rust/pyde-host/src/lib.rs L818): allocate the 32-byte output, call the
// host fn with (in_ptr, in_len, out_ptr), return the digest.

import { hash_blake3, hash_poseidon2, hash_keccak256 } from "@pyde-net/host/assembly";
import { Bytes32, newBytes32 } from "./types";

// Blake3 — general-purpose hashing (address derivation, event topic-0,
// content addressing).
export function blake3(input: StaticArray<u8>): Bytes32 {
  const out = newBytes32();
  hash_blake3(changetype<usize>(input), input.length, changetype<usize>(out));
  return out;
}

// Poseidon2 — Pyde's canonical storage-slot derivation
// (`slot = Poseidon2(self_address || field || key)`) and ZK-friendly
// hashing.
export function poseidon2(input: StaticArray<u8>): Bytes32 {
  const out = newBytes32();
  hash_poseidon2(changetype<usize>(input), input.length, changetype<usize>(out));
  return out;
}

// Keccak256 — EVM-interop hashing (Merkle-Patricia proofs, etc.).
export function keccak256(input: StaticArray<u8>): Bytes32 {
  const out = newBytes32();
  hash_keccak256(changetype<usize>(input), input.length, changetype<usize>(out));
  return out;
}
