//go:build tinygo

// Raw Pyde host-fn bindings — the entire pyde::* ABI a TinyGo contract
// can call into, one //go:wasmimport per HOST_FN_ABI_SPEC §7 wire name
// (https://book.pyde.network/companion/HOST_FN_ABI_SPEC).
//
// These are UNEXPORTED plumbing. Contract authors never call them
// directly — they use the ergonomic wrappers in this package (ctx.go,
// storage.go, exit.go, …) which own the pointer marshalling and
// borsh (de)serialization. The raw layer only speaks int32 offsets
// into linear memory, so a bad pointer here is a memory-safety bug;
// the wrappers exist to make that unrepresentable.
//
// WASM signatures here are matched EXACTLY to what the engine registers
// (params + results). A result-arity mismatch (e.g. declaring an i32
// return where the engine registers void) fails the on-chain import
// type check and the otigen-abi validator, so every signature is
// cross-checked against the engine func_wrap + host_fn_signature table.
// The wire name in each //go:wasmimport directive is what the engine
// sees at instantiation; the Go identifier is free to differ (and here
// is the lowercase camelCase of the wire name).
//
// Pointer convention: every int32 with a "Ptr" suffix is a 32-bit
// offset into linear memory, obtained via ptrOf/ptrOfArr (mem.go).
// Multi-byte integers cross the boundary little-endian unless a
// signature says otherwise.
//
// Unused declarations are stripped by TinyGo's wasm-ld dead-code
// elimination — the final .wasm only imports what a contract actually
// calls. Tagged //go:build tinygo so the pure-Go core (codec, borsh,
// u128, types) still compiles and tests natively.

package pyde

// ─────────────────────────────────────────────────────────────────────
// §7.1 Storage
// ─────────────────────────────────────────────────────────────────────

// Storage values are variable-length (capped at 16 KB per slot). Slot
// keys are always 32 bytes — derive them with hash_poseidon2 using
// the canonical recipe `slot = Poseidon2(self_address || field || key)`.

// sload reads a storage slot. Writes up to outMaxLen bytes; returns
// the actual length (so callers can detect truncation), or -1 for a
// missing slot.
//
// gas: 100 base + 1 per byte copied.
//
//go:wasmimport pyde sload
func sload(slotPtr int32, outPtr int32, outMaxLen int32) int32

// sstore writes valLen bytes to the slot. Capped at 16 KB.
//
// gas: 5,000 base + 32 per byte. ERR_FORBIDDEN from a view-attributed
// function traps.
//
//go:wasmimport pyde sstore
func sstore(slotPtr int32, valPtr int32, valLen int32)

// sdelete clears a storage slot (a subsequent sload returns -1).
//
// gas: 5,000 base. No refund (PIP-4 gas-no-refund).
//
//go:wasmimport pyde sdelete
func sdelete(slotPtr int32)

// ── Typed storage (schema-derived slots) ─────────────────────────────
//
// The typed-storage family lets the host derive slot keys from the
// declared field name (+ optional keys), so the contract never has to
// call hash_poseidon2 itself. The host also validates the value's
// byte length against the field's declared type. Gas: sstore/sload
// base + a small surcharge for schema lookup + type validation.

// sstoreScalar writes to a declared scalar field. Slot derives as
// Poseidon2(self_address || field_name).
//
//go:wasmimport pyde sstore_scalar
func sstoreScalar(fieldPtr int32, fieldLen int32, valuePtr int32, valueLen int32) int32

// sloadScalar reads a scalar field. Returns actual value length, or
// -1 if never written.
//
//go:wasmimport pyde sload_scalar
func sloadScalar(fieldPtr int32, fieldLen int32, outPtr int32, outMaxLen int32) int32

// sdeleteScalar clears a scalar field.
//
//go:wasmimport pyde sdelete_scalar
func sdeleteScalar(fieldPtr int32, fieldLen int32) int32

