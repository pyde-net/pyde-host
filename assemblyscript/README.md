# @pyde-net/host — AssemblyScript

Canonical AssemblyScript declarations for every `pyde::*` host function
a WASM contract can call into. One file, no runtime code, no external
dependencies. Every extern is annotated with its section in
[`HOST_FN_ABI_SPEC.md`](../HOST_FN_ABI_SPEC.md) and its gas cost.

## Install

Via npm:

```bash
npm install @pyde-net/host
```

Then import from your contract via the `/assembly` subpath (the
AssemblyScript library convention — the same shape as `as-bignum/assembly`):

```ts
import {
  sload,
  sstore,
  self_address,
  emit_event,
  hash_poseidon2,
} from "@pyde-net/host/assembly";
```

Or copy `assembly/host_fns.ts` directly into your project's `assembly/`
directory and import it locally. Both approaches produce identical
`.wasm` — unused externs are stripped by the `asc` linker's dead-code
elimination.

## Pointer convention

Every `usize` marked as a `*Ptr` parameter is a 32-bit offset into
linear memory (wasm32 makes `usize` 32 bits). Get one from an
AssemblyScript object's backing buffer with `changetype<usize>(obj)` —
works on `StaticArray`, `ArrayBuffer`, and typed arrays' `dataStart`.

## ABI stability

This package tracks the Pyde host ABI. Breaking changes to a host fn
signature are versioned per the one-way ratchet documented in
`HOST_FN_ABI_SPEC.md` §12: no removals after v1 mainnet, only
additions. Pin an exact version if you need bit-for-bit reproducibility
across builds.

## License

MIT OR Apache-2.0
