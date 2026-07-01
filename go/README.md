# pyde-host — Go bindings

Canonical `//go:wasmimport` declarations for every host function in
the Pyde host ABI. One file, no dependencies, matches
[`HOST_FN_ABI_SPEC.md`](https://book.pyde.network/companion/HOST_FN_ABI_SPEC)
one-to-one.

## What this is

A single Go source file (`host_fns.go`) that declares the entire
`pyde::*` host-function surface a smart contract can call into.
Storage, account/balance, execution context, events, hashing,
post-quantum crypto, cross-contract calls, halts, gas metering, and
the VRF beacon — every fn from spec §7.1 through §7.11.

Pyde does not ship a maintained per-language SDK. Contract authors
declare their host imports directly against the ABI. This file exists
so you never have to copy-paste a signature from the spec.

## Install

Two supported patterns:

**Vendor the file directly (recommended).** Copy `host_fns.go` into
your contract's source tree and change the `package pyde` line to
match your own package. The `//go:wasmimport` directives resolve at
wasm compile time regardless of Go package name.

**Or `go get` and re-export.** Fetch the module, then copy the file
into your package:

```
go get github.com/pyde-net/pyde-host/go
```

The functions in this file are intentionally unexported (lowercase)
so they can be invoked as `sload(...)`, `sstore(...)`, etc. from any
package that includes the file. They cannot be called across package
boundaries — this is a canonical-source repo, not a linked library.

## Toolchain requirement

Compile with TinyGo targeting `wasm-unknown`:

```
tinygo build -target=wasm-unknown -o contract.wasm .
```

Standard Go's `GOOS=js GOARCH=wasm` target is **not** supported. It
emits a Go-runtime-heavy binary that will not link against Pyde's
minimal host ABI.

## Pointer convention

Every int32 parameter with a `Ptr` suffix is a 32-bit offset into your
contract's linear memory. Obtain one from a Go local via:

```go
int32(uintptr(unsafe.Pointer(&buf[0])))
```

Multi-byte integers cross the boundary in little-endian order unless
the spec explicitly says otherwise.

## Unused declarations

Safe to leave every declaration in place — TinyGo's `wasm-ld`
dead-code elimination strips imports your contract never calls, so
the final `.wasm` only lists what you actually use.

## Reference

- Full ABI: [`HOST_FN_ABI_SPEC.md`](https://book.pyde.network/companion/HOST_FN_ABI_SPEC)
- Rust and AssemblyScript bindings live alongside this directory.
