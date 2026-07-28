//go:build tinygo

// Calldata access (§7.4). The engine hands a contract its argument blob
// through calldata_size + calldata_copy (an in/out length convention on
// a single buffer). These wrappers hide that dance: Calldata returns the
// bytes, Args wraps them in a borsh Decoder for reading typed arguments
// in declaration order.

package pyde

// Calldata returns the current invocation's raw argument bytes (the
// borsh-encoded argument tuple). Empty for a no-argument call.
func Calldata() []byte {
	n := calldataSize()
	if n <= 0 {
		return nil
	}
	buf := make([]byte, n)
	// The length cell carries the max we accept in, and the actual
	// bytes copied out (never more than n).
	actual := n
	calldataCopy(ptr(buf), ptrOf32(&actual))
	if actual < 0 {
		actual = 0
	}
	if actual > n {
		actual = n
	}
	return buf[:actual]
}

// Args returns a borsh Decoder over the calldata, for pulling typed
// arguments in the order the function declares them:
//
//	a := pyde.Args()
//	to := a.Address()
//	amount := a.U128()
//
// A short or malformed calldata fails the call: the Decoder panics on
// underrun, which traps (the transaction fails and state is discarded)
// but surfaces no reason payload. Validate and call Revert when you
// want a caller-visible message.
func Args() *Decoder {
	return NewDecoder(Calldata())
}
