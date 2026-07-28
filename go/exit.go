//go:build tinygo

// Halt operators — the two ways a contract call ends: Return (success,
// with return data) and Revert (failure, with a reason). Both are
// semantically noreturn: once the host fn is called the engine unwinds
// the wasm frame and execution stops. TinyGo can't express noreturn in
// an import signature, so each wrapper ends in `for {}` — an infinite
// loop is a terminating statement in Go, so no function epilogue is
// emitted after the host call (which is what avoids a post-return trap;
// see the counter-go reference).
//
// Return data is the borsh encoding of the function's declared return
// type. The typed helpers (ReturnU64, …) do that encoding; Return sets
// the bytes verbatim (used by the generated entry dispatch, which has
// already borsh-encoded the result).

package pyde

// Return ends the call successfully, setting return data to the given
// bytes verbatim. The bytes must be the borsh encoding of the declared
// return type. Never returns.
func Return(data []byte) {
	pydeReturn(ptr(data), int32(len(data)))
	for { // unreachable — the engine unwinds the frame at pydeReturn
	}
}

// ReturnNothing ends the call successfully with empty return data (for
// functions declared with no return type).
func ReturnNothing() { Return(nil) }

// ReturnU64 returns a borsh-encoded u64.
func ReturnU64(v uint64) { Return(NewEncoder().U64(v).Finish()) }

// ReturnU128 returns a borsh-encoded u128.
func ReturnU128(v U128) { Return(NewEncoder().U128(v).Finish()) }

// ReturnBool returns a borsh-encoded bool.
func ReturnBool(v bool) { Return(NewEncoder().Bool(v).Finish()) }

// ReturnAddress returns a borsh-encoded address (32 raw bytes).
func ReturnAddress(a Address) { Return(NewEncoder().Address(a).Finish()) }

// ReturnString returns a borsh-encoded string (u32 length prefix + UTF-8).
func ReturnString(s string) { Return(NewEncoder().String(s).Finish()) }

// ReturnBytes returns a borsh-encoded byte string (u32 length prefix +
// payload). For raw, unframed return data use Return.
func ReturnBytes(b []byte) { Return(NewEncoder().Bytes(b).Finish()) }

// Revert aborts the current call frame, discarding all state changes
// since it began. The reason surfaces as the failure payload to the
// caller (or the transaction receipt if top-level). Never returns.
//
// Go has no noreturn, so callers in a value-returning function still
// need a trailing `return` the compiler can see (it is never reached):
//
//	func balanceOf(a Address) U128 {
//		if a.IsZero() { Revert("zero address"); return U128{} }
//		...
//	}
func Revert(reason string) {
	revert(ptr([]byte(reason)), int32(len(reason)))
	for { // unreachable — the engine unwinds the frame at revert
	}
}

// Require reverts with reason unless cond holds. A terse guard for the
// common precondition pattern.
func Require(cond bool, reason string) {
	if !cond {
		Revert(reason)
	}
}
