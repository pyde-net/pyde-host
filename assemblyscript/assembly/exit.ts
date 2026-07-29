// Halt operations — pointer-free wrappers over the §7.9 host fns.
// Mirrors the Rust reference `pyde::return_` / `pyde::revert`
// (rust/pyde-host/src/lib.rs L902/L911), which are typed `-> !`.
//
// AssemblyScript has no "never" type, so these are typed `void` and end
// with `unreachable()`: the host has already halted the frame by the time
// the host fn returns, so the trailing `unreachable` is never executed
// on-chain — but it tells the AS type checker that control does not
// continue, making any code after a `writeReturn` / `revertStr` correctly
// dead. (This is the idiom host_fns.ts documents for `revert`.)

import { pyde_return, revert as h_revert } from "./host_fns";

// Set this call's return data and exit successfully. `data` is typically
// the borsh encoding of the entry's declared outputs.
export function writeReturn(data: StaticArray<u8>): void {
  pyde_return(changetype<usize>(data), data.length);
  unreachable();
}

// Revert the current call frame with a UTF-8 reason. All state changes
// since the frame started are discarded; the bytes surface as the failure
// payload to the caller / receipt.
export function revertStr(reason: string): void {
  const utf8 = String.UTF8.encode(reason);
  h_revert(changetype<usize>(utf8), utf8.byteLength);
  unreachable();
}
