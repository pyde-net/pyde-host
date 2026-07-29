//go:build !tinygo

package pyde

import (
	"math/big"
	"math/rand"
	"testing"
)

var mod128 = new(big.Int).Lsh(big.NewInt(1), 128)

func u128ToBig(u U128) *big.Int {
	b := new(big.Int).SetUint64(u.Hi)
	b.Lsh(b, 64)
	return b.Or(b, new(big.Int).SetUint64(u.Lo))
}

func bigToU128(b *big.Int) U128 {
	m := new(big.Int).Mod(b, mod128)
	lo := new(big.Int).And(m, new(big.Int).SetUint64(^uint64(0)))
	hi := new(big.Int).Rsh(m, 64)
	return U128{Lo: lo.Uint64(), Hi: hi.Uint64()}
}

// TestU128Arith cross-checks Mul / MulChecked / DivMod / Div / Mod against
// math/big (ground truth) over edge cases plus a deterministic pseudo-random
// set. math/big is the reference only — the SDK itself never imports it.
func TestU128Arith(t *testing.T) {
	max64 := ^uint64(0)
	vals := []U128{
		{0, 0}, {1, 0}, {2, 0}, {0, 1}, {1, 1},
		{max64, 0}, {0, max64}, {max64, max64},
		{max64, 1}, {1, max64}, {max64 - 1, max64},
		{0x8000000000000000, 0}, {0, 0x8000000000000000},
		{12345678901234567, 98765432109876543},
	}
	r := rand.New(rand.NewSource(1))
	for i := 0; i < 400; i++ {
		vals = append(vals, U128{Lo: r.Uint64(), Hi: r.Uint64()})
		vals = append(vals, U128{Lo: r.Uint64(), Hi: r.Uint64() >> 40}) // smaller Hi → 64-bit divisor path
	}

	for _, a := range vals {
		A := u128ToBig(a)
		for _, b := range vals {
			B := u128ToBig(b)

			if got, want := a.Mul(b), bigToU128(new(big.Int).Mul(A, B)); got != want {
				t.Fatalf("Mul(%v,%v)=%v want %v", a, b, got, want)
			}

			prod := new(big.Int).Mul(A, B)
			gotMC, ok := a.MulChecked(b)
			if wantOK := prod.Cmp(mod128) < 0; ok != wantOK {
				t.Fatalf("MulChecked(%v,%v) ok=%v want %v", a, b, ok, wantOK)
			} else if ok && gotMC != bigToU128(prod) {
				t.Fatalf("MulChecked(%v,%v)=%v want %v", a, b, gotMC, bigToU128(prod))
			}

			if !b.IsZero() {
				q, rem := a.DivMod(b)
				wantQ := bigToU128(new(big.Int).Div(A, B))
				wantR := bigToU128(new(big.Int).Mod(A, B))
				if q != wantQ || rem != wantR {
					t.Fatalf("DivMod(%v,%v)=(%v,%v) want (%v,%v)", a, b, q, rem, wantQ, wantR)
				}
				if a.Div(b) != wantQ || a.Mod(b) != wantR {
					t.Fatalf("Div/Mod(%v,%v) mismatch", a, b)
				}
			}
		}
	}
}

func TestU128DivByZeroPanics(t *testing.T) {
	defer func() {
		if recover() == nil {
			t.Fatal("expected panic dividing by zero")
		}
	}()
	U128{Lo: 5}.Div(U128{})
}
