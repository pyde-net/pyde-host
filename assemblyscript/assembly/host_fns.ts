// Canonical Pyde host fn declarations — the entire pyde::* ABI an
// AssemblyScript contract can call into. Every fn here is declared
// in HOST_FN_ABI_SPEC §7
// (https://book.pyde.network/companion/HOST_FN_ABI_SPEC).
//
// Why this file is comprehensive: the otigen toolchain doesn't ship a
// per-language SDK. Authors declare imports directly via
// @external("pyde", "..."). Declaring every fn once here means you
// never have to copy-paste a signature from the spec.
//
// Pointer convention: every `usize` marked as a "*Ptr" parameter is a
// 32-bit offset into linear memory (wasm32 makes usize 32 bits). Get
// one from an AssemblyScript object's backing buffer with
// `changetype<usize>(obj)` (works on StaticArray / ArrayBuffer / typed
// arrays' dataStart, etc.).
//
// Unused declarations are stripped by the asc linker's dead-code
// elimination — the final .wasm only imports what you actually call.
// Safe to keep them all.

// ─────────────────────────────────────────────────────────────────────
// §7.1 Storage
// ─────────────────────────────────────────────────────────────────────

// Storage values are variable-length (capped at 16 KB per slot).
// Slot keys are always 32 bytes — derive them with hash_poseidon2
// using the canonical recipe `slot = Poseidon2(self_address || field
// [|| key])`.

@external("pyde", "sload")
export declare function sload(slot_ptr: usize, out_ptr: usize, out_max_len: i32): i32;

@external("pyde", "sstore")
export declare function sstore(slot_ptr: usize, val_ptr: usize, val_len: i32): void;

@external("pyde", "sdelete")
export declare function sdelete(slot_ptr: usize): void;

// ── Typed storage (schema-derived slots) ─────────────────────────────
//
// The typed-storage family lets the host derive slot keys from the
// declared field name (+ optional keys), so the contract never has to
// call `hash_poseidon2` itself. The host also validates the value's
// byte length against the field's declared type. Gas: `sstore`/`sload`
// base + a small surcharge for schema lookup + type validation.

// Scalar field write. Slot derives as
// `Poseidon2(self_address || field_name)`.
@external("pyde", "sstore_scalar")
export declare function sstore_scalar(field_ptr: usize, field_len: i32, value_ptr: usize, value_len: i32): i32;

// Scalar field read. Returns actual value length, or -1 if unset.
@external("pyde", "sload_scalar")
export declare function sload_scalar(field_ptr: usize, field_len: i32, out_ptr: usize, out_max_len: i32): i32;

// Scalar field delete.
@external("pyde", "sdelete_scalar")
export declare function sdelete_scalar(field_ptr: usize, field_len: i32): i32;

// 1-key map write. Slot derives as
// `Poseidon2(self_address || field_name || key)`.
@external("pyde", "sstore_map1")
export declare function sstore_map1(field_ptr: usize, field_len: i32, key_ptr: usize, key_len: i32, value_ptr: usize, value_len: i32): i32;

// 1-key map read.
@external("pyde", "sload_map1")
export declare function sload_map1(field_ptr: usize, field_len: i32, key_ptr: usize, key_len: i32, out_ptr: usize, out_max_len: i32): i32;

// 1-key map delete.
@external("pyde", "sdelete_map1")
export declare function sdelete_map1(field_ptr: usize, field_len: i32, key_ptr: usize, key_len: i32): i32;

// 2-key map write. Slot derives as
// `Poseidon2(self_address || field_name || k1 || k2)`.
@external("pyde", "sstore_map2")
export declare function sstore_map2(field_ptr: usize, field_len: i32, k1_ptr: usize, k1_len: i32, k2_ptr: usize, k2_len: i32, value_ptr: usize, value_len: i32): i32;

// 2-key map read.
@external("pyde", "sload_map2")
export declare function sload_map2(field_ptr: usize, field_len: i32, k1_ptr: usize, k1_len: i32, k2_ptr: usize, k2_len: i32, out_ptr: usize, out_max_len: i32): i32;

// 2-key map delete.
@external("pyde", "sdelete_map2")
export declare function sdelete_map2(field_ptr: usize, field_len: i32, k1_ptr: usize, k1_len: i32, k2_ptr: usize, k2_len: i32): i32;

