// Canonical Pyde host fn declarations for TinyGo contracts — the
// entire pyde::* ABI a TinyGo contract can call into. Every fn here
// is declared in HOST_FN_ABI_SPEC §7
// (https://book.pyde.network/companion/HOST_FN_ABI_SPEC).
//
// Usage:
//
//   import "github.com/pyde-net/pyde-host/go" as pyde
//
//   pyde.Sload(slotPtr, outPtr, outMaxLen)
//   pyde.Sstore(slotPtr, valPtr, valLen)
//   pyde.EmitEvent(topicsPtr, topicsCount, dataPtr, dataLen)
//
// Function names are PascalCase (Go export convention); the underlying
// wire names in the //go:wasmimport directive stay lowercase to match
// HOST_FN_ABI_SPEC. TinyGo separates the two.
//
// Pointer convention: every int32 marked with a "Ptr" suffix is a
// 32-bit offset into linear memory. Use:
//
//   int32(uintptr(unsafe.Pointer(&buf[0])))
//
// to obtain one from a Go local. Multi-byte integers cross the
// boundary in little-endian unless the spec explicitly says otherwise.
//
// Unused declarations are stripped by TinyGo's wasm-ld dead-code
// elimination — the final .wasm only imports what you actually call.
// Safe to depend on this package even if you use only a handful of
// host fns.

package pyde

// ─────────────────────────────────────────────────────────────────────
// §7.1 Storage
// ─────────────────────────────────────────────────────────────────────

// Storage values are variable-length (capped at 16 KB per slot). Slot
// keys are always 32 bytes — derive them with hash_poseidon2 using
// the canonical recipe `slot = Poseidon2(self_address || field || key)`.

// Sload reads a storage slot. Writes up to outMaxLen bytes; returns
// the actual length (so callers can detect truncation), or -1 for a
// missing slot.
//
// gas: 100 base + 1 per byte copied.
//
//go:wasmimport pyde sload
func Sload(slotPtr int32, outPtr int32, outMaxLen int32) int32

// Sstore writes valLen bytes to the slot. Capped at 16 KB.
//
// gas: 5,000 base + 32 per byte. ERR_FORBIDDEN from a view-attributed
// function traps.
//
//go:wasmimport pyde sstore
func Sstore(slotPtr int32, valPtr int32, valLen int32)

// Sdelete clears a storage slot (a subsequent sload returns -1).
//
// gas: 5,000 base. No refund (PIP-4 gas-no-refund).
//
//go:wasmimport pyde sdelete
func Sdelete(slotPtr int32)

// ── Typed storage (schema-derived slots) ─────────────────────────────
//
// The typed-storage family lets the host derive slot keys from the
// declared field name (+ optional keys), so the contract never has to
// call hash_poseidon2 itself. The host also validates the value's
// byte length against the field's declared type. Gas: sstore/sload
// base + a small surcharge for schema lookup + type validation.

// SstoreScalar writes to a declared scalar field. Slot derives as
// Poseidon2(self_address || field_name).
//
//go:wasmimport pyde sstore_scalar
func SstoreScalar(fieldPtr int32, fieldLen int32, valuePtr int32, valueLen int32) int32

// SloadScalar reads a scalar field. Returns actual value length, or
// -1 if never written.
//
//go:wasmimport pyde sload_scalar
func SloadScalar(fieldPtr int32, fieldLen int32, outPtr int32, outMaxLen int32) int32

// SdeleteScalar clears a scalar field.
//
//go:wasmimport pyde sdelete_scalar
func SdeleteScalar(fieldPtr int32, fieldLen int32) int32

// SstoreMap1 writes to a 1-key map field. Slot derives as
// Poseidon2(self_address || field_name || key).
//
//go:wasmimport pyde sstore_map1
func SstoreMap1(fieldPtr int32, fieldLen int32, keyPtr int32, keyLen int32, valuePtr int32, valueLen int32) int32

// SloadMap1 reads a 1-key map field.
//
//go:wasmimport pyde sload_map1
func SloadMap1(fieldPtr int32, fieldLen int32, keyPtr int32, keyLen int32, outPtr int32, outMaxLen int32) int32

