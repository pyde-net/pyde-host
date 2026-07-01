# Changelog

All notable changes to `pyde-host` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project's versioning tracks the `HOST_FN_ABI_SPEC` version — see
[`README.md`](./README.md#versioning) for the exact policy.

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
