// Account and balance (HOST_FN_ABI_SPEC §7.2) — native-PYDE balance reads
// and transfers. Amounts are `u128` quanta throughout, never a display
// value: 1 PYDE is 10^9 quanta, and the chain only ever deals in quanta.
//
// Semantics mirror the Go SDK's `account.go` exactly, so a contract ported
// between the two behaves identically rather than subtly differently.

import { balance as h_balance, transfer as h_transfer } from "./host_fns";
import { Address } from "./types";
import { u128, u128ToBytesLE, u128FromBytesLE } from "./u128";
import { revertStr } from "./exit";
import { STATUS_OK } from "./status";

/// `addr`'s native-PYDE balance, in quanta.
///
/// The host writes 16 little-endian bytes into the scratch buffer rather
/// than returning a value, since a `u128` does not fit a wasm return slot.
export function balanceOf(addr: Address): u128 {
  const out = new StaticArray<u8>(16);
  h_balance(changetype<usize>(addr), changetype<usize>(out));
  return u128FromBytesLE(out);
}

/// Send `amount` quanta of native PYDE from this contract to `to`,
/// reverting on insufficient balance.
///
/// A zero amount and a self-transfer are both no-op successes. Calling this
/// from a view frame traps: the engine forbids moving value there, and that
/// is enforced host-side rather than here.
export function transfer(to: Address, amount: u128): void {
  const b = u128ToBytesLE(amount);
  if (h_transfer(changetype<usize>(to), changetype<usize>(b)) != STATUS_OK) {
    revertStr("pyde: transfer failed (insufficient balance)");
  }
}

/// [[transfer]], but returns `false` on insufficient balance instead of
/// reverting — for a caller that wants to handle the shortfall (skip a
/// recipient, fall back to a queue) rather than abort the whole frame.
export function tryTransfer(to: Address, amount: u128): bool {
  const b = u128ToBytesLE(amount);
  return h_transfer(changetype<usize>(to), changetype<usize>(b)) == STATUS_OK;
}
