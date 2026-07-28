//go:build !tinygo

// Decimal parsing/formatting for U128/I128. Gated OFF for TinyGo builds so a
// contract never pulls in math/big (a large wasm-size cost); it's here for
// off-chain tooling, tests, and the golden-vector parity suite. On-chain a
// contract builds 128-bit values from uint64 (U128From) or from bytes
// (U128FromBytesLE) instead.

package pyde

import (
	"fmt"
	"math/big"
)

var (
	bigOne    = big.NewInt(1)
	big2To64  = new(big.Int).Lsh(bigOne, 64)  // 2^64
	big2To128 = new(big.Int).Lsh(bigOne, 128) // 2^128
	big2To127 = new(big.Int).Lsh(bigOne, 127) // 2^127
	bigMaskLo = new(big.Int).Sub(big2To64, bigOne)
)

func u128FromBig(n *big.Int) U128 {
	lo := new(big.Int).And(n, bigMaskLo).Uint64()
	hi := new(big.Int).Rsh(n, 64).Uint64()
	return U128{Lo: lo, Hi: hi}
}

// U128FromString parses an unsigned decimal string into a U128, erroring on a
// bad digit or a value outside [0, 2^128).
func U128FromString(s string) (U128, error) {
	n, ok := new(big.Int).SetString(s, 10)
	if !ok {
		return U128{}, fmt.Errorf("pyde: invalid U128 %q", s)
	}
	if n.Sign() < 0 || n.Cmp(big2To128) >= 0 {
		return U128{}, fmt.Errorf("pyde: U128 out of range %q", s)
	}
	return u128FromBig(n), nil
}

// BigInt returns u as a *big.Int.
func (u U128) BigInt() *big.Int {
	hi := new(big.Int).Lsh(new(big.Int).SetUint64(u.Hi), 64)
	return hi.Or(hi, new(big.Int).SetUint64(u.Lo))
}

// String renders u as an unsigned decimal.
func (u U128) String() string { return u.BigInt().String() }

// I128FromString parses a signed decimal string into an I128, erroring on a bad
// digit or a value outside [-2^127, 2^127).
func I128FromString(s string) (I128, error) {
	n, ok := new(big.Int).SetString(s, 10)
	if !ok {
		return I128{}, fmt.Errorf("pyde: invalid I128 %q", s)
	}
	if n.Cmp(big2To127) >= 0 || n.Cmp(new(big.Int).Neg(big2To127)) < 0 {
		return I128{}, fmt.Errorf("pyde: I128 out of range %q", s)
	}
	m := n
	if n.Sign() < 0 {
		m = new(big.Int).Add(n, big2To128) // wrap negatives to two's complement
	}
	u := u128FromBig(m)
	return I128{Lo: u.Lo, Hi: u.Hi}, nil
}

// BigInt returns i as a signed *big.Int.
func (i I128) BigInt() *big.Int {
	u := U128{Lo: i.Lo, Hi: i.Hi}.BigInt()
	if i.IsNegative() {
		u.Sub(u, big2To128)
	}
	return u
}

// String renders i as a signed decimal.
func (i I128) String() string { return i.BigInt().String() }
