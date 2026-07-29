// Execution + transaction context — pointer-free wrappers over the
// §7.3 / §7.4 / §7.11 host fns. Mirrors the Rust reference `pyde::ctx::*`
// (rust/pyde-host/src/lib.rs L678): allocate the output buffer, call the
// host fn, decode the bytes into a typed value.
//
// The address/hash getters ignore the host fn's i32 status return (it is
// always 0 for these fixed-width writers) and hand back the buffer the
// host filled. The scalar getters (wave/chain/gas) come back as i64 from
// the host and are re-typed to u64 — the chain never produces a negative
// value for any of them, so the reinterpret is lossless.

import {
  caller as h_caller,
  origin as h_origin,
  self_address as h_self_address,
  tx_hash as h_tx_hash,
  tx_value as h_tx_value,
  tx_gas_remaining as h_tx_gas_remaining,
  wave_id as h_wave_id,
  wave_timestamp as h_wave_timestamp,
  chain_id as h_chain_id,
  beacon_get as h_beacon_get,
} from "@pyde-net/host/assembly";

import { Address, Bytes32, newAddress, newBytes32 } from "./types";
import { u128, readU128LE, U128_LEN } from "./u128";

// ── addresses ───────────────────────────────────────────────────────────

// Immediate caller of the current call frame. Equals `origin()` for
// top-level transactions; prefer this over `origin()` for authorization.
export function caller(): Address {
  const a = newAddress();
  h_caller(changetype<usize>(a));
  return a;
}

// The transaction originator (the EOA that signed). `tx.origin`-style
// checks are a phishing footgun — reach for `caller()` unless you truly
// need the originator.
export function origin(): Address {
  const a = newAddress();
  h_origin(changetype<usize>(a));
  return a;
}

// This contract's own address — the first input to the canonical
// storage-slot derivation `Poseidon2(self_address || field || key)`.
export function selfAddress(): Address {
  const a = newAddress();
  h_self_address(changetype<usize>(a));
  return a;
}

// ── 32-byte hashes ──────────────────────────────────────────────────────

// Hash of the currently-executing transaction.
export function txHash(): Bytes32 {
  const h = newBytes32();
  h_tx_hash(changetype<usize>(h));
  return h;
}

// Current wave's committee-derived VRF beacon (deterministic, but
// publicly predictable within the wave).
export function beacon(): Bytes32 {
  const h = newBytes32();
  h_beacon_get(changetype<usize>(h));
  return h;
}

// ── u128 value ──────────────────────────────────────────────────────────

// PYDE value attached to the current call. Always zero for non-`payable`
// functions. Decoded from the host's 16-byte little-endian buffer.
export function txValue(): u128 {
  const buf = new StaticArray<u8>(U128_LEN);
  h_tx_value(changetype<usize>(buf));
  return readU128LE(changetype<usize>(buf));
}

// ── u64 scalars ─────────────────────────────────────────────────────────

// Gas remaining in the current call frame.
export function txGasRemaining(): u64 { return <u64>h_tx_gas_remaining(); }

// Monotonic consensus-round counter.
export function waveId(): u64 { return <u64>h_wave_id(); }

// Canonical wave timestamp, seconds since Unix epoch (committee-attested).
export function waveTimestamp(): u64 { return <u64>h_wave_timestamp(); }

// Chain id (1 = mainnet, 31337 = devnet).
export function chainId(): u64 { return <u64>h_chain_id(); }
