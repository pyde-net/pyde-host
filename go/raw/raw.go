//go:build tinygo

package raw

// The raw //go:wasmimport bindings follow; see doc.go for the package
// overview. Signatures match EXACTLY what the engine registers (params +
// results) — a result-arity mismatch fails the on-chain import type check
// and the otigen-abi validator. Unused bindings are stripped by TinyGo's
// wasm-ld dead-code elimination, so importing this package costs nothing
// a contract does not actually call.

// ─────────────────────────────────────────────────────────────────────
// §7.1 Storage — raw slots
// ─────────────────────────────────────────────────────────────────────

// Sload reads a storage slot into outPtr[..outMaxLen]; returns the
// actual length (detect truncation), or -1 for a missing slot.
// Slot keys are 32 bytes. gas: 100 base + 1 per byte copied.
//
//go:wasmimport pyde sload
func Sload(slotPtr int32, outPtr int32, outMaxLen int32) int32

// Sstore writes valLen bytes to the slot (capped 16 KB).
// gas: 5,000 base + 32 per byte. Traps from a view function.
//
//go:wasmimport pyde sstore
func Sstore(slotPtr int32, valPtr int32, valLen int32)

// Sdelete clears a storage slot (a later Sload returns -1).
// gas: 5,000 base. No refund (PIP-4).
//
//go:wasmimport pyde sdelete
func Sdelete(slotPtr int32)

// ── Typed storage (host derives slot keys from the schema) ───────────

// SstoreScalar writes to a declared scalar field.
//
//go:wasmimport pyde sstore_scalar
func SstoreScalar(fieldPtr int32, fieldLen int32, valuePtr int32, valueLen int32) int32

// SloadScalar reads a scalar field. Returns actual length, or -1.
//
//go:wasmimport pyde sload_scalar
func SloadScalar(fieldPtr int32, fieldLen int32, outPtr int32, outMaxLen int32) int32

// SdeleteScalar clears a scalar field.
//
//go:wasmimport pyde sdelete_scalar
func SdeleteScalar(fieldPtr int32, fieldLen int32) int32

// SstoreMap1 writes to a 1-key map field.
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

// SstoreMap2 writes to a 2-key map field.
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

// SstoreMap3 writes to a 3-key map field.
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

// Balance reads another account's native-PYDE balance into a 16-byte
// buffer (uint128 LE). Void return (engine registers `(i32 i32) -> ()`).
//
//go:wasmimport pyde balance
func Balance(addrPtr int32, balanceOutPtr int32)

// Transfer sends native PYDE from this contract. toPtr: 32-byte
// recipient; amountPtr: 16-byte u128 LE. gas: 7,000 base.
//
//go:wasmimport pyde transfer
func Transfer(toPtr int32, amountPtr int32) int32

// ─────────────────────────────────────────────────────────────────────
// §7.3 Execution context
// ─────────────────────────────────────────────────────────────────────

// Caller writes the immediate caller's 32-byte address. gas: 5.
//
//go:wasmimport pyde caller
func Caller(addrOutPtr int32) int32

// Origin writes the tx-signing EOA, regardless of call nesting. Prefer
// Caller for authorization (tx.origin is the classic phishing footgun).
//
//go:wasmimport pyde origin
func Origin(addrOutPtr int32) int32

// SelfAddress writes this contract's own address. gas: 5.
//
//go:wasmimport pyde self_address
func SelfAddress(addrOutPtr int32) int32

// WaveId returns Pyde's monotonic consensus-round counter. gas: 2.
//
//go:wasmimport pyde wave_id
func WaveId() int64

// WaveTimestamp returns the wave's canonical timestamp in MILLISECONDS
// since the Unix epoch (divide by 1000 for seconds). gas: 2.
//
//go:wasmimport pyde wave_timestamp
func WaveTimestamp() int64

// ChainId returns the chain identifier (1 = mainnet, 31337 = devnet).
//
//go:wasmimport pyde chain_id
func ChainId() int64

// ─────────────────────────────────────────────────────────────────────
// §7.4 Transaction context
// ─────────────────────────────────────────────────────────────────────

// TxHash writes the 32-byte Blake3 hash of the executing tx. Void
// return (engine registers `(i32) -> ()`). gas: 5.
//
//go:wasmimport pyde tx_hash
func TxHash(hashOutPtr int32)

// TxValue writes the PYDE value attached to the call (16-byte u128 LE);
// always zero for non-payable functions. Void return. gas: 5.
//
//go:wasmimport pyde tx_value
func TxValue(valueOutPtr int32)

// TxGasRemaining returns the remaining gas in the current frame. gas: 2.
//
//go:wasmimport pyde tx_gas_remaining
func TxGasRemaining() int64