// sstoreMap1 writes to a 1-key map field. Slot derives as
// Poseidon2(self_address || field_name || key).
//
//go:wasmimport pyde sstore_map1
func sstoreMap1(fieldPtr int32, fieldLen int32, keyPtr int32, keyLen int32, valuePtr int32, valueLen int32) int32

// sloadMap1 reads a 1-key map field.
//
//go:wasmimport pyde sload_map1
func sloadMap1(fieldPtr int32, fieldLen int32, keyPtr int32, keyLen int32, outPtr int32, outMaxLen int32) int32

// sdeleteMap1 clears a 1-key map entry.
//
//go:wasmimport pyde sdelete_map1
func sdeleteMap1(fieldPtr int32, fieldLen int32, keyPtr int32, keyLen int32) int32

// sstoreMap2 writes to a 2-key map field. Slot derives as
// Poseidon2(self_address || field_name || k1 || k2).
//
//go:wasmimport pyde sstore_map2
func sstoreMap2(fieldPtr int32, fieldLen int32, k1Ptr int32, k1Len int32, k2Ptr int32, k2Len int32, valuePtr int32, valueLen int32) int32

// sloadMap2 reads a 2-key map field.
//
//go:wasmimport pyde sload_map2
func sloadMap2(fieldPtr int32, fieldLen int32, k1Ptr int32, k1Len int32, k2Ptr int32, k2Len int32, outPtr int32, outMaxLen int32) int32

// sdeleteMap2 clears a 2-key map entry.
//
//go:wasmimport pyde sdelete_map2
func sdeleteMap2(fieldPtr int32, fieldLen int32, k1Ptr int32, k1Len int32, k2Ptr int32, k2Len int32) int32

// sstoreMap3 writes to a 3-key map field. Slot derives as
// Poseidon2(self_address || field_name || k1 || k2 || k3).
//
//go:wasmimport pyde sstore_map3
func sstoreMap3(fieldPtr int32, fieldLen int32, k1Ptr int32, k1Len int32, k2Ptr int32, k2Len int32, k3Ptr int32, k3Len int32, valuePtr int32, valueLen int32) int32

// sloadMap3 reads a 3-key map field.
//
//go:wasmimport pyde sload_map3
func sloadMap3(fieldPtr int32, fieldLen int32, k1Ptr int32, k1Len int32, k2Ptr int32, k2Len int32, k3Ptr int32, k3Len int32, outPtr int32, outMaxLen int32) int32

// sdeleteMap3 clears a 3-key map entry.
//
//go:wasmimport pyde sdelete_map3
func sdeleteMap3(fieldPtr int32, fieldLen int32, k1Ptr int32, k1Len int32, k2Ptr int32, k2Len int32, k3Ptr int32, k3Len int32) int32

// ─────────────────────────────────────────────────────────────────────
// §7.2 Account & balance
// ─────────────────────────────────────────────────────────────────────

// balance reads another account's native-PYDE balance into a 16-byte
// buffer (uint128 LE). Void return: the engine registers balance as
// `(param i32 i32) -> ()` (HOST_FN_ABI_SPEC §7.2; verified against the
// engine func_wrap + otigen-abi validator). Declaring an i32 result
// makes the import fail the on-chain type check.
//
// addrPtr:        32-byte address.
// balanceOutPtr:  16-byte buffer (uint128 LE).
// gas: 100 base.
//
//go:wasmimport pyde balance
func balance(addrPtr int32, balanceOutPtr int32)

// transfer sends native PYDE from this contract's balance.
//
// toPtr:      32-byte recipient.
// amountPtr:  16-byte u128 amount (LE).
// gas: 7,000 base. Reverts with ERR_INSUFFICIENT_BALANCE if caller's
// balance < amount.
//
//go:wasmimport pyde transfer
func transfer(toPtr int32, amountPtr int32) int32

// ─────────────────────────────────────────────────────────────────────
// §7.3 Execution context
// ─────────────────────────────────────────────────────────────────────

