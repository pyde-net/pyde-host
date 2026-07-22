// Factory child-address derivation — HOST_FN_ABI_SPEC §7.12.
//
//   child_address = Poseidon2("pyde-child:" ‖ parent[32] ‖ template[32] ‖ salt[32])
//
// The preimage is 107 bytes fixed-width — no length prefixes, no
// separators — required for hash injectivity. Golden conformance
// vectors live at vectors/child_address.json (repo root); every
// implementation must reproduce every vector byte-for-byte.
//
// Inputs are `StaticArray<u8>` and must be exactly 32 bytes; length is
// enforced with `assert` (traps via abort; stripped under `--noAssert`,
// where a short input would read stale bytes — keep asserts on).

import { hash_poseidon2 } from "./host_fns";

// Domain-separator prefix for child (factory-instantiated) addresses:
// "pyde-child:" — exactly 11 ASCII bytes (70 79 64 65 2d 63 68 69 6c
// 64 3a). Keeps child addresses structurally disjoint from every other
// address family (EOA, contract-name, system).
export const CHILD_ADDRESS_PREFIX: StaticArray<u8> = [
  0x70, 0x79, 0x64, 0x65, 0x2d, 0x63, 0x68, 0x69, 0x6c, 0x64, 0x3a,
];

// Byte length of the child-address preimage: 11 + 32 + 32 + 32.
export const CHILD_PREIMAGE_LEN: i32 = 107;

// Assemble the canonical 107-byte child-address preimage:
//
//   "pyde-child:" ‖ parent (32 B) ‖ template (32 B) ‖ salt (32 B)
//
// Pure byte assembly — no hashing, no host calls — so conformance
// tests can pin the layout without a chain. Contracts normally want
// `childAddress` instead.
export function childPreimage(
  parent: StaticArray<u8>,
  template: StaticArray<u8>,
  salt: StaticArray<u8>,
): StaticArray<u8> {
  assert(parent.length == 32, "childPreimage: parent must be 32 bytes");
  assert(template.length == 32, "childPreimage: template must be 32 bytes");
  assert(salt.length == 32, "childPreimage: salt must be 32 bytes");
  const p = new StaticArray<u8>(CHILD_PREIMAGE_LEN);
  const base = changetype<usize>(p);
  memory.copy(base, changetype<usize>(CHILD_ADDRESS_PREFIX), 11);
  memory.copy(base + 11, changetype<usize>(parent), 32);
  memory.copy(base + 43, changetype<usize>(template), 32);
  memory.copy(base + 75, changetype<usize>(salt), 32);
  return p;
}

// Canonical encoding for an UNORDERED pair of 32-byte values (the
// identity-salt recipe for symmetric pairs — one pool per token pair,
// one channel per (a, b), ...): sort the two values ascending BYTEWISE
// (lexicographic), then concatenate — raw 64 bytes, no framing.
// Argument order cannot influence the result. The salt is
// `hash_poseidon2` of these bytes.
export function unorderedPairEncoding(
  a: StaticArray<u8>,
  b: StaticArray<u8>,
): StaticArray<u8> {
  assert(a.length == 32, "unorderedPairEncoding: a must be 32 bytes");
  assert(b.length == 32, "unorderedPairEncoding: b must be 32 bytes");
  let swap = false;
  for (let i = 0; i < 32; i++) {
    const ai = a[i];
    const bi = b[i];
    if (ai != bi) {
      swap = ai > bi;
      break;
    }
  }
  const lo = swap ? b : a;
  const hi = swap ? a : b;
  const out = new StaticArray<u8>(64);
  memory.copy(changetype<usize>(out), changetype<usize>(lo), 32);
  memory.copy(changetype<usize>(out) + 32, changetype<usize>(hi), 32);
  return out;
}

// Address of the child a factory would (or did) create for
// `(template, salt)` — Poseidon2 of `childPreimage`.
//
// - `parent` — the FACTORY's own address. On-chain, pass the
//   `self_address` result; the engine derives from the executing
//   frame, so a contract can only ever mint into its OWN namespace.
// - `template` — the deployed template contract's address (an
//   address, NOT a code hash).
// - `salt` — opaque 32 bytes, caller-derived; deterministically
//   unique only ("random" salts forfeit counterfactual addressing).
//
// Same inputs → same address, forever. Hashes via the
// `hash_poseidon2` host fn — the same Poseidon2 the engine's
// authoritative derivation uses, so the two cannot disagree. On-chain
// only (host import); DCE strips it from the .wasm when unused.
export function childAddress(
  parent: StaticArray<u8>,
  template: StaticArray<u8>,
  salt: StaticArray<u8>,
): StaticArray<u8> {
  const preimage = childPreimage(parent, template, salt);
  const out = new StaticArray<u8>(32);
  hash_poseidon2(changetype<usize>(preimage), CHILD_PREIMAGE_LEN, changetype<usize>(out));
  return out;
}
