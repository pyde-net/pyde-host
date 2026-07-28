// Fixed 32-byte value types shared across the SDK. These are pure data —
// no host calls — so they compile and test natively as well as under TinyGo.
//
// Borsh encodes them as `[u8; 32]`: 32 raw bytes, NO length prefix (unlike
// `bytes`/`string`). See HOST_FN_ABI_SPEC §14.

package pyde

// Byte lengths of the fixed value types.
const (
	AddressLen = 32
	Bytes32Len = 32
)

// Address is a 32-byte account address (contracts, EOAs). Comparable with `==`.
type Address [32]byte

// Bytes32 is a 32-byte fixed value — hashes, salts, event topics. Comparable
// with `==`.
type Bytes32 [32]byte

// AddressFromBytes copies the first 32 bytes of b into an Address. It panics if
// b is shorter than 32 bytes, so a truncated decode reverts rather than reading
// out of bounds.
func AddressFromBytes(b []byte) Address {
	var a Address
	if len(b) < AddressLen {
		panic("pyde: Address needs 32 bytes")
	}
	copy(a[:], b)
	return a
}

// Bytes32FromBytes copies the first 32 bytes of b into a Bytes32. It panics if
// b is shorter than 32 bytes.
func Bytes32FromBytes(b []byte) Bytes32 {
	var h Bytes32
	if len(b) < Bytes32Len {
		panic("pyde: Bytes32 needs 32 bytes")
	}
	copy(h[:], b)
	return h
}

// Bytes returns the address as a 32-byte slice (aliasing the array).
func (a Address) Bytes() []byte { return a[:] }

// Bytes returns the value as a 32-byte slice (aliasing the array).
func (h Bytes32) Bytes() []byte { return h[:] }

// Equal reports whether a and b are the same address.
func (a Address) Equal(b Address) bool { return a == b }

// Equal reports whether h and b are the same 32-byte value.
func (h Bytes32) Equal(b Bytes32) bool { return h == b }

// IsZero reports whether the address is all-zero (the null address).
func (a Address) IsZero() bool { return a == Address{} }

// IsZero reports whether the value is all-zero.
func (h Bytes32) IsZero() bool { return h == Bytes32{} }