// caller writes the immediate caller's 32-byte address. For top-level
// transactions equal to origin(); for nested cross_call's the calling
// contract.
//
// gas: 5 base.
//
//go:wasmimport pyde caller
func caller(addrOutPtr int32) int32

// origin writes the externally-owned account that signed the tx,
// regardless of call nesting. Use sparingly: tx.origin checks are
// the source of the classic phishing footgun. Prefer caller() for
// authorization.
//
// gas: 5 base.
//
//go:wasmimport pyde origin
func origin(addrOutPtr int32) int32

// selfAddress writes this contract's own address.
//
// gas: 5 base.
//
//go:wasmimport pyde self_address
func selfAddress(addrOutPtr int32) int32

// waveId returns Pyde's consensus-round counter (uint64),
// monotonically increasing.
//
// gas: 2 base.
//
//go:wasmimport pyde wave_id
func waveId() int64

// waveTimestamp returns the wave's canonical timestamp in
// MILLISECONDS since the Unix epoch — NOT seconds (the engine derives
// it as genesis_unix_secs*1000 + wave_id*500; see engine
// executor_impl.rs with_wave_timestamp(wave_timestamp_ms)). Divide by
// 1000 for seconds. Committee-attested, identical across validators.
// Use this instead of Go's time package (which doesn't exist in
// wasm-unknown anyway).
//
// gas: 2 base.
//
//go:wasmimport pyde wave_timestamp
func waveTimestamp() int64

// chainId returns the chain identifier (1 = mainnet, 31337 = devnet).
//
// gas: 2 base.
//
//go:wasmimport pyde chain_id
func chainId() int64

// ─────────────────────────────────────────────────────────────────────
// §7.4 Transaction context
// ─────────────────────────────────────────────────────────────────────

// txHash writes the 32-byte Blake3 hash of the executing tx. Void
// return: the engine registers tx_hash as `(param i32) -> ()`
// (HOST_FN_ABI_SPEC §7.4). An i32 result fails the on-chain import
// type check.
//
// gas: 5 base.
//
//go:wasmimport pyde tx_hash
func txHash(hashOutPtr int32)

// txValue writes the PYDE value attached to the current call
// (uint128 LE in 16 bytes). Always zero for non-payable functions.
// Void return: the engine registers tx_value as `(param i32) -> ()`
// (HOST_FN_ABI_SPEC §7.4).
//
// gas: 5 base.
//
//go:wasmimport pyde tx_value
func txValue(valueOutPtr int32)

// txGasRemaining returns the remaining gas (fuel) in the current
// call frame.
//
// gas: 2 base.
//
//go:wasmimport pyde tx_gas_remaining
func txGasRemaining() int64

// calldataSize returns the total byte-length of the current
// invocation's calldata buffer.
//
// gas: 2 base.
//
//go:wasmimport pyde calldata_size
func calldataSize() int32

// calldataCopy copies calldata into outPtr using the in/out
// length convention:
//   - On call: the u32 at outLenPtr holds the max bytes the
//     contract is willing to accept.
//   - On return: the u32 at outLenPtr is overwritten with the
//     actual bytes copied (≤ calldata_size()).
//
// gas: 8 base + 1 per byte. The old 3-arg signature
// (offset/length/outPtr) is rejected by the engine's import
// validator — the host fn never took an offset; pass the
// full buffer in one shot.
//
//go:wasmimport pyde calldata_copy
func calldataCopy(outPtr int32, outLenPtr int32) int32

// ─────────────────────────────────────────────────────────────────────
// §7.5 Events
// ─────────────────────────────────────────────────────────────────────

// emitEvent appends an event log entry to the transaction receipt.
//
// topicsCount: 1..=4. topic[0] is conventionally
// Blake3(canonical_event_signature). Indexed fields go in topics[1..];
// non-indexed payload goes in data.
//
// gas: 100 base + 50 × topicsCount + 8 per data byte.
//
//go:wasmimport pyde emit_event
func emitEvent(topicsPtr int32, topicsCount int32, dataPtr int32, dataLen int32) int32

