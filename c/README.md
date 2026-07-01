# pyde-host — C bindings

Canonical C header for the Pyde host function ABI. This is the entire
`pyde::*` surface a WASM contract compiled from C can import, matching
[HOST_FN_ABI_SPEC §7](https://book.pyde.network/companion/HOST_FN_ABI_SPEC).

Pyde does not ship a per-language SDK — contracts declare their host
imports directly. `include/pyde/host.h` gives you every signature in one
place so you never have to hand-copy from the spec.

## Install

There is no C package registry, so pick whichever works for you:

**Git submodule** (recommended, tracks upstream):

```sh
git submodule add https://github.com/pyde-net/pyde-host vendor/pyde-host
```

Then point your build at `vendor/pyde-host/c/include`.

**Copy** (vendor the header directly):

```sh
mkdir -p include/pyde
curl -o include/pyde/host.h \
  https://raw.githubusercontent.com/pyde-net/pyde-host/main/c/include/pyde/host.h
```

## Build

Pyde contracts target `wasm32-unknown-unknown`. You need a `clang` with
the WASM backend (LLVM 15+) and `wasm-ld`:

```sh
clang --target=wasm32-unknown-unknown \
      -nostdlib -Wl,--no-entry -Wl,--export=<entry> \
      -Iinclude \
      -O2 -o contract.wasm contract.c
```

If you use GNU make, the `Makefile.pyde` fragment sets a
`PYDE_HOST_INCLUDE` variable:

```make
include vendor/pyde-host/c/Makefile.pyde
CFLAGS += $(PYDE_HOST_INCLUDE)
```

## Pointer convention

Every `const uint8_t*` / `uint8_t*` parameter is a 32-bit offset into
the contract's linear memory. Multi-byte integers cross the boundary
in little-endian unless the spec says otherwise. See the header
comments for per-fn details.