// SdeleteMap1 clears a 1-key map entry.
//
//go:wasmimport pyde sdelete_map1
func SdeleteMap1(fieldPtr int32, fieldLen int32, keyPtr int32, keyLen int32) int32

// SstoreMap2 writes to a 2-key map field. Slot derives as
// Poseidon2(self_address || field_name || k1 || k2).
//
//go:wasmimport pyde sstore_map2
func SstoreMap2(fieldPtr int32, fieldLen int32, k1Ptr int32, k1Len int32, k2Ptr int32, k2Len int32, valuePtr int32, valueLen int32) int32

// SloadMap2 reads a 2-key map field.
//
//go:wasmimport pyde sload_map2
func SloadMap2(fieldPtr int32, fieldLen int32, k1Ptr int32, k1Len int32, k2Ptr int32, k2Len int32, outPtr int32, outMaxLen int32) int32

// SdeleteMap2 clears a 2-key map entry.
//
//go:wasmimport pyde sdelete_map2
func SdeleteMap2(fieldPtr int32, fieldLen int32, k1Ptr int32, k1Len int32, k2Ptr int32, k2Len int32) int32

// SstoreMap3 writes to a 3-key map field. Slot derives as
// Poseidon2(self_address || field_name || k1 || k2 || k3).
//
//go:wasmimport pyde sstore_map3
func SstoreMap3(fieldPtr int32, fieldLen int32, k1Ptr int32, k1Len int32, k2Ptr int32, k2Len int32, k3Ptr int32, k3Len int32, valuePtr int32, valueLen int32) int32

// SloadMap3 reads a 3-key map field.
//
//go:wasmimport pyde sload_map3
func SloadMap3(fieldPtr int32, fieldLen int32, k1Ptr int32, k1Len int32, k2Ptr int32, k2Len int32, k3Ptr int32, k3Len int32, outPtr int32, outMaxLen int32) int32

// SdeleteMap3 clears a 3-key map entry.
//
//go:wasmimport pyde sdelete_map3
func SdeleteMap3(fieldPtr int32, fieldLen int32, k1Ptr int32, k1Len int32, k2Ptr int32, k2Len int32, k3Ptr int32, k3Len int32) int32

// ─────────────────────────────────────────────────────────────────────
// §7.2 Account & balance
// ─────────────────────────────────────────────────────────────────────

// Balance reads another account's native-PYDE balance.
//
// addrPtr:        32-byte address.
// balanceOutPtr:  16-byte buffer (uint128 LE).
// gas: 100 base.
//
//go:wasmimport pyde balance
func Balance(addrPtr int32, balanceOutPtr int32) int32

// Transfer sends native PYDE from this contract's balance.
//
// toPtr:      32-byte recipient.
// amountPtr:  16-byte u128 amount (LE).
// gas: 7,000 base. Reverts with ERR_INSUFFICIENT_BALANCE if caller's
// Balance < amount.
//
//go:wasmimport pyde transfer
func Transfer(toPtr int32, amountPtr int32) int32

// ─────────────────────────────────────────────────────────────────────
// §7.3 Execution context
// ─────────────────────────────────────────────────────────────────────

// Caller writes the immediate caller's 32-byte address. For top-level
// transactions equal to origin(); for nested cross_call's the calling
// contract.
//
// gas: 5 base.
//
//go:wasmimport pyde caller
func Caller(addrOutPtr int32) int32

// Origin writes the externally-owned account that signed the tx,
// regardless of call nesting. Use sparingly: tx.origin checks are
// the source of the classic phishing footgun. Prefer caller() for
// authorization.
//
// gas: 5 base.
//
//go:wasmimport pyde origin
func Origin(addrOutPtr int32) int32

// SelfAddress writes this contract's own address.
//
// gas: 5 base.
//
//go:wasmimport pyde self_address
func SelfAddress(addrOutPtr int32) int32

// WaveId returns Pyde's consensus-round counter (uint64),
// monotonically increasing.
//
// gas: 2 base.
//
//go:wasmimport pyde wave_id
func WaveId() int64

