/*
Package pyde provides canonical host-function declarations for
Pyde smart contracts compiled with TinyGo to wasm32-unknown-unknown.

Every function declared here corresponds one-to-one with an entry in
HOST_FN_ABI_SPEC §7 (https://book.pyde.network/companion/HOST_FN_ABI_SPEC).
Signatures, wire names, and gas costs are the canonical source; if this
file and the spec disagree, the spec wins.

# Usage

Install:

	go get github.com/pyde-net/pyde-host/go

Import and call:

	import "github.com/pyde-net/pyde-host/go"

	// example: read a storage slot
	var out [32]byte
	n := pyde.Sload(
	    int32(uintptr(unsafe.Pointer(&slot[0]))),
	    int32(uintptr(unsafe.Pointer(&out[0]))),
	    32,
	)

Function names are PascalCase to satisfy Go's export rules. The wire
names on the underlying //go:wasmimport directives stay lowercase to
match HOST_FN_ABI_SPEC — TinyGo separates directive name from Go
identifier, so pyde.Sload imports the wire host fn "sload".

# Toolchain

Compile with TinyGo targeting wasm-unknown:

	tinygo build -target=wasm-unknown -o contract.wasm .

Standard Go's wasm target (GOOS=js GOARCH=wasm) is NOT supported —
it emits a Go-runtime-heavy binary that will not link against the
minimal Pyde host ABI.

# Pointer convention

Every parameter typed int32 and named with a `Ptr` suffix is a 32-bit
offset into the contract's WebAssembly linear memory. Obtain one from
a Go local via:

	int32(uintptr(unsafe.Pointer(&buf[0])))

Multi-byte integers cross the boundary in little-endian byte order
unless the spec explicitly says otherwise.
*/
package pyde
