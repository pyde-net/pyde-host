//go:build tinygo

// Linear-memory marshalling — the one place `unsafe` is used to bridge
// Go values and the raw host-fn ABI (which speaks only int32 offsets
// into wasm linear memory). Every ergonomic wrapper goes through these
// two helpers so pointer handling lives in exactly one audited spot.
//
// Lifetime: an offset is valid only for the duration of the synchronous
// host call it is passed to. TinyGo's GC is non-moving (no compaction),
// so the address is stable while the host reads/writes, but the offset
// must never be stored and reused across calls. Callers always take the
// address of a live Go value (a stack/heap buffer) and use it inline.

package pyde

import "unsafe"

// ptr returns the linear-memory offset of b's first byte, or 0 for an
// empty slice (paired with a length of 0, which every host fn treats as
// a no-op read/write).
func ptr(b []byte) int32 {
	if len(b) == 0 {
		return 0
	}
	return int32(uintptr(unsafe.Pointer(&b[0])))
}

// ptrOf32 returns the linear-memory offset of an int32 cell. Used for
// the in/out length cells of host fns like calldata_copy, where the
// host reads the max length and writes back the actual length.
func ptrOf32(p *int32) int32 {
	return int32(uintptr(unsafe.Pointer(p)))
}