// WaveTimestamp returns the wave's canonical timestamp in seconds
// since Unix epoch. Committee-attested, identical across validators.
// Use this instead of Go's time package (which doesn't exist in
// wasm-unknown anyway).
//
// gas: 2 base.
//
//go:wasmimport pyde wave_timestamp
func WaveTimestamp() int64

// ChainId returns the chain identifier (1 = mainnet, 31337 = devnet).
//
// gas: 2 base.
//
//go:wasmimport pyde chain_id
func ChainId() int64

// ─────────────────────────────────────────────────────────────────────
// §7.4 Transaction context
// ─────────────────────────────────────────────────────────────────────

// TxHash writes the 32-byte Blake3 hash of the executing tx.
//
// gas: 5 base.
//
//go:wasmimport pyde tx_hash
func TxHash(hashOutPtr int32) int32

// TxValue writes the PYDE value attached to the current call
// (uint128 LE in 16 bytes). Always zero for non-payable functions.
//
// gas: 5 base.
//
//go:wasmimport pyde tx_value
func TxValue(valueOutPtr int32) int32

// TxGasRemaining returns the remaining gas (fuel) in the current
// call frame.
//
// gas: 2 base.
//
//go:wasmimport pyde tx_gas_remaining
func TxGasRemaining() int64

// CalldataSize returns the total byte-length of the current
// invocation's calldata buffer.
//
// gas: 2 base.
//
//go:wasmimport pyde calldata_size
func CalldataSize() int32

// CalldataCopy copies calldata into outPtr using the in/out
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
func CalldataCopy(outPtr int32, outLenPtr int32) int32

// ─────────────────────────────────────────────────────────────────────
// §7.5 Events
// ─────────────────────────────────────────────────────────────────────

// EmitEvent appends an event log entry to the transaction receipt.
//
// topicsCount: 1..=4. topic[0] is conventionally
// Blake3(canonical_event_signature). Indexed fields go in topics[1..];
// non-indexed payload goes in data.
//
// gas: 100 base + 50 × topicsCount + 8 per data byte.
//
//go:wasmimport pyde emit_event
func EmitEvent(topicsPtr int32, topicsCount int32, dataPtr int32, dataLen int32) int32

// ─────────────────────────────────────────────────────────────────────
// §7.6 Hashing primitives
// ─────────────────────────────────────────────────────────────────────
//
// NB: the hash host fns in the current test runner have no return value
// (gas: 15+ as listed). HOST_FN_ABI_SPEC §7.6 says they `-> i32`
// ("Returns: 0 always."). Template matches the runner so `otigen test`
// instantiates the contract; the spec / runner divergence is tracked
// for follow-up.

// HashBlake3 computes Blake3 over inPtr[..inLen], writes 32 bytes
// to outPtr. General-purpose hash — address derivation, event topic-0,
// content addressing.
//
// gas: 15 base + 3 per word (8 bytes).
//
//go:wasmimport pyde hash_blake3
func HashBlake3(inPtr int32, inLen int32, outPtr int32)

// HashPoseidon2 computes Poseidon2 over inPtr[..inLen], writes 32
// bytes to outPtr. ZK-friendly but more expensive than Blake3 in
// native execution. Use for slot derivation + state-root commitments.
//
// gas: 100 base + 30 per word.
//
//go:wasmimport pyde hash_poseidon2
func HashPoseidon2(inPtr int32, inLen int32, outPtr int32)

// HashKeccak256 computes Keccak256. Provided for cross-chain interop
// (verifying Ethereum Merkle Patricia proofs). Pyde itself doesn't
// use Keccak natively.
//
// gas: 30 base + 6 per word.
//
//go:wasmimport pyde hash_keccak256
func HashKeccak256(inPtr int32, inLen int32, outPtr int32)

// ─────────────────────────────────────────────────────────────────────
// §7.7 Post-quantum cryptography
// ─────────────────────────────────────────────────────────────────────

