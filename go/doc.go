/*
Package pyde provides canonical host-function declarations for
Pyde smart contracts compiled with TinyGo to wasm32-unknown-unknown.

Every function declared here corresponds one-to-one with an entry in
HOST_FN_ABI_SPEC §7 (https://book.pyde.network/companion/HOST_FN_ABI_SPEC).
Signatures, wire names, and gas costs are the canonical source; if this
file and the spec disagree, the spec wins.

# Usage

Pyde does not ship a maintained per-language SDK. Instead, this package
publishes a single canonical host_fns.go that contract authors copy into
their own package. The //go:wasmimport directives resolve at wasm
compile time regardless of Go package name, so the file works verbatim
once you rewrite the `package pyde` line to match your contract's
package.

Two consumption patterns are supported:

  - Copy host_fns.go directly into your contract source tree
    (recommended). Rename the package clause; keep everything else
    intact. TinyGo's wasm-ld dead-code elimination strips any
    unused imports from the final .wasm.

  - `go get github.com/pyde-net/pyde-host/go` and reference this
    package from a same-package fork. Because the host fns are
    unexported (lowercase) they cannot be called across package
    boundaries — the module is published primarily so `go get` can
    fetch the canonical file for vendoring.

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
