# pyde-host

**The Pyde contract ABI, in four languages.**

This repository hosts the canonical host-function bindings that every Pyde
smart contract calls into, published side-by-side in Rust (Cargo),
AssemblyScript (npm), Go (vendored), and C (copy-or-submodule). The four
sets of bindings are kept byte-for-byte in lockstep with the canonical
[`HOST_FN_ABI_SPEC.md`](./HOST_FN_ABI_SPEC.md) by a parity check in CI, so a
contract compiled from any supported language sees the same import
signatures, the same gas costs, and the same error codes on-chain.

## Layout

| Path                     | Contents                                                                 |
| ------------------------ | ------------------------------------------------------------------------ |
| `rust/`                  | The `pyde-host` crate plus `pyde-entry-macros`, `pyde-storage-macros`, and `pyde-events-macros`. |
| `assemblyscript/`        | The `@pyde-net/host` npm package: typed `.ts` declarations for every host fn. |
| `go/`                    | The `github.com/pyde-net/pyde-host/go` module for TinyGo contracts.      |
| `c/`                     | `c/include/pyde/host.h` — a single-header C binding, plus a minimal example. |
| `scripts/`               | Parity-check tooling that reads the spec and diffs it against each binding set. |
| `HOST_FN_ABI_SPEC.md`    | The frozen ABI spec. Source of truth for the parity check; mirrored from `pyde-book`. |

## Install

### Rust

```bash
cargo add pyde-host
```

Then in your contract:

```rust
use pyde_host::{storage, events, entry, log};
```

### AssemblyScript

```bash
npm install @pyde-net/host
```

Then in your contract:

```ts
import { storage, events, log } from "@pyde-net/host";
```

### Go (TinyGo)

```bash
go get github.com/pyde-net/pyde-host/go
```

Or vendor the file directly — `go/host.go` is a single file with no
runtime dependencies. TinyGo picks it up transparently.

### C

```bash
git submodule add https://github.com/pyde-net/pyde-host third_party/pyde-host
```

Or, since the C surface is one header, copy `c/include/pyde/host.h` into
your project. The header only declares the `import` names; you link them
by compiling to `wasm32-unknown-unknown` (or `wasm32-wasi` with wasi
imports stripped) and letting the Pyde host resolve them at instantiation.

## Versioning

`pyde-host`'s version tracks the `HOST_FN_ABI_SPEC` version, not the
publishing calendar of any one language ecosystem. That means:

- Every published Rust, AssemblyScript, Go, and C release under the same
  tag corresponds to the same frozen spec revision.
- Pre-mainnet, releases track the otigen release train. The current
  version, `0.1.0-alpha.5`, matches the ABI shipped by
  `otigen 0.1.0-alpha.5`.
- After mainnet, the ABI is under a one-way ratchet: only additions are
  allowed, and additions bump the minor version (`v1.0` → `v1.1`). Removing
  or renaming a host fn — or changing its signature, gas cost, or error
  taxonomy — would be a hard fork and is not on the table.

The spec's own version field (`ABI_VERSION` inside
`HOST_FN_ABI_SPEC.md`) is what the runtime checks at contract deploy time.
This repository's tag simply mirrors it.

## Contributing

To add a new host fn:

1. Add it to `HOST_FN_ABI_SPEC.md` first, with a full entry: signature,
   gas cost, error codes, and any parachain-only gating.
2. Add matching entries to all four language bindings in the same PR:
   - `rust/pyde-host/src/host.rs`
   - `assemblyscript/assembly/host.ts`
   - `go/host.go`
   - `c/include/pyde/host.h`
3. Run `scripts/parity-check.sh` locally. It parses the spec, diffs it
   against each binding, and fails if any language is missing the new fn
   or disagrees on its signature.

CI runs the parity check on every push. A PR that touches the spec
without also touching the four bindings — or vice versa — will not merge.

For anything else (bug fixes, doc improvements, example contracts), open
a PR against `main`. Small changes don't need a design doc; anything that
touches the ABI surface itself does.

## Spec

The canonical ABI spec lives at
[`book.pyde.network/companion/HOST_FN_ABI_SPEC`](https://book.pyde.network/companion/HOST_FN_ABI_SPEC.html)
and is mirrored verbatim into this repo as
[`HOST_FN_ABI_SPEC.md`](./HOST_FN_ABI_SPEC.md). If the two ever drift,
the book copy wins; the mirror is a convenience for offline builds and
for the CI parity check.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](./LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms
or conditions.