// ─────────────────────────────────────────────────────────────────────
// §7.6 Hashing primitives
// ─────────────────────────────────────────────────────────────────────
//
// NB: the hash host fns in the current test runner have no return value
// (gas: 15+ as listed). HOST_FN_ABI_SPEC §7.6 says they `-> i32`
// ("Returns: 0 always."). Template matches the runner so `otigen test`
// instantiates the contract; the spec / runner divergence is tracked
// for follow-up.

// hashBlake3 computes Blake3 over inPtr[..inLen], writes 32 bytes
// to outPtr. General-purpose hash — address derivation, event topic-0,
// content addressing.
//
// gas: 15 base + 3 per word (8 bytes).
//
//go:wasmimport pyde hash_blake3
func hashBlake3(inPtr int32, inLen int32, outPtr int32)

// hashPoseidon2 computes Poseidon2 over inPtr[..inLen], writes 32
// bytes to outPtr. ZK-friendly but more expensive than Blake3 in
// native execution. Use for slot derivation + state-root commitments.
//
// gas: 100 base + 30 per word.
//
//go:wasmimport pyde hash_poseidon2
func hashPoseidon2(inPtr int32, inLen int32, outPtr int32)

// hashKeccak256 computes Keccak256. Provided for cross-chain interop
// (verifying Ethereum Merkle Patricia proofs). Pyde itself doesn't
// use Keccak natively.
//
// gas: 30 base + 6 per word.
//
//go:wasmimport pyde hash_keccak256
func hashKeccak256(inPtr int32, inLen int32, outPtr int32)

// ─────────────────────────────────────────────────────────────────────
// §7.7 Post-quantum cryptography
// ─────────────────────────────────────────────────────────────────────

// falconVerify verifies a FALCON-512 signature.
//
// pkPtr:           ~897-byte FALCON-512 public key.
// msgPtr/msgLen:   arbitrary message.
// sigPtr/sigLen:   signature bytes (variable, ~660-690).
//
// Returns 0 if valid, ERR_SIGNATURE_INVALID otherwise.
//
//go:wasmimport pyde falcon_verify
func falconVerify(pkPtr int32, msgPtr int32, msgLen int32, sigPtr int32, sigLen int32) int32

// ─────────────────────────────────────────────────────────────────────
// §7.8 Cross-contract calls
// ─────────────────────────────────────────────────────────────────────

// crossCall synchronously calls into another contract.
//
// gas: 1,000 base + 8 per calldata byte + sub-call gas_used.
// Sub-call runs in a nested overlay — state changes merge on success
// or roll back on revert.
//
//go:wasmimport pyde cross_call
func crossCall(
	targetPtr int32,
	fnNamePtr int32, fnNameLen int32,
	calldataPtr int32, calldataLen int32,
	valuePtr int32,
	gasLimit int64,
	returnDataOutPtr int32,
	returnDataOutLenPtr int32,
) int32

// crossCallStatic is the view-only variant of cross_call. Target
// must be a view function. Sub-call is FREE for the caller — see
// HOST_FN_ABI_SPEC §7.8.
//
// gas: 50 base for dispatch.
//
//go:wasmimport pyde cross_call_static
func crossCallStatic(
	targetPtr int32,
	fnNamePtr int32, fnNameLen int32,
	calldataPtr int32, calldataLen int32,
	gasLimit int64,
	returnDataOutPtr int32,
	returnDataOutLenPtr int32,
) int32

// delegateCall executes target's code in THIS contract's storage
// context. Used by proxies / upgradeable contracts. See
// HOST_FN_ABI_SPEC §7.8 for the security model.
//
// gas: 1,200 base + 8 per calldata byte + sub-call gas_used.
//
//go:wasmimport pyde delegate_call
func delegateCall(
	targetPtr int32,
	fnNamePtr int32, fnNameLen int32,
	calldataPtr int32, calldataLen int32,
	gasLimit int64,
	returnDataOutPtr int32,
	returnDataOutLenPtr int32,
) int32

