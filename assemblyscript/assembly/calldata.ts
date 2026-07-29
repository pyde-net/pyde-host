// Calldata access — pointer-free wrapper over the §7.4 host fns. Mirrors
// the Rust reference `pyde::calldata::{size, read}`
// (rust/pyde-host/src/lib.rs L788).
//
// `read()` follows the engine's in/out length convention for
// `calldata_copy` (host_fns.ts): `out_len_ptr` points at a 4-byte LE u32
// that the CONTRACT sets to the max bytes it will accept, and the HOST
// overwrites with the actual bytes copied. We size the buffer to the
// probed calldata length and pass that as the limit, so no truncation.

import { calldata_size, calldata_copy } from "./host_fns";
import { writeU32LE } from "./codec";

// Byte length of the current invocation's calldata.
// @ts-ignore: decorator
@inline export function size(): i32 { return calldata_size(); }

// Read the entire calldata buffer into an owned StaticArray<u8>. This is
// the buffer you hand to `new BorshDecoder(...)` to decode the entry's
// arguments.
export function read(): StaticArray<u8> {
  const n = calldata_size();
  const buf = new StaticArray<u8>(n);
  if (n > 0) {
    // limit := n (LE u32); the host writes the actual length back here.
    const limit = new StaticArray<u8>(4);
    writeU32LE(changetype<usize>(limit), <u32>n);
    calldata_copy(changetype<usize>(buf), changetype<usize>(limit));
  }
  return buf;
}
