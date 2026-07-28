//go:build tinygo

// Execution & transaction context — typed accessors over the §7.3/§7.4
// host fns. Each returns a value type (Address, Bytes32, U128, uint64)
// instead of the raw "write 32 bytes to this offset" ABI, so contracts
// never touch a pointer.

package pyde

// Caller is the immediate caller's address. For a top-level transaction
// it equals Origin; for a nested cross-call it is the calling contract.
// Prefer Caller over Origin for authorization — Origin-based checks are
// the classic phishing footgun.
func Caller() Address {
	var a Address
	caller(ptr(a[:]))
	return a
}

// Origin is the externally-owned account that signed the transaction,
// regardless of call nesting. Use sparingly (see Caller).
func Origin() Address {
	var a Address
	origin(ptr(a[:]))
	return a
}

// Self is this contract's own address.
func Self() Address {
	var a Address
	selfAddress(ptr(a[:]))
	return a
}

// TxHash is the 32-byte Blake3 hash of the executing transaction.
func TxHash() Bytes32 {
	var h Bytes32
	txHash(ptr(h[:]))
	return h
}

// TxValue is the native PYDE value attached to the current call, in
// quanta. Always zero for non-payable functions.
func TxValue() U128 {
	var b [16]byte
	txValue(ptr(b[:]))
	return U128FromBytesLE(b[:])
}

// Beacon is the current wave's committee-derived VRF beacon: deterministic,
// public randomness (32 bytes). It is publicly predictable within a wave —
// never use it as an adversary-private secret.
func Beacon() Bytes32 {
	var b Bytes32
	beaconGet(ptr(b[:]))
	return b
}

// WaveId is Pyde's monotonically increasing consensus-round counter.
func WaveId() uint64 { return uint64(waveId()) }

// WaveTimestamp is the wave's committee-attested timestamp in
// MILLISECONDS since the Unix epoch (not seconds — divide by 1000 for
// seconds). Identical across validators; use it instead of wall-clock
// time, which does not exist under wasm-unknown.
func WaveTimestamp() uint64 { return uint64(waveTimestamp()) }

// ChainId is the chain identifier (1 = mainnet, 31337 = devnet).
func ChainId() uint64 { return uint64(chainId()) }

// GasRemaining is the fuel left in the current call frame.
func GasRemaining() uint64 { return uint64(txGasRemaining()) }
