// Borsh v1 encode/decode, byte-identical to Rust `borsh::to_vec`. This is the
// wire format the engine speaks for calldata, return values, storage values,
// and event data — so every byte here must match Rust exactly (proven by the
// golden-vector parity test against the shared vectors/codec_borsh.json).
//
// Rules (HOST_FN_ABI_SPEC §14): integers are fixed-width little-endian,
// two's-complement for signed; bool is 1 byte (0/1); address/hash32/[u8;N] are
// N raw bytes with NO length prefix; bytes and string are u32-LE length prefix
// + payload (string length is UTF-8 byte count); vec<T> is u32-LE count +
// elements back-to-back; structs/tuples are frameless (fields concatenated in
// declaration order, no wrapper). Pure Go — natively testable, TinyGo-clean.

package pyde

// Encoder builds a borsh byte stream. Field methods return the encoder for
// chaining; call Finish to read the result.
type Encoder struct{ buf []byte }

// NewEncoder returns a fresh Encoder.
func NewEncoder() *Encoder { return &Encoder{} }

// Finish returns the accumulated borsh bytes.
func (e *Encoder) Finish() []byte { return e.buf }

// Len reports how many bytes have been written so far.
func (e *Encoder) Len() int { return len(e.buf) }

func (e *Encoder) U8(v uint8) *Encoder { e.buf = append(e.buf, v); return e }

func (e *Encoder) U16(v uint16) *Encoder {
	e.buf = append(e.buf, byte(v), byte(v>>8))
	return e
}

func (e *Encoder) U32(v uint32) *Encoder {
	e.buf = append(e.buf, byte(v), byte(v>>8), byte(v>>16), byte(v>>24))
	return e
}

func (e *Encoder) U64(v uint64) *Encoder {
	e.buf = append(e.buf,
		byte(v), byte(v>>8), byte(v>>16), byte(v>>24),
		byte(v>>32), byte(v>>40), byte(v>>48), byte(v>>56))
	return e
}

func (e *Encoder) U128(v U128) *Encoder {
	b := v.ToBytesLE()
	e.buf = append(e.buf, b[:]...)
	return e
}

// Signed integers are two's-complement — identical bytes to the unsigned bit
// pattern, so we reinterpret and reuse the unsigned writers.
func (e *Encoder) I8(v int8) *Encoder   { return e.U8(uint8(v)) }
func (e *Encoder) I16(v int16) *Encoder { return e.U16(uint16(v)) }
func (e *Encoder) I32(v int32) *Encoder { return e.U32(uint32(v)) }
func (e *Encoder) I64(v int64) *Encoder { return e.U64(uint64(v)) }
func (e *Encoder) I128(v I128) *Encoder {
	b := v.ToBytesLE()
	e.buf = append(e.buf, b[:]...)
	return e
}

func (e *Encoder) Bool(v bool) *Encoder {
	if v {
		e.buf = append(e.buf, 1)
	} else {
		e.buf = append(e.buf, 0)
	}
	return e
}

// Address / Bytes32: 32 raw bytes, no length prefix.
func (e *Encoder) Address(a Address) *Encoder { e.buf = append(e.buf, a[:]...); return e }
func (e *Encoder) Bytes32(h Bytes32) *Encoder { e.buf = append(e.buf, h[:]...); return e }

// Bytes: u32-LE length prefix + raw payload (borsh Vec<u8>).
func (e *Encoder) Bytes(b []byte) *Encoder {
	e.U32(uint32(len(b)))
	e.buf = append(e.buf, b...)
	return e
}

// String: u32-LE UTF-8-byte-length prefix + UTF-8 payload.
func (e *Encoder) String(s string) *Encoder {
	e.U32(uint32(len(s)))
	e.buf = append(e.buf, s...)
	return e
}

// VecU64: u32-LE count + each element as 8-byte LE.
func (e *Encoder) VecU64(vs []uint64) *Encoder {
	e.U32(uint32(len(vs)))
	for _, v := range vs {
		e.U64(v)
	}
	return e
}

// Decoder reads a borsh byte stream with a forward cursor. Every read is
// bounds-checked; an underrun panics, which on-chain routes to `pyde.revert`
// (so malformed/short calldata reverts rather than reading out of bounds).
type Decoder struct {
	buf []byte
	pos int
}

// NewDecoder returns a Decoder over b.
func NewDecoder(b []byte) *Decoder { return &Decoder{buf: b} }

// Remaining reports how many bytes are left unread.
func (d *Decoder) Remaining() int { return len(d.buf) - d.pos }

func (d *Decoder) take(n int) []byte {
	if n < 0 || d.pos+n > len(d.buf) {
		panic("pyde: borsh decode underrun")
	}
	s := d.buf[d.pos : d.pos+n]
	d.pos += n
	return s
}

func (d *Decoder) U8() uint8   { return d.take(1)[0] }
func (d *Decoder) U16() uint16 { return u16LE(d.take(2)) }
func (d *Decoder) U32() uint32 { return u32LE(d.take(4)) }
func (d *Decoder) U64() uint64 { return u64LE(d.take(8)) }
func (d *Decoder) U128() U128  { return U128FromBytesLE(d.take(16)) }
func (d *Decoder) I8() int8    { return int8(d.take(1)[0]) }
func (d *Decoder) I16() int16  { return int16(u16LE(d.take(2))) }
func (d *Decoder) I32() int32  { return int32(u32LE(d.take(4))) }
func (d *Decoder) I64() int64  { return int64(u64LE(d.take(8))) }
func (d *Decoder) I128() I128  { return I128FromBytesLE(d.take(16)) }

// Bool decodes a strict borsh bool: only 0x00/0x01 are valid (matching Rust,
// which rejects other bytes).
func (d *Decoder) Bool() bool {
	b := d.take(1)[0]
	if b > 1 {
		panic("pyde: borsh invalid bool")
	}
	return b == 1
}

func (d *Decoder) Address() Address { return AddressFromBytes(d.take(AddressLen)) }
func (d *Decoder) Bytes32() Bytes32 { return Bytes32FromBytes(d.take(Bytes32Len)) }

// Bytes decodes a length-prefixed byte string. The result aliases the decoder
// buffer (read-only calldata); copy if you need to retain it past the frame.
func (d *Decoder) Bytes() []byte { return d.take(int(d.U32())) }

// String decodes a length-prefixed UTF-8 string (the bytes are copied).
func (d *Decoder) String() string { return string(d.take(int(d.U32()))) }

// VecU64 decodes a count-prefixed slice of u64.
func (d *Decoder) VecU64() []uint64 {
	n := int(d.U32())
	out := make([]uint64, n)
	for i := range out {
		out[i] = d.U64()
	}
	return out
}
