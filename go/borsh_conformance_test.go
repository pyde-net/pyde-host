package pyde

// Structural conformance for the borsh codec — the invariants the golden
// vectors don't exercise: frameless struct composition, the Option tag / enum
// discriminant layout, and the Decoder's safety guarantees (underrun and
// strict-bool both revert). These assert PROPERTIES rather than hand-authored
// expected bytes: the byte-level truth is owned by the golden parity test, and
// fabricating expected bytes here would just re-encode any mistake instead of
// catching it.

import (
	"bytes"
	"testing"
)

// A borsh struct is frameless: encoding fields into one stream must be
// byte-identical to concatenating each field's independent encoding. This is
// exactly what the Phase-4 codegen relies on when it emits one field-encode per
// declared field, in declaration order, with no wrapper.
func TestFramelessStructComposition(t *testing.T) {
	// struct { a u32; b bool; c string; d Address }
	addr := AddressFromBytes(bytes.Repeat([]byte{0xAB}, 32))

	combined := NewEncoder().U32(1).Bool(true).String("hi").Address(addr).Finish()

	var concat []byte
	concat = append(concat, NewEncoder().U32(1).Finish()...)
	concat = append(concat, NewEncoder().Bool(true).Finish()...)
	concat = append(concat, NewEncoder().String("hi").Finish()...)
	concat = append(concat, NewEncoder().Address(addr).Finish()...)

	if !bytes.Equal(combined, concat) {
		t.Fatalf("struct not frameless:\n combined: %x\n concat:   %x", combined, concat)
	}

	// And it decodes back field-by-field with nothing left over.
	d := NewDecoder(combined)
	if d.U32() != 1 || d.Bool() != true || d.String() != "hi" || !d.Address().Equal(addr) {
		t.Fatal("struct round-trip mismatch")
	}
	if d.Remaining() != 0 {
		t.Fatalf("struct decode left %d bytes", d.Remaining())
	}
}

// Option<T> is a 1-byte presence tag (0=None, 1=Some) followed by T only when
// present. Enum is a 1-byte discriminant followed by the selected variant's
// fields. Both compose over the primitive writers — assert the layout shape.
func TestOptionAndEnumLayout(t *testing.T) {
	// Option::<u32>::None  -> [0x00]
	none := NewEncoder().U8(0).Finish()
	if len(none) != 1 || none[0] != 0 {
		t.Fatalf("Option None = %x, want 00", none)
	}
	// Option::<u32>::Some(7) -> [0x01, <u32 LE 7>]
	some := NewEncoder().U8(1).U32(7).Finish()
	if len(some) != 5 || some[0] != 1 {
		t.Fatalf("Option Some tag/len wrong: %x", some)
	}
	if got := NewEncoder().U32(7).Finish(); !bytes.Equal(some[1:], got) {
		t.Fatalf("Option Some payload %x != bare u32 %x", some[1:], got)
	}

	// enum E { A, B(u64), C } value B(42) -> [0x01, <u64 LE 42>]
	variant := NewEncoder().U8(1).U64(42).Finish()
	d := NewDecoder(variant)
	if d.U8() != 1 || d.U64() != 42 || d.Remaining() != 0 {
		t.Fatalf("enum B(42) round-trip failed: %x", variant)
	}
}

// A short/truncated buffer must panic (which on-chain routes to pyde.revert),
// never read out of bounds or return garbage.
func TestDecodeUnderrunPanics(t *testing.T) {
	cases := map[string]func(){
		"u32 from 3 bytes":   func() { NewDecoder([]byte{1, 2, 3}).U32() },
		"u64 from 0 bytes":   func() { NewDecoder(nil).U64() },
		"u128 from 15 bytes": func() { NewDecoder(make([]byte, 15)).U128() },
		"address from 31":    func() { NewDecoder(make([]byte, 31)).Address() },
		"bytes past len":     func() { NewDecoder([]byte{9, 0, 0, 0, 1}).Bytes() },        // len=9, only 1 byte
		"string past len":    func() { _ = NewDecoder([]byte{4, 0, 0, 0}).String() },      // len=4, 0 payload
		"vec count past buf": func() { NewDecoder([]byte{2, 0, 0, 0, 1, 2, 3}).VecU64() }, // 2 elems, 3 bytes
	}
	for name, fn := range cases {
		t.Run(name, func(t *testing.T) {
			defer func() {
				if recover() == nil {
					t.Fatalf("%s: expected panic, got none", name)
				}
			}()
			fn()
		})
	}
}

// Strict bool: only 0x00/0x01 decode; anything else reverts (matching Rust,
// which rejects non-canonical bools).
func TestStrictBoolRejects(t *testing.T) {
	if NewDecoder([]byte{0}).Bool() != false || NewDecoder([]byte{1}).Bool() != true {
		t.Fatal("canonical bool bytes mis-decoded")
	}
	for _, b := range []byte{2, 0x7f, 0xff} {
		func() {
			defer func() {
				if recover() == nil {
					t.Fatalf("bool byte 0x%02x: expected panic", b)
				}
			}()
			NewDecoder([]byte{b}).Bool()
		}()
	}
}

// Every primitive must survive an encode→decode→re-encode round-trip with no
// residue, over boundary values (min/max/zero) that stress sign and width.
func TestPrimitiveRoundTrip(t *testing.T) {
	checkReenc := func(name string, enc []byte, reenc func(*Decoder) []byte) {
		d := NewDecoder(enc)
		out := reenc(d)
		if !bytes.Equal(out, enc) {
			t.Fatalf("%s: reencode %x != %x", name, out, enc)
		}
		if d.Remaining() != 0 {
			t.Fatalf("%s: %d bytes left", name, d.Remaining())
		}
	}
	checkReenc("u128 max", NewEncoder().U128(U128{Lo: ^uint64(0), Hi: ^uint64(0)}).Finish(),
		func(d *Decoder) []byte { return NewEncoder().U128(d.U128()).Finish() })
	checkReenc("i128 min", NewEncoder().I128(I128{Lo: 0, Hi: 1 << 63}).Finish(),
		func(d *Decoder) []byte { return NewEncoder().I128(d.I128()).Finish() })
	checkReenc("i64 min", NewEncoder().I64(-1<<63).Finish(),
		func(d *Decoder) []byte { return NewEncoder().I64(d.I64()).Finish() })
	checkReenc("empty bytes", NewEncoder().Bytes(nil).Finish(),
		func(d *Decoder) []byte { return NewEncoder().Bytes(d.Bytes()).Finish() })
	checkReenc("empty string", NewEncoder().String("").Finish(),
		func(d *Decoder) []byte { return NewEncoder().String(d.String()).Finish() })
	checkReenc("empty vec", NewEncoder().VecU64(nil).Finish(),
		func(d *Decoder) []byte { return NewEncoder().VecU64(d.VecU64()).Finish() })
}
