// Child-address preimage helpers for the Pyde factory pattern
// (PIP-0006). This header owns the two DRIFT-CRITICAL byte
// constructions every factory contract repeats:
//
//   child_address = Poseidon2("pyde-child:" || parent[32]
//                             || template[32] || salt[32])
//
// — a 107-byte fixed-width preimage, no length prefixes, no
// separators — and the unordered-pair salt-source encoding (sort the
// two 32-byte values ascending bytewise, concatenate to 64 raw
// bytes). Poseidon2 itself always runs ON-CHAIN via the
// hash_poseidon2 host fn (HOST_FN_ABI_SPEC §7.6); these helpers only
// assemble its input. Conformance for every implementation is the
// shared golden vectors: vectors/child_address.json.

#ifndef PYDE_CHILD_H
#define PYDE_CHILD_H

#include <stdint.h>

// wasm32-unknown-unknown contract builds are freestanding (-nostdlib,
// no libc sysroot): <string.h> does not exist there. The compiler
// builtins lower these fixed 32/64-byte copies/compares inline, so no
// libc symbol is emitted either. Hosted builds (tests, tooling) use
// <string.h> proper. pyde_child_address additionally imports the
// hash_poseidon2 host fn, which only exists inside the WASM sandbox —
// host-side builds get the pure byte helpers only, no host.h
// dependency.
#if defined(__wasm32__)
#include <pyde/host.h>
#define PYDE_CHILD_MEMCPY_ __builtin_memcpy
#define PYDE_CHILD_MEMCMP_ __builtin_memcmp
#else
#include <string.h>
#define PYDE_CHILD_MEMCPY_ memcpy
#define PYDE_CHILD_MEMCMP_ memcmp
#endif

#ifdef __cplusplus
extern "C" {
#endif

// Domain-separation prefix — exactly 11 ASCII bytes, no NUL:
// 70 79 64 65 2d 63 68 69 6c 64 3a ("pyde-child:").
#define PYDE_CHILD_PREFIX "pyde-child:"
#define PYDE_CHILD_PREFIX_LEN 11

// 11 (prefix) + 32 (parent) + 32 (template) + 32 (salt).
#define PYDE_CHILD_PREIMAGE_LEN 107

// Canonical preimage assembly. `template` is a C++ keyword, hence
// `template_addr` (this header stays C++-safe like host.h).
static inline void pyde_child_preimage(
    const uint8_t parent[32],
    const uint8_t template_addr[32],
    const uint8_t salt[32],
    uint8_t out[107])
{
    PYDE_CHILD_MEMCPY_(out, PYDE_CHILD_PREFIX, PYDE_CHILD_PREFIX_LEN);
    PYDE_CHILD_MEMCPY_(out + 11, parent, 32);
    PYDE_CHILD_MEMCPY_(out + 43, template_addr, 32);
    PYDE_CHILD_MEMCPY_(out + 75, salt, 32);
}

// Salt::of_unordered_pair source encoding: sort the two 32-byte
// values ascending BYTEWISE (memcmp order), concatenate — 64 raw
// bytes, no framing. Symmetric: (a, b) and (b, a) encode
// identically. Per the identity-salt rule the salt is then
// Poseidon2(out) — on-chain via hash_poseidon2.
static inline void pyde_unordered_pair_encoding(
    const uint8_t a[32],
    const uint8_t b[32],
    uint8_t out[64])
{
    const uint8_t* lo = (PYDE_CHILD_MEMCMP_(a, b, 32) <= 0) ? a : b;
    const uint8_t* hi = (lo == a) ? b : a;
    PYDE_CHILD_MEMCPY_(out, lo, 32);
    PYDE_CHILD_MEMCPY_(out + 32, hi, 32);
}

#if defined(__wasm32__)
// Convenience: preimage + on-chain Poseidon2 in one call — exactly
// what pyde::instantiate computes internally. Call it to PREDICT a
// child address (e.g. for a counterfactual transfer) before or
// without instantiating. WASM-only: imports hash_poseidon2.
static inline void pyde_child_address(
    const uint8_t parent[32],
    const uint8_t template_addr[32],
    const uint8_t salt[32],
    uint8_t out[32])
{
    uint8_t preimage[PYDE_CHILD_PREIMAGE_LEN];
    pyde_child_preimage(parent, template_addr, salt, preimage);
    hash_poseidon2(preimage, PYDE_CHILD_PREIMAGE_LEN, out);
}
#endif  // __wasm32__

#ifdef __cplusplus
}  // extern "C"
#endif

#endif // PYDE_CHILD_H
