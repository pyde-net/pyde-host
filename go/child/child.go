// Package child implements the byte-assembly half of Pyde child-contract
// addressing (pyde::instantiate).
//
//	child_address = Poseidon2("pyde-child:" || parent[32] || template[32] || salt[32])
//
// Go has no Poseidon2 binding, so the hash step happens on-chain: a
// contract assembles the 107-byte preimage with Preimage and hashes it
// via the hash_poseidon2 host import (pyde.HashPoseidon2). What CAN
// drift across the language bindings — and what this package pins — is
// the preimage layout plus the identity-salt SOURCE encodings
// (Salt::of, likewise hashed on-chain: salt = Poseidon2(source borsh
// bytes)). Conformance = the shared golden vectors in
// vectors/child_address.json at the repo root; every implementation
// must reproduce every vector byte-for-byte.
//
// This package has no //go:wasmimport declarations and is plain Go —
// usable from contracts (TinyGo) and off-chain tooling / host `go test`
// alike.
package child

import (
	"bytes"
	"encoding/binary"
)

// DomainTag is the ASCII domain-separation prefix of the child-address
// preimage: 11 bytes, 70 79 64 65 2d 63 68 69 6c 64 3a.
const DomainTag = "pyde-child:"

// PreimageLen is the fixed width of the child-address preimage:
// 11-byte domain tag + 3 × 32 bytes. No length prefixes, no separators.
const PreimageLen = 107

// Preimage assembles the canonical child-address preimage:
//
//	"pyde-child:" || parent[32] || template[32] || salt[32]
//
// 107 bytes fixed-width, no length prefixes, no separators. On-chain
// code hashes this via the hash_poseidon2 host import to obtain the
// child address: child_address = Poseidon2(preimage).
func Preimage(parent, template, salt [32]byte) [PreimageLen]byte {
	var out [PreimageLen]byte
	copy(out[:11], DomainTag)
	copy(out[11:43], parent[:])
	copy(out[43:75], template[:])
	copy(out[75:107], salt[:])
	return out
}

// UnorderedPairEncoding is the Salt::of_unordered_pair source encoding:
// sort the two 32-byte values ascending BYTEWISE (lexicographic), then
// concatenate — raw 64 bytes, no framing. The UniswapV2 sortTokens
// footgun handled once: (a, b) and (b, a) produce identical bytes, so
// the derived salt — and therefore the child address — is
// order-independent.
func UnorderedPairEncoding(a, b [32]byte) [64]byte {
	if bytes.Compare(a[:], b[:]) > 0 {
		a, b = b, a
	}
	var out [64]byte
	copy(out[:32], a[:])
	copy(out[32:], b[:])
	return out
}

// CounterEncoding is the Salt::of source encoding of a u64 identity
// counter: borsh u64, 8-byte little-endian. (One u64, NOT two u32s.)
func CounterEncoding(counter uint64) [8]byte {
	var out [8]byte
	binary.LittleEndian.PutUint64(out[:], counter)
	return out
}
