# pyde-host — Go bindings

Canonical `//go:wasmimport` declarations for every host function in
the Pyde host ABI. One file, no dependencies, matches
[`HOST_FN_ABI_SPEC.md`](https://book.pyde.network/companion/HOST_FN_ABI_SPEC)
one-to-one.

## What this is

A Go package (`pyde`) that declares the entire `pyde::*` host-function
surface a smart contract can call into. Storage, account/balance,
execution context, events, hashing, post-quantum crypto, cross-contract
calls, halts, gas metering, and the VRF beacon — every fn from spec
§7.1 through §7.11.

## Install

```
go get github.com/pyde-net/pyde-host/go@latest
```

Then import and call:

```go
import "github.com/pyde-net/pyde-host/go"

func main() {
    var out [32]byte
    n := pyde.Sload(
        int32(uintptr(unsafe.Pointer(&slot[0]))),
        int32(uintptr(unsafe.Pointer(&out[0]))),
        32,
    )
    _ = n
}
```

Every host fn is exported as `pyde.<PascalCase>(...)`. The underlying
wire name in the `//go:wasmimport pyde <name>` directive stays
lowercase snake_case to match the ABI spec — TinyGo separates
directive name from Go identifier, so `pyde.Sload` imports the wire
host fn `sload`.

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

Safe to import the whole package even if you use only a handful of host
fns — TinyGo's `wasm-ld` dead-code elimination strips imports your
contract never calls, so the final `.wasm` only lists what you actually
use.

## Reference

- Full ABI: [`HOST_FN_ABI_SPEC.md`](https://book.pyde.network/companion/HOST_FN_ABI_SPEC)
- Rust (crates.io: `pyde-host`), AssemblyScript (`@pyde-net/host`), and
  C bindings live alongside this directory.
