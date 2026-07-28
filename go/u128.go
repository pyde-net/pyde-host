// 128-bit integers. Go has no native u128/i128, so these are two-limb structs
// (lo/hi u64) with the canonical 16-byte little-endian, two's-complement wire
// layout the engine and Rust `borsh` use. Pure Go (math/bits only — no
// math/big, so contracts stay small): compiles natively and under TinyGo.
//
// Decimal parsing/formatting (FromString/String) lives in u128_decimal.go,
// gated to non-TinyGo builds so contracts don't pull in math/big.

package pyde

import "math/bits"

const U128Len = 16

// U128 is an unsigned 128-bit integer. Lo holds the low 64 bits, Hi the high 64.
type U128 struct{ Lo, Hi uint64 }

// I128 is a signed 128-bit integer held as two's-complement bits (Hi's MSB is
// the sign). Its wire layout is identical to U128; only interpretation differs.
type I128 struct{ Lo, Hi uint64 }

// U128From builds a U128 from a uint64 (Hi = 0).
func U128From(v uint64) U128 { return U128{Lo: v} }

// I128From builds an I128 from an int64, sign-extending into Hi.
func I128From(v int64) I128 {
	hi := uint64(0)
	if v < 0 {
		hi = ^uint64(0) // all ones (sign extension)
	}
	return I128{Lo: uint64(v), Hi: hi}
}

// ToBytesLE returns the 16-byte little-endian encoding: low limb then high limb.
func (u U128) ToBytesLE() [16]byte {
	var b [16]byte
	putU64LE(b[0:8], u.Lo)
	putU64LE(b[8:16], u.Hi)
	return b
}

// ToBytesLE returns the 16-byte little-endian two's-complement encoding.
func (i I128) ToBytesLE() [16]byte {
	var b [16]byte
	putU64LE(b[0:8], i.Lo)
	putU64LE(b[8:16], i.Hi)
	return b
}

// U128FromBytesLE decodes a 16-byte little-endian value. It panics if b is
// shorter than 16 bytes (a truncated decode reverts).
func U128FromBytesLE(b []byte) U128 {
	if len(b) < U128Len {
		panic("pyde: U128 needs 16 bytes")
	}
	return U128{Lo: u64LE(b[0:8]), Hi: u64LE(b[8:16])}
}

// I128FromBytesLE decodes a 16-byte little-endian two's-complement value.
func I128FromBytesLE(b []byte) I128 {
	if len(b) < U128Len {
		panic("pyde: I128 needs 16 bytes")
	}
	return I128{Lo: u64LE(b[0:8]), Hi: u64LE(b[8:16])}
}

// IsZero reports whether u == 0.
func (u U128) IsZero() bool { return u.Lo == 0 && u.Hi == 0 }

// IsZero reports whether i == 0.
func (i I128) IsZero() bool { return i.Lo == 0 && i.Hi == 0 }

// IsNegative reports whether i < 0 (high-limb sign bit set).
func (i I128) IsNegative() bool { return i.Hi&(1<<63) != 0 }

// Cmp compares u and other, returning -1, 0, or +1 (unsigned).
func (u U128) Cmp(other U128) int {
	if u.Hi != other.Hi {
		if u.Hi < other.Hi {
			return -1
		}
		return 1
	}
	if u.Lo != other.Lo {
		if u.Lo < other.Lo {
			return -1
		}
		return 1
	}
	return 0
}

// Cmp compares i and other, returning -1, 0, or +1 (signed).
func (i I128) Cmp(other I128) int {
	in, on := i.IsNegative(), other.IsNegative()
	if in != on {
		if in {
			return -1
		}
		return 1
	}
	// Same sign: two's-complement bits order the same as the values.
	return U128{i.Lo, i.Hi}.Cmp(U128{other.Lo, other.Hi})
}

// Eq reports whether u == other.
func (u U128) Eq(other U128) bool { return u == other }

// Lt reports whether u < other (unsigned).
func (u U128) Lt(other U128) bool { return u.Cmp(other) < 0 }

// Gt reports whether u > other (unsigned).
func (u U128) Gt(other U128) bool { return u.Cmp(other) > 0 }

// Lte reports whether u <= other (unsigned).
func (u U128) Lte(other U128) bool { return u.Cmp(other) <= 0 }

// Gte reports whether u >= other (unsigned).
func (u U128) Gte(other U128) bool { return u.Cmp(other) >= 0 }

// Add returns u+other, wrapping on overflow (matching Rust's u128 wrapping_add).
func (u U128) Add(other U128) U128 {
	lo, carry := bits.Add64(u.Lo, other.Lo, 0)
	hi, _ := bits.Add64(u.Hi, other.Hi, carry)
	return U128{Lo: lo, Hi: hi}
}

// Sub returns u-other, wrapping on underflow.
func (u U128) Sub(other U128) U128 {
	lo, borrow := bits.Sub64(u.Lo, other.Lo, 0)
	hi, _ := bits.Sub64(u.Hi, other.Hi, borrow)
	return U128{Lo: lo, Hi: hi}
}

// AddChecked returns u+other and false if it overflowed 128 bits.
func (u U128) AddChecked(other U128) (U128, bool) {
	lo, carry := bits.Add64(u.Lo, other.Lo, 0)
	hi, carry2 := bits.Add64(u.Hi, other.Hi, carry)
	return U128{Lo: lo, Hi: hi}, carry2 == 0
}

// SubChecked returns u-other and false if it underflowed below zero.
func (u U128) SubChecked(other U128) (U128, bool) {
	lo, borrow := bits.Sub64(u.Lo, other.Lo, 0)
	hi, borrow2 := bits.Sub64(u.Hi, other.Hi, borrow)
	return U128{Lo: lo, Hi: hi}, borrow2 == 0
}