// 3-key map write. Slot derives as
// `Poseidon2(self_address || field_name || k1 || k2 || k3)`.
@external("pyde", "sstore_map3")
export declare function sstore_map3(field_ptr: usize, field_len: i32, k1_ptr: usize, k1_len: i32, k2_ptr: usize, k2_len: i32, k3_ptr: usize, k3_len: i32, value_ptr: usize, value_len: i32): i32;

// 3-key map read.
@external("pyde", "sload_map3")
export declare function sload_map3(field_ptr: usize, field_len: i32, k1_ptr: usize, k1_len: i32, k2_ptr: usize, k2_len: i32, k3_ptr: usize, k3_len: i32, out_ptr: usize, out_max_len: i32): i32;

// 3-key map delete.
@external("pyde", "sdelete_map3")
export declare function sdelete_map3(field_ptr: usize, field_len: i32, k1_ptr: usize, k1_len: i32, k2_ptr: usize, k2_len: i32, k3_ptr: usize, k3_len: i32): i32;

// ─────────────────────────────────────────────────────────────────────
// §7.2 Account & balance
// ─────────────────────────────────────────────────────────────────────

@external("pyde", "balance")
export declare function balance(addr_ptr: usize, balance_out_ptr: usize): void;

@external("pyde", "transfer")
export declare function transfer(to_ptr: usize, amount_ptr: usize): i32;

// ─────────────────────────────────────────────────────────────────────
// §7.3 Execution context
// ─────────────────────────────────────────────────────────────────────

@external("pyde", "caller")
export declare function caller(addr_out_ptr: usize): i32;

@external("pyde", "origin")
export declare function origin(addr_out_ptr: usize): i32;

@external("pyde", "self_address")
export declare function self_address(addr_out_ptr: usize): i32;

@external("pyde", "wave_id")
export declare function wave_id(): i64;

@external("pyde", "wave_timestamp")
export declare function wave_timestamp(): i64;

@external("pyde", "chain_id")
export declare function chain_id(): i64;

// ─────────────────────────────────────────────────────────────────────
// §7.4 Transaction context
// ─────────────────────────────────────────────────────────────────────

// The engine registers both as `(out_ptr: i32) -> ()` — they always write
// their fixed-size buffer and never fail, so there is no result. wasmtime
// type-checks imports including results, so declaring an i32 result here makes
// any contract that calls tx_hash/tx_value fail instantiation on-chain.
@external("pyde", "tx_hash")
export declare function tx_hash(hash_out_ptr: usize): void;

@external("pyde", "tx_value")
export declare function tx_value(value_out_ptr: usize): void;

@external("pyde", "tx_gas_remaining")
export declare function tx_gas_remaining(): i64;

@external("pyde", "calldata_size")
export declare function calldata_size(): i32;

// calldata_copy uses the engine's in/out length convention:
// `out_len_ptr` points at a u32 buffer; on call, it holds the
// max bytes the contract is willing to accept; on return, it
// holds the actual bytes copied. The old 3-arg signature
// (offset + len) is rejected by the engine's import validator.
@external("pyde", "calldata_copy")
export declare function calldata_copy(out_ptr: usize, out_len_ptr: usize): i32;

// ─────────────────────────────────────────────────────────────────────
// §7.5 Events
// ─────────────────────────────────────────────────────────────────────

@external("pyde", "emit_event")
export declare function emit_event(
  topics_ptr: usize,
  topics_count: i32,
  data_ptr: usize,
  data_len: i32,
): i32;

// ─────────────────────────────────────────────────────────────────────
// §7.6 Hashing primitives
// ─────────────────────────────────────────────────────────────────────
//
// NB: the hash host fns in the current test runner have no return
// value. HOST_FN_ABI_SPEC §7.6 says they `-> i32` ("Returns: 0
// always."). Template matches the runner so `otigen test`
// instantiates the contract; the spec / runner divergence is tracked
// for follow-up.

@external("pyde", "hash_blake3")
export declare function hash_blake3(in_ptr: usize, in_len: i32, out_ptr: usize): void;

@external("pyde", "hash_poseidon2")
export declare function hash_poseidon2(in_ptr: usize, in_len: i32, out_ptr: usize): void;

