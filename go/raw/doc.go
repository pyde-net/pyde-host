// Package raw is the low-level Pyde host-function escape hatch — the
// complete pyde::* ABI as exported //go:wasmimport bindings, one per
// HOST_FN_ABI_SPEC §7 wire name
// (https://book.pyde.network/companion/HOST_FN_ABI_SPEC).
//
// The ergonomic wrappers in the parent package (pyde.StoreScalar,
// pyde.Caller, pyde.Return, …) are what a contract should reach for
// first: they own the pointer marshalling and borsh (de)serialization so
// a bad offset is unrepresentable. Drop down to raw.* only when you need
// byte-exact control the typed layer does not offer — a custom slot
// scheme over raw.Sstore, a host fn the SDK has not wrapped yet, or
// cross-language ABI parity. This mirrors the Rust SDK's
// pyde_host::raw::* module and the C header's flat extern surface: every
// language exposes the same raw ABI as a first-class escape hatch, the
// way Solidity exposes inline assembly.
//
// Everything here speaks int32 offsets into linear memory — obtain one
// with int32(uintptr(unsafe.Pointer(&b[0]))). A wrong offset is a
// memory-safety bug the host cannot catch; this is the sharp tool, used
// deliberately.
//
// The bindings themselves are //go:build tinygo (a //go:wasmimport only
// resolves under the TinyGo wasm target). This file carries no build
// constraint so `go build`/`go test` of the pure-Go core in the parent
// module can still resolve this import path natively — under a native
// build the package is intentionally empty.
package raw
