// Little-endian integer codec — the byte-level substrate every higher layer
// (borsh, u128, storage) composes over. Pure Go, no host calls: compiles and
// tests natively and under TinyGo.
//
// Borsh integers are fixed-width, little-endian, two's-complement for signed
// (HOST_FN_ABI_SPEC §14). We name these explicitly rather than reaching for
// encoding/binary so the wire format is owned in one place and stays TinyGo-
// lean (no reflection, no interface dispatch).

package pyde

func putU8LE(b []byte, v uint8)   { b[0] = v }
func putU16LE(b []byte, v uint16) { b[0] = byte(v); b[1] = byte(v >> 8) }
func putU32LE(b []byte, v uint32) {
	b[0] = byte(v)
	b[1] = byte(v >> 8)
	b[2] = byte(v >> 16)
	b[3] = byte(v >> 24)
}
func putU64LE(b []byte, v uint64) {
	b[0] = byte(v)
	b[1] = byte(v >> 8)
	b[2] = byte(v >> 16)
	b[3] = byte(v >> 24)
	b[4] = byte(v >> 32)
	b[5] = byte(v >> 40)
	b[6] = byte(v >> 48)
	b[7] = byte(v >> 56)
}

func u8LE(b []byte) uint8   { return b[0] }
func u16LE(b []byte) uint16 { return uint16(b[0]) | uint16(b[1])<<8 }
func u32LE(b []byte) uint32 {
	return uint32(b[0]) | uint32(b[1])<<8 | uint32(b[2])<<16 | uint32(b[3])<<24
}
func u64LE(b []byte) uint64 {
	return uint64(b[0]) | uint64(b[1])<<8 | uint64(b[2])<<16 | uint64(b[3])<<24 |
		uint64(b[4])<<32 | uint64(b[5])<<40 | uint64(b[6])<<48 | uint64(b[7])<<56
}
