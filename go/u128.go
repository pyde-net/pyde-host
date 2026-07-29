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

// Mul returns u*other, wrapping mod 2^128 (matching Rust's u128 wrapping_mul).
// The a1*b1 term lands entirely at 2^128 and vanishes under the modulus.
func (u U128) Mul(other U128) U128 {
	hi, lo := bits.Mul64(u.Lo, other.Lo)
	hi += u.Lo*other.Hi + u.Hi*other.Lo
	return U128{Lo: lo, Hi: hi}
}

// MulChecked returns u*other and false if the product overflowed 128 bits.
// It forms the full 256-bit product in four 64-bit limbs (r3:r2:r1:r0) and
// reports overflow when either high limb is non-zero.
func (u U128) MulChecked(other U128) (U128, bool) {
	p00h, p00l := bits.Mul64(u.Lo, other.Lo)
	p01h, p01l := bits.Mul64(u.Lo, other.Hi)
	p10h, p10l := bits.Mul64(u.Hi, other.Lo)
	p11h, p11l := bits.Mul64(u.Hi, other.Hi)

	r0 := p00l
	// r1 = p00h + p01l + p10l, capturing carries into r2.
	r1, c1 := bits.Add64(p00h, p01l, 0)
	r1, c2 := bits.Add64(r1, p10l, 0)
	// r2 = p01h + p10h + p11l + (c1+c2), capturing carries into r3.
	r2, c3 := bits.Add64(p01h, p10h, 0)
	r2, c4 := bits.Add64(r2, p11l, 0)
	r2, c5 := bits.Add64(r2, c1+c2, 0)
	// r3 = p11h + (c3+c4+c5).
	r3 := p11h + c3 + c4 + c5

	return U128{Lo: r0, Hi: r1}, (r2 | r3) == 0
}

// shl1 returns u<<1.
func (u U128) shl1() U128 {
	return U128{Lo: u.Lo << 1, Hi: (u.Hi << 1) | (u.Lo >> 63)}
}

// bit returns bit i (0 = LSB, 127 = MSB) as 0 or 1.
func (u U128) bit(i int) uint64 {
	if i < 64 {
		return (u.Lo >> uint(i)) & 1
	}
	return (u.Hi >> uint(i-64)) & 1
}

// DivMod returns (u/other, u%other), truncating toward zero. It panics on a
// zero divisor (a divide-by-zero in a contract reverts the frame).
func (u U128) DivMod(other U128) (U128, U128) {
	if other.IsZero() {
		panic("pyde: U128 division by zero")
	}
	if u.Cmp(other) < 0 {
		return U128{}, u
	}
	// Fast path: a 64-bit divisor divides limb-by-limb via math/bits.
	if other.Hi == 0 {
		qHi, rem := bits.Div64(0, u.Hi, other.Lo)
		qLo, rem2 := bits.Div64(rem, u.Lo, other.Lo)
		return U128{Lo: qLo, Hi: qHi}, U128{Lo: rem2}
	}
	// General path: schoolbook binary long division, MSB to LSB.
	var q, rem U128
	for i := 127; i >= 0; i-- {
		rem = rem.shl1()
		rem.Lo |= u.bit(i)
		if rem.Cmp(other) >= 0 {
			rem = rem.Sub(other)
			if i < 64 {
				q.Lo |= uint64(1) << uint(i)
			} else {
				q.Hi |= uint64(1) << uint(i-64)
			}
		}
	}
	return q, rem
}

// Div returns u/other, truncating toward zero. Panics on a zero divisor.
func (u U128) Div(other U128) U128 { q, _ := u.DivMod(other); return q }

// Mod returns u%other. Panics on a zero divisor.
func (u U128) Mod(other U128) U128 { _, r := u.DivMod(other); return r }