// FalconVerify verifies a FALCON-512 signature.
//
// pkPtr:           ~897-byte FALCON-512 public key.
// msgPtr/msgLen:   arbitrary message.
// sigPtr/sigLen:   signature bytes (variable, ~660-690).
//
// Returns 0 if valid, ERR_SIGNATURE_INVALID otherwise.
//
//go:wasmimport pyde falcon_verify
func FalconVerify(pkPtr int32, msgPtr int32, msgLen int32, sigPtr int32, sigLen int32) int32

// ─────────────────────────────────────────────────────────────────────
// §7.8 Cross-contract calls
// ─────────────────────────────────────────────────────────────────────

// CrossCall synchronously calls into another contract.
//
// gas: 1,000 base + 8 per calldata byte + sub-call gas_used.
// Sub-call runs in a nested overlay — state changes merge on success
// or roll back on revert.
//
//go:wasmimport pyde cross_call
func CrossCall(
	targetPtr int32,
	fnNamePtr int32, fnNameLen int32,
	calldataPtr int32, calldataLen int32,
	valuePtr int32,
	gasLimit int64,
	returnDataOutPtr int32,
	returnDataOutLenPtr int32,
) int32

// CrossCallStatic is the view-only variant of cross_call. Target
// must be a view function. Sub-call is FREE for the caller — see
// HOST_FN_ABI_SPEC §7.8.
//
// gas: 50 base for dispatch.
//
//go:wasmimport pyde cross_call_static
func CrossCallStatic(
	targetPtr int32,
	fnNamePtr int32, fnNameLen int32,
	calldataPtr int32, calldataLen int32,
	gasLimit int64,
	returnDataOutPtr int32,
	returnDataOutLenPtr int32,
) int32

// DelegateCall executes target's code in THIS contract's storage
// context. Used by proxies / upgradeable contracts. See
// HOST_FN_ABI_SPEC §7.8 for the security model.
//
// gas: 1,200 base + 8 per calldata byte + sub-call gas_used.
//
//go:wasmimport pyde delegate_call
func DelegateCall(
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

// PydeReturn sets this call's return data and exits successfully.
// Useful for functions that return variable-length data — the WASM
// ABI return value is a single primitive; this lets you "return"
// bytes via the caller's returnDataOut buffer.
//
// Wire name: "return". Renamed here because `return` is a Go keyword.
//
//go:wasmimport pyde return
func PydeReturn(dataPtr int32, dataLen int32)

// Revert reverts the current call frame. All state changes since the
// call started are discarded. Reason bytes surface as the failure
// payload to the caller (or to the tx receipt if top-level).
//
// Never returns. TinyGo doesn't have a `noreturn` directive — call it
// then add `for {}` or `panic("unreachable")` after for control-flow.
//
//go:wasmimport pyde revert
func Revert(reasonPtr int32, reasonLen int32)

// ─────────────────────────────────────────────────────────────────────
// §7.10 Explicit gas metering
// ─────────────────────────────────────────────────────────────────────

// ConsumeGas charges `amount` units of gas explicitly. Used by
// contracts that perform off-fuel work (synchronous loops bounded by
// external data) and want the cost visible in receipts.
//
// gas: 2 base + amount.
//
//go:wasmimport pyde consume_gas
func ConsumeGas(amount int64) int32

// ─────────────────────────────────────────────────────────────────────
// §7.11 VRF beacon
// ─────────────────────────────────────────────────────────────────────

// BeaconGet writes the current wave's committee-derived VRF beacon
// (32 bytes). Deterministic, public randomness. Publicly predictable
// within a wave — use threshold encryption if you need
// adversary-private randomness.
//
// gas: 50 base.
//
//go:wasmimport pyde beacon_get
func BeaconGet(outPtr int32) int32

// ─────────────────────────────────────────────────────────────────────
// §7.12 Factory instantiation
// ─────────────────────────────────────────────────────────────────────

// Instantiate creates a child instance of the DEPLOYED template
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
func Instantiate(
	templatePtr int32,
	saltPtr int32,
	initCalldataPtr int32, initCalldataLen int32,
	valuePtr int32,
	gasLimit int64,
	childAddrOutPtr int32,
	returnDataOutPtr int32,
	returnDataOutLenPtr int32,
) int32
