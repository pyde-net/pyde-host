// scalar-store-as — the live acceptance contract for @pyde-net/sdk
// (issue #9, criterion 3). It links ONLY @pyde-net/sdk and proves the
// runtime foundation works end-to-end on the real engine:
//
//   • borsh decode of the entry argument   (BorshDecoder)
//   • the typed-storage host fns           (sstore_scalar / sload_scalar)
//   • borsh encode of the return value     (BorshEncoder + writeReturn)
//   • the abort handler                    (no env import, deploy-clean)
//
// `set(value)` writes; `get()` reads the exact value back — the round-trip
// the acceptance criterion requires.

import { BorshEncoder, BorshDecoder, read, writeReturn } from "@pyde-net/sdk/assembly";
import { sstore_scalar, sload_scalar } from "@pyde-net/sdk/assembly/raw";
import { abort as sdkAbort } from "@pyde-net/sdk/assembly/abort";

// Local, non-exported abort shim so asconfig's
// `use: ["abort=assembly/index/abort"]` resolves to the SDK's handler
// (routes overflow/bounds traps → pyde.revert; never emits `env.abort`).
// otigen rejects any exported-but-undeclared function, so this stays
// private — the `--use` directive finds it by module path regardless.
function abort(
  message: string | null = null,
  fileName: string | null = null,
  line: u32 = 0,
  column: u32 = 0,
): void {
  sdkAbort(message, fileName, line, column);
}

// Storage field "value", declared in otigen.toml [state]. The host
// derives the slot as Poseidon2(self_address || "value") from this name,
// so the contract never hashes a slot itself.
const FIELD_VALUE: StaticArray<u8> = [0x76, 0x61, 0x6c, 0x75, 0x65]; // "value"

// set(value: u64) — decode the borsh u64 argument and persist it.
export function set(): void {
  const value = new BorshDecoder(read()).u64();
  const encoded = new BorshEncoder().u64(value).toBytes();
  sstore_scalar(
    changetype<usize>(FIELD_VALUE), FIELD_VALUE.length,
    changetype<usize>(encoded), encoded.length,
  );
}

// get() -> u64 — load the stored value (0 if never written) and return it.
export function get(): void {
  const buf = new StaticArray<u8>(8);
  const actual = sload_scalar(
    changetype<usize>(FIELD_VALUE), FIELD_VALUE.length,
    changetype<usize>(buf), 8,
  );
  const enc = new BorshEncoder();
  if (actual == 8) enc.u64(new BorshDecoder(buf).u64());
  else enc.u64(0); // -1 = slot never written
  writeReturn(enc.toBytes());
}
