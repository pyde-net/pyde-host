//go:build tinygo

// Hashing primitives (§7.6). Each takes a byte slice and returns a
// 32-byte digest as a value — no output buffer to manage.

package pyde

// Blake3 hashes input with Blake3. General-purpose: address derivation,
// event topic-0, content addressing. Cheapest of the three.
func Blake3(input []byte) Bytes32 {
	var out Bytes32
	hashBlake3(ptr(input), int32(len(input)), ptr(out[:]))
	return out
}

// Poseidon2 hashes input with Poseidon2. ZK-friendly but more expensive
// than Blake3 in native execution; used for storage-slot derivation and
// state-root commitments. Contracts rarely need it directly — typed
// storage derives slots host-side.
func Poseidon2(input []byte) Bytes32 {
	var out Bytes32
	hashPoseidon2(ptr(input), int32(len(input)), ptr(out[:]))
	return out
}

// Keccak256 hashes input with Keccak256. Provided for cross-chain
// interop (e.g. verifying Ethereum proofs); Pyde does not use Keccak
// natively.
func Keccak256(input []byte) Bytes32 {
	var out Bytes32
	hashKeccak256(ptr(input), int32(len(input)), ptr(out[:]))
	return out
}