// ─────────────────────────────────────────────────────────────────────
// §7.9 Halt operations
// ─────────────────────────────────────────────────────────────────────

// pydeReturn sets this call's return data and exits successfully.
// Useful for functions that return variable-length data — the WASM
// ABI return value is a single primitive; this lets you "return"
// bytes via the caller's returnDataOut buffer.
//
// Wire name: "return". Renamed here because `return` is a Go keyword.
//
//go:wasmimport pyde return
func pydeReturn(dataPtr int32, dataLen int32)

// revert reverts the current call frame. All state changes since the
// call started are discarded. Reason bytes surface as the failure
// payload to the caller (or to the tx receipt if top-level).
//
// Never returns. TinyGo doesn't have a `noreturn` directive — call it
// then add `for {}` or `panic("unreachable")` after for control-flow.
//
//go:wasmimport pyde revert
func revert(reasonPtr int32, reasonLen int32)

// ─────────────────────────────────────────────────────────────────────
// §7.10 Explicit gas metering
// ─────────────────────────────────────────────────────────────────────

// consumeGas charges `amount` units of gas explicitly. Used by
// contracts that perform off-fuel work (synchronous loops bounded by
// external data) and want the cost visible in receipts. Void return:
// the engine registers consume_gas as `(param i64) -> ()`
// (HOST_FN_ABI_SPEC §7.10).
//
// gas: 2 base + amount.
//
//go:wasmimport pyde consume_gas
func consumeGas(amount int64)

// ─────────────────────────────────────────────────────────────────────
// §7.11 VRF beacon
// ─────────────────────────────────────────────────────────────────────

// beaconGet writes the current wave's committee-derived VRF beacon
// (32 bytes). Deterministic, public randomness. Publicly predictable
// within a wave — use threshold encryption if you need
// adversary-private randomness.
//
// gas: 50 base.
//
//go:wasmimport pyde beacon_get
func beaconGet(outPtr int32) int32

// ─────────────────────────────────────────────────────────────────────
// §7.12 Factory instantiation
// ─────────────────────────────────────────────────────────────────────

// instantiate creates a child instance of the DEPLOYED template
// contract at templatePtr (32 bytes), addressed by
// child_address(self, template, salt) — by reference: the child
// shares the template's cached code, nothing is copied or
// recompiled. saltPtr → 32 opaque caller-derived bytes; init* →
// borsh ctor args (≤ 16384 bytes; zero for ctor-less templates);
// valuePtr → 16-byte LE u128 endowment; gasLimit < 0 = forward all
// remaining.
//
// childAddrOutPtr (32 bytes) is written on every path past the
// early cap/bounds checks (0, -40, -43, -44, -45, -46, -3 — NOT
// -48). Return data carries the ctor's return value on 0 and its
// revert payload VERBATIM on -40.
//
// Returns 0, or: -40 ctor reverted (ATOMIC refund — no child,
// endowment back); -43 template not a contract; -44 child address
// occupied by a NON-mergeable account (balance-only EOA shells
// merge instead); -45 nonempty init on a ctor-less template; -46
// PIP-2 prefix collision; -48 per-tx cap (64); -3 balance <
// endowment. Traps from view/static frames and at depth ≥ 1024.
//
// gas: 20000 base + 8 per init byte + the ctor's own gas.
//
//go:wasmimport pyde instantiate
func instantiate(
	templatePtr int32,
	saltPtr int32,
	initCalldataPtr int32, initCalldataLen int32,
	valuePtr int32,
	gasLimit int64,
	childAddrOutPtr int32,
	returnDataOutPtr int32,
	returnDataOutLenPtr int32,
) int32
