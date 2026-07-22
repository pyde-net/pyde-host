# Changelog

All notable changes to `pyde-host` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project's versioning tracks the `HOST_FN_ABI_SPEC` version — see
[`README.md`](./README.md#versioning) for the exact policy.

## [Unreleased]

### Added
- **Factory pattern (PIP-0006) groundwork — canonical child-address
  derivation + golden conformance vectors.** The engine's
  `pyde::instantiate` host fn (the 41st, shipped engine-side) addresses
  children by `Poseidon2("pyde-child:" ‖ parent ‖ template ‖ salt)`
  (107-byte fixed-width preimage). This release ships the toolchain
  half of that contract:
  - Rust binding: pure `child_preimage` / `child_address` (hashes via
    the `hash_poseidon2` host fn — the same Poseidon2 the engine's
    authoritative derivation uses) and `Salt` identity-salt helpers
    (`Salt::of`, `Salt::of_unordered_pair` with the sort-canonicalized
    pair handled once, plus their pure `encoding` surfaces for
    off-chain reproduction).
  - `vectors/child_address.json` — the shared golden set every
    implementation (engine KAT, 4 language bindings, rust/ts SDKs,
    CLI) must reproduce byte-for-byte, anchored on the engine's pinned
    KAT vector and covering zero/max/field-order edges plus
    identity-salt cases (empty borsh, counters, i128/u128 extremes,
    strings, unordered pairs, mixed tuples).
  - Conformance tests in the Rust CI replay the whole set against the
    reference `pyde-crypto` Poseidon2 (`=`-pinned dev-dependency; never
    in contract wasm).
  - `HOST_FN_ABI_SPEC.md`: §7.12 `instantiate` entry (wire signature,
    error semantics incl. atomic ctor-revert refund and mergeability,
    gas), the child-address derivation + anchor vector, the pinned
    `Instantiated` event layout, §4 error codes `-40..-48` (with
    `-41/-42/-47` reserved-dead), and the §10 gas-table row.
  The `instantiate` extern itself lands in all four language bindings
  together in the next release (the wire-name parity gate moves 40 → 41
  atomically).

## [0.1.0-alpha.7] — 2026-07-03

### Added
- `#[pyde::entry]` records each entry's true Rust signature into a
  `pyde.sig.v1` custom section (one text record per entry, linker-
  concatenated). `otigen build` cross-checks the manifest's declared
  `[functions.*]` inputs/outputs against these records and strips the
  section at bundle time, so a manifest that lies about a signature
  fails the build instead of producing garbage calldata at runtime.
  Tools that don't know the section ignore it; the chain never sees it.

## [0.1.0-alpha.6] — 2026-07-01

### Changed

- **AssemblyScript package restructured to the standard library layout.**
  Source moved from `src/` to `assembly/` and the package is now imported
  via the `@pyde-net/host/assembly` subpath (the AssemblyScript ecosystem
  convention, matching `as-bignum/assembly`). The previous `src/`-based
  layout with `main`/`types` pointing at `src/index.ts` did not resolve
  under `asc` — bare `import ... from "@pyde-net/host"` failed with
  `File '~lib/@pyde-net/host.ts' not found`. `package.json` now sets
  `main`/`types` to `assembly/index.ts` and ships `assembly/` in `files`.
- **AssemblyScript peer dependency widened** from `^0.27.0` to `>=0.27.0`
  so projects on `assemblyscript@0.28.x` install without an `ERESOLVE`
  peer conflict. The bindings use only stable `@external` decorators and
  `usize`/`i32`/`i64` types, which are unchanged across 0.27/0.28.
- Rust, Go, and C bindings republished at `0.1.0-alpha.6` with no
  functional change, so all four language bindings share a version
  identifier.

## [0.1.0-alpha.5] — 2026-07-01

### Changed

- **Go bindings**: every host fn is now exported (PascalCase Go identifier).
  External Go users can `import "github.com/pyde-net/pyde-host/go"` and
  call `pyde.Sload(...)`, `pyde.Sstore(...)`, etc. — the previous
  lowercase names were package-private and forced a copy-the-file pattern.
  The underlying `//go:wasmimport pyde <name>` wire names stay lowercase
  snake_case (TinyGo separates directive name from Go identifier), so the
  ABI surface is unchanged. Signature-parity check still enforces one
  wire name per host fn across all four bindings.
- Rust and AssemblyScript bindings republished at `0.1.0-alpha.5` with
  no functional change beyond the version bump, so consumers of all four
  language bindings track the same version identifier.

## [0.1.0-alpha.4] — 2026-07-01

### Added

- Initial split of `pyde-host` + `pyde-entry-macros` + `pyde-storage-macros`
  + `pyde-events-macros` out of the otigen mono-tree into a standalone
  public repo.
- AssemblyScript, Go, and C canonical bindings alongside the Rust crates,
  so contracts can be written in any of the four supported languages
  against the same on-chain ABI.
- CI signature-parity check to keep all four language bindings in lockstep
  with `HOST_FN_ABI_SPEC.md`. Any PR that touches the spec without
  matching updates in all four binding sets — or vice versa — fails CI.
