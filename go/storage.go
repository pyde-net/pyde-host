//go:build tinygo

// Typed storage (§7.1) — the schema-derived slot family. The host derives
// the slot as Poseidon2(self_address || field_name || keys...) and validates
// the value's byte length against the field's declared type, so a contract
// never computes a Poseidon2 slot itself and can't write to a slot derived
// from another contract's address.
//
// These are the generic byte-level primitives: the caller passes the field
// name, borsh-encoded key(s), and a borsh-encoded value. The Phase-4 codegen
// layers typed, named accessors (State.Balances.Get(addr)) on top of them.
//
// Value/key bytes MUST be the borsh encoding of the field's declared type
// (build them with an Encoder); a length/type mismatch makes the host return
// a negative code, which these wrappers surface as a revert. A store from a
// view-attributed function traps (ERR_FORBIDDEN) in the engine.

package pyde

// storeErr reverts with a schema/type-mismatch message when a typed-storage
// write returns a negative code (all of which are contract/schema bugs).
func storeErr(op string, rc int32) {
	if rc != StatusOK {
		Revert("pyde: " + op + " rejected by state schema")
	}
}

// probeThenRead reads a variable-length typed-storage value using the
// length-probe convention (out_max=0 returns the actual length without
// copying), then a sized read. Returns (value, true) or (nil, false) when
// the slot was never written.
func probeThenRead(read func(outPtr, outMax int32) int32) ([]byte, bool) {
	n := read(0, 0)
	if n < 0 { // SloadMissing
		return nil, false
	}
	if n == 0 {
		return []byte{}, true
	}
	buf := make([]byte, n)
	got := read(ptr(buf), n)
	if got < 0 {
		return nil, false
	}
	if got > n {
		got = n
	}
	return buf[:got], true
}

// ── scalar ───────────────────────────────────────────────────────────

// StoreScalar writes a borsh-encoded value to a declared scalar field.
func StoreScalar(field string, value []byte) {
	fb := []byte(field)
	storeErr("sstore_scalar", sstoreScalar(ptr(fb), int32(len(fb)), ptr(value), int32(len(value))))
}

// LoadScalar reads a scalar field. Returns (value, true), or (nil, false)
// if it was never written.
func LoadScalar(field string) ([]byte, bool) {
	fb := []byte(field)
	return probeThenRead(func(outPtr, outMax int32) int32 {
		return sloadScalar(ptr(fb), int32(len(fb)), outPtr, outMax)
	})
}

// DeleteScalar clears a scalar field.
func DeleteScalar(field string) {
	fb := []byte(field)
	sdeleteScalar(ptr(fb), int32(len(fb)))
}

// ── map1 ─────────────────────────────────────────────────────────────

// StoreMap1 writes value at a 1-key map field. key is the borsh-encoded key.
func StoreMap1(field string, key, value []byte) {
	fb := []byte(field)
	storeErr("sstore_map1", sstoreMap1(ptr(fb), int32(len(fb)), ptr(key), int32(len(key)), ptr(value), int32(len(value))))
}

// LoadMap1 reads a 1-key map entry. Returns (value, true) or (nil, false).
func LoadMap1(field string, key []byte) ([]byte, bool) {
	fb := []byte(field)
	return probeThenRead(func(outPtr, outMax int32) int32 {
		return sloadMap1(ptr(fb), int32(len(fb)), ptr(key), int32(len(key)), outPtr, outMax)
	})
}

// DeleteMap1 clears a 1-key map entry.
func DeleteMap1(field string, key []byte) {
	fb := []byte(field)
	sdeleteMap1(ptr(fb), int32(len(fb)), ptr(key), int32(len(key)))
}

// ── map2 ─────────────────────────────────────────────────────────────

// StoreMap2 writes value at a 2-key map field.
func StoreMap2(field string, k1, k2, value []byte) {
	fb := []byte(field)
	storeErr("sstore_map2", sstoreMap2(ptr(fb), int32(len(fb)), ptr(k1), int32(len(k1)), ptr(k2), int32(len(k2)), ptr(value), int32(len(value))))
}

// LoadMap2 reads a 2-key map entry.
func LoadMap2(field string, k1, k2 []byte) ([]byte, bool) {
	fb := []byte(field)
	return probeThenRead(func(outPtr, outMax int32) int32 {
		return sloadMap2(ptr(fb), int32(len(fb)), ptr(k1), int32(len(k1)), ptr(k2), int32(len(k2)), outPtr, outMax)
	})
}

// DeleteMap2 clears a 2-key map entry.
func DeleteMap2(field string, k1, k2 []byte) {
	fb := []byte(field)
	sdeleteMap2(ptr(fb), int32(len(fb)), ptr(k1), int32(len(k1)), ptr(k2), int32(len(k2)))
}

// ── map3 ─────────────────────────────────────────────────────────────

// StoreMap3 writes value at a 3-key map field.
func StoreMap3(field string, k1, k2, k3, value []byte) {
	fb := []byte(field)
	storeErr("sstore_map3", sstoreMap3(ptr(fb), int32(len(fb)), ptr(k1), int32(len(k1)), ptr(k2), int32(len(k2)), ptr(k3), int32(len(k3)), ptr(value), int32(len(value))))
}

// LoadMap3 reads a 3-key map entry.
func LoadMap3(field string, k1, k2, k3 []byte) ([]byte, bool) {
	fb := []byte(field)
	return probeThenRead(func(outPtr, outMax int32) int32 {
		return sloadMap3(ptr(fb), int32(len(fb)), ptr(k1), int32(len(k1)), ptr(k2), int32(len(k2)), ptr(k3), int32(len(k3)), outPtr, outMax)
	})
}

// DeleteMap3 clears a 3-key map entry.
func DeleteMap3(field string, k1, k2, k3 []byte) {
	fb := []byte(field)
	sdeleteMap3(ptr(fb), int32(len(fb)), ptr(k1), int32(len(k1)), ptr(k2), int32(len(k2)), ptr(k3), int32(len(k3)))
}
