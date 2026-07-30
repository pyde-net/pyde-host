# @pyde-net/host — AssemblyScript

The AssemblyScript SDK for Pyde contracts: canonical declarations for
every `pyde::*` host function a WASM contract can call into, plus the
pointer-free wrappers and codecs above them — LE integer codecs, 128-bit
math, borsh, `ctx` / `calldata` / `exit` / `hash`, and the factory
child-address helpers. Every extern is annotated with its section in
[`HOST_FN_ABI_SPEC.md`](../HOST_FN_ABI_SPEC.md) and its gas cost.

## Install

Via npm:

```bash
npm install @pyde-net/host
```

Then import from your contract via the `/assembly` subpath (the
AssemblyScript library convention — the same shape as `as-bignum/assembly`).
There are two subpaths, and the split matters:

```ts
// Ergonomic wrappers + codecs — pointer-free, what you normally want.
import {
  caller,          // ctx    → Address
  poseidon2,       // hash   → Bytes32
  BorshEncoder,    // borsh
  BorshDecoder,
  writeReturn,     // exit
  revertStr,
  Address,
  u128,
} from "@pyde-net/host/assembly";

// Raw @external("pyde", …) declarations, for host fns the wrappers
// don't cover yet — storage, events, cross-contract calls.
import {
  sload_scalar,
  sstore_scalar,
  emit_event,
} from "@pyde-net/host/assembly/raw";
```

The two live on separate subpaths so the raw `caller(out_ptr) -> i32`
doesn't collide with the wrapper `caller() -> Address`. Everything in
`raw` mirrors the Rust reference's `pyde::raw::*`.

Unused externs are stripped by the `asc` linker's dead-code elimination,
so importing broadly costs nothing in the final `.wasm`.

## Generated entry dispatch: the `__<fn>_impl` seam

Pyde requires every chain-facing export to have the WASM signature
`() -> ()`. Arguments arrive through `calldata_size` / `calldata_copy`
and results leave through `pyde::return`, both borsh-encoded. Writing
that marshalling by hand, per entry, is the bulk of AssemblyScript
contract boilerplate — and the easiest place to drift out of sync with
your `otigen.toml`.

`otigen build` generates it for you. For each `[functions.<name>]` you
write one ordinary function in `assembly/contract.ts` named
`__<name>_impl`, with real parameters and a real return type:

```toml
# otigen.toml
[functions.credit]
attributes = ["entry"]
inputs     = ["address", "uint128"]
outputs    = ["uint64"]
```

```ts
// assembly/contract.ts
import { Address, u128 } from "@pyde-net/host/assembly";

export function __credit_impl(who: Address, amount: u128): u64 {
  // no calldata, no pyde_return — just logic
  return newValue;
}
```

otigen emits the export into `assembly/pyde.generated.ts`:

```ts
export function credit(): void {
  const __d = new BorshDecoder(read());
  const __a0: Address = __d.address();
  const __a1: u128 = __d.u128();
  const __r: u64 = __credit_impl(__a0, __a1);
  const __e = new BorshEncoder();
  __e.u64(__r);
  writeReturn(__e.toBytes());
}
```

Your `assembly/index.ts` stays tiny — it re-exports the generated shims
(which is what makes them wasm exports) and hosts the abort handler that
`asconfig.json`'s `use: ["abort=assembly/index/abort"]` resolves:

```ts
import { abort as sdkAbort } from "@pyde-net/host/assembly/abort";

export * from "./pyde.generated";

function abort(
  message: string | null = null,
  fileName: string | null = null,
  line: u32 = 0,
  column: u32 = 0,
): void {
  sdkAbort(message, fileName, line, column);
}
```

### Type mapping

`inputs` / `outputs` tokens map onto this SDK's types and codecs:

| `otigen.toml` | AssemblyScript | borsh wire |
|---|---|---|
| `uint8` … `uint64` | `u8` … `u64` | fixed-width LE |
| `int8` … `int64` | `i8` … `i64` | fixed-width LE |
| `uint128` / `int128` | `u128` / `i128` | 16 bytes LE |
| `bool` | `bool` | 1 byte |
| `address` | `Address` | 32 raw bytes, no prefix |
| `hash32` / `bytes32` | `Bytes32` | 32 raw bytes, no prefix |
| `bytes` | `StaticArray<u8>` | u32-LE length + bytes |
| `string` | `string` | u32-LE byte length + UTF-8 |
| `vec(T)` | `Array<T>` | u32-LE count + elements |

No `outputs` ⇒ the `_impl` returns `void`. Multiple `outputs`, custom
`[types.*]`, and nested `vec(vec(T))` aren't generated yet — declare
`bytes` and borsh-encode inside the `_impl`.

Because these are the same borsh rules the Rust `#[pyde::entry]` macro
uses, an AssemblyScript contract and a Rust contract with identical
`[functions.*]` accept **byte-identical calldata** and return
byte-identical data.

### Drift can't ship

`pyde.generated.ts` is rewritten from the manifest on every build, so the
shim and `otigen.toml` can never disagree. If **your** `_impl` signature
stops matching, the build fails:

```
otigen [ERROR] AsCodegen: AssemblyScript entry signatures disagree with otigen.toml:
  function "set": otigen.toml declares inputs [uint32], but the code's
  signature is [uint64] — update [functions.set] to match the code
```

This catches what `asc` alone cannot. AssemblyScript implicitly widens
numerics, so a `uint32` input in front of an `_impl` taking `u64` would
otherwise compile clean and then decode calldata 4 bytes wider than the
caller encoded it. A missing `_impl` reports the exact signature to add.

### Opting in and out

Generated dispatch activates when `assembly/contract.ts` exists **and**
the project imports `@pyde-net/host`. Contracts that export their own
`() -> ()` entries from `index.ts` — including what
`otigen init --lang as` scaffolds — are left completely untouched;
adding `contract.ts` is the one-file opt-in, deleting it the opt-out.

Commit `pyde.generated.ts` (like a `.pb.go`): it keeps diffs honest and
lets editors resolve the exports without a build step.

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
