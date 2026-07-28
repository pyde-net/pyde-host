//go:build tinygo

// Account & balance (§7.2) — native-PYDE balance reads and transfers. Amounts
// are U128 quanta.

package pyde

// Balance returns addr's native-PYDE balance, in quanta.
func Balance(addr Address) U128 {
	var b [16]byte
	balance(ptr(addr[:]), ptr(b[:]))
	return U128FromBytesLE(b[:])
}

// Transfer sends amount quanta of native PYDE from this contract to `to`,
// reverting on insufficient balance. A zero amount and a self-transfer are
// both no-op successes. Transfer from a view function traps.
func Transfer(to Address, amount U128) {
	b := amount.ToBytesLE()
	if transfer(ptr(to[:]), ptr(b[:])) != StatusOK {
		Revert("pyde: transfer failed (insufficient balance)")
	}
}

// TryTransfer is like Transfer but returns false on insufficient balance
// instead of reverting, letting the caller handle the shortfall.
func TryTransfer(to Address, amount U128) bool {
	b := amount.ToBytesLE()
	return transfer(ptr(to[:]), ptr(b[:])) == StatusOK
}