@external("pyde", "hash_keccak256")
export declare function hash_keccak256(in_ptr: usize, in_len: i32, out_ptr: usize): void;

// ─────────────────────────────────────────────────────────────────────
// §7.7 Post-quantum cryptography
// ─────────────────────────────────────────────────────────────────────

@external("pyde", "falcon_verify")
export declare function falcon_verify(
  pk_ptr: usize,
  msg_ptr: usize,
  msg_len: i32,
  sig_ptr: usize,
  sig_len: i32,
): i32;

// ─────────────────────────────────────────────────────────────────────
// §7.8 Cross-contract calls
// ─────────────────────────────────────────────────────────────────────

@external("pyde", "cross_call")
export declare function cross_call(
  target_ptr: usize,
  fn_name_ptr: usize, fn_name_len: i32,
  calldata_ptr: usize, calldata_len: i32,
  value_ptr: usize,
  gas_limit: i64,
  return_data_out_ptr: usize,
  return_data_out_len_ptr: usize,
): i32;

@external("pyde", "cross_call_static")
export declare function cross_call_static(
  target_ptr: usize,
  fn_name_ptr: usize, fn_name_len: i32,
  calldata_ptr: usize, calldata_len: i32,
  gas_limit: i64,
  return_data_out_ptr: usize,
  return_data_out_len_ptr: usize,
): i32;

@external("pyde", "delegate_call")
export declare function delegate_call(
  target_ptr: usize,
  fn_name_ptr: usize, fn_name_len: i32,
  calldata_ptr: usize, calldata_len: i32,
  gas_limit: i64,
  return_data_out_ptr: usize,
  return_data_out_len_ptr: usize,
): i32;

// ─────────────────────────────────────────────────────────────────────
// §7.9 Halt operations
// ─────────────────────────────────────────────────────────────────────

// Wire name "return" — renamed because `return` is a TypeScript keyword.
@external("pyde", "return")
export declare function pyde_return(data_ptr: usize, data_len: i32): void;

// revert never returns. AssemblyScript can't express "noreturn" in
// the type system; after calling revert, do `unreachable()` or
// `assert(false)` to satisfy the type checker if needed.
@external("pyde", "revert")
export declare function revert(reason_ptr: usize, reason_len: i32): void;

// ─────────────────────────────────────────────────────────────────────
// §7.10 Explicit gas metering
// ─────────────────────────────────────────────────────────────────────

@external("pyde", "consume_gas")
export declare function consume_gas(amount: i64): void;

// ─────────────────────────────────────────────────────────────────────
// §7.11 VRF beacon
// ─────────────────────────────────────────────────────────────────────

@external("pyde", "beacon_get")
export declare function beacon_get(out_ptr: usize): i32;

// ─────────────────────────────────────────────────────────────────────
// §7.12 Factory instantiation
// ─────────────────────────────────────────────────────────────────────
//
// Create a child instance of the DEPLOYED template contract at
// template_addr_ptr (32 bytes), addressed by
// child_address(self, template, salt) — by reference: the child
// shares the template's cached code. salt_ptr → 32 opaque
// caller-derived bytes; init_* → borsh ctor args (≤ 16384 bytes;
// zero for ctor-less templates); value_ptr → 16-byte LE u128
// endowment; gas_limit < 0 = forward all remaining.
//
// child_addr_out_ptr (32 bytes) is written on every path past the
// early cap/bounds checks (0, -40, -43, -44, -45, -46, -3 — NOT
// -48). Return data carries the ctor's return value on 0 and its
// revert payload VERBATIM on -40. Codes: -40 ctor reverted (ATOMIC
// refund); -43 template not a contract; -44 occupied by a
// NON-mergeable account; -45 nonempty init on a ctor-less template;
// -46 PIP-2 prefix collision; -48 per-tx cap (64); -3 balance <
// endowment. Traps from view/static frames and at depth >= 1024.
//
// gas: 20000 base + 8 per init byte + the ctor's own gas.

@external("pyde", "instantiate")
export declare function instantiate(
  template_addr_ptr: usize,
  salt_ptr: usize,
  init_calldata_ptr: usize, init_calldata_len: i32,
  value_ptr: usize,
  gas_limit: i64,
  child_addr_out_ptr: usize,
  return_data_out_ptr: usize,
  return_data_out_len_ptr: usize,
): i32;
