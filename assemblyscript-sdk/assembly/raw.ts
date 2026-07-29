// Raw host-function declarations, re-exported.
//
// The ergonomic wrappers (ctx / calldata / exit / hash) cover the common
// surface, but a contract sometimes needs a host fn the SDK doesn't wrap
// yet — storage (`sstore_scalar` / `sload_scalar` / `sload` / `sstore`),
// cross-contract calls, `emit_event`, `instantiate`, and so on. Rather
// than force a contract to also depend on `@pyde-net/host` directly (a
// phantom dependency), the SDK re-exports the entire declaration surface
// here, mirroring the Rust reference's `pyde::raw::*`
// (rust/pyde-host/src/lib.rs L132).
//
// Import via the submodule to avoid colliding with the wrappers — e.g.
// the raw `caller(out_ptr) -> i32` vs the wrapper `ctx.caller() ->
// Address`:
//
//   import { sstore_scalar, sload_scalar } from "@pyde-net/sdk/assembly/raw";
//
// Pointer convention is the raw ABI's: every `usize` is a byte offset in
// linear memory; get one with `changetype<usize>(staticArray)`.

export * from "@pyde-net/host/assembly";