// CalldataSize returns the byte-length of the invocation's calldata.
//
//go:wasmimport pyde calldata_size
func CalldataSize() int32

// CalldataCopy copies calldata into outPtr using the in/out length
// convention: the u32 at outLenPtr holds the max bytes on call and is
// overwritten with the actual bytes copied on return. gas: 8 + 1/byte.
//
//go:wasmimport pyde calldata_copy
func CalldataCopy(outPtr int32, outLenPtr int32) int32

// ─────────────────────────────────────────────────────────────────────
// §7.5 Events
// ─────────────────────────────────────────────────────────────────────

// EmitEvent appends an event log entry. topicsCount 1..=4; topic[0] is
// conventionally Blake3(canonical_event_signature). gas: 100 + 50 ×
// topicsCount + 8 per data byte.
//
//go:wasmimport pyde emit_event
func EmitEvent(topicsPtr int32, topicsCount int32, dataPtr int32, dataLen int32) int32

// ─────────────────────────────────────────────────────────────────────
// §7.6 Hashing primitives — write 32 bytes to outPtr
// ─────────────────────────────────────────────────────────────────────

// HashBlake3 computes Blake3 over inPtr[..inLen]. gas: 15 + 3 per word.
//
//go:wasmimport pyde hash_blake3
func HashBlake3(inPtr int32, inLen int32, outPtr int32)

// HashPoseidon2 computes Poseidon2 over inPtr[..inLen]. ZK-friendly;
// use for slot derivation. gas: 100 + 30 per word.
//
//go:wasmimport pyde hash_poseidon2
func HashPoseidon2(inPtr int32, inLen int32, outPtr int32)

// HashKeccak256 computes Keccak256 (cross-chain interop). gas: 30 + 6/word.
//
//go:wasmimport pyde hash_keccak256
func HashKeccak256(inPtr int32, inLen int32, outPtr int32)

// ─────────────────────────────────────────────────────────────────────
// §7.7 Post-quantum cryptography
// ─────────────────────────────────────────────────────────────────────

// FalconVerify verifies a FALCON-512 signature. Returns 0 if valid,
// ERR_SIGNATURE_INVALID otherwise.
//
//go:wasmimport pyde falcon_verify
func FalconVerify(pkPtr int32, msgPtr int32, msgLen int32, sigPtr int32, sigLen int32) int32

// ─────────────────────────────────────────────────────────────────────
// §7.8 Cross-contract calls
// ─────────────────────────────────────────────────────────────────────

// CrossCall synchronously calls into another contract. Sub-call runs in
// a nested overlay (merges on success, rolls back on revert).
// gas: 1,000 base + 8 per calldata byte + sub-call gas.
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

// CrossCallStatic is the view-only variant of CrossCall (target must be
// a view function; free for the caller). gas: 50 base for dispatch.
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
// context (proxies / upgradeable contracts). gas: 1,200 base + 8 per
// calldata byte + sub-call gas.
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

// PydeReturn sets this call's return data and exits successfully — the
// way to "return" variable-length bytes past the single-primitive WASM
// ABI return. Wire name "return" (a Go keyword, hence the Pyde prefix).
//
//go:wasmimport pyde return
func PydeReturn(dataPtr int32, dataLen int32)

// Revert reverts the current call frame, discarding its state changes.
// reason bytes surface as the failure payload. Never returns; TinyGo has
// no noreturn directive, so add `for {}` after the call.
//
//go:wasmimport pyde revert
func Revert(reasonPtr int32, reasonLen int32)

// ─────────────────────────────────────────────────────────────────────
// §7.10 Explicit gas metering
// ─────────────────────────────────────────────────────────────────────

// ConsumeGas charges `amount` units explicitly (for off-fuel work you
// want visible in receipts). Void return. gas: 2 base + amount.
//
//go:wasmimport pyde consume_gas
func ConsumeGas(amount int64)

// ─────────────────────────────────────────────────────────────────────
// §7.11 VRF beacon
// ─────────────────────────────────────────────────────────────────────

// BeaconGet writes the wave's committee-derived VRF beacon (32 bytes).
// Deterministic, publicly predictable within a wave. gas: 50 base.
//
//go:wasmimport pyde beacon_get
func BeaconGet(outPtr int32) int32

// ─────────────────────────────────────────────────────────────────────
// §7.12 Factory instantiation
// ─────────────────────────────────────────────────────────────────────

// Instantiate creates a child instance of the deployed template at
// templatePtr (32 bytes) by reference. See HOST_FN_ABI_SPEC §7.12 for
// the full return-code table. gas: 20000 base + 8 per init byte + ctor gas.
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
