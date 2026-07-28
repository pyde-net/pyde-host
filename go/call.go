//go:build tinygo

// Cross-contract calls (§7.8) — a small builder over cross_call /
// cross_call_static / delegate_call. A sub-call that reverts returns a
// negative status code (recoverable); only critical failures (call depth,
// code-cache miss) trap the whole transaction.

package pyde

const (
	// forwardAllGas is the default gas budget for cross-calls. The engine
	// forwards min(gas, parent_remaining), so a value far above any real
	// budget means "forward all remaining".
	forwardAllGas int64 = 1 << 62
	// maxReturnBytes caps return-data copied back from a sub-call (symmetric
	// with the 16 KiB calldata cap). Exec reverts rather than truncating.
	maxReturnBytes = 16 * 1024
)

type callKind uint8

const (
	kindCall callKind = iota
	kindStatic
	kindDelegate
)

// CallBuilder builds a cross-contract call. Start with Call, StaticCall, or
// DelegateCall; chain Args/Value/Gas/ReturnCap; finish with Exec or
// ExecOrRevert.
type CallBuilder struct {
	target    Address
	fn        string
	calldata  []byte
	value     U128
	gas       int64
	returnCap int
	kind      callKind
}

func newCall(target Address, fn string, kind callKind) *CallBuilder {
	return &CallBuilder{target: target, fn: fn, gas: forwardAllGas, returnCap: maxReturnBytes, kind: kind}
}

// Call builds a mutating cross-contract call to target.fn.
func Call(target Address, fn string) *CallBuilder { return newCall(target, fn, kindCall) }

// StaticCall builds a read-only cross-contract call; target.fn must be a view
// function and the sub-call cannot mutate state, move value, or emit events.
func StaticCall(target Address, fn string) *CallBuilder { return newCall(target, fn, kindStatic) }

// DelegateCall builds a delegatecall: target's code runs in THIS contract's
// storage context (proxy / upgradeable pattern). Value does not apply.
func DelegateCall(target Address, fn string) *CallBuilder { return newCall(target, fn, kindDelegate) }

// Args sets the borsh-encoded argument bytes.
func (c *CallBuilder) Args(calldata []byte) *CallBuilder { c.calldata = calldata; return c }

// Value attaches native PYDE (quanta). Applies only to Call; ignored by
// StaticCall/DelegateCall, which take no value.
func (c *CallBuilder) Value(v U128) *CallBuilder { c.value = v; return c }

// Gas caps the gas forwarded (the engine forwards min(gas, remaining)).
// Default: forward all remaining.
func (c *CallBuilder) Gas(g uint64) *CallBuilder { c.gas = int64(g); return c }

// ReturnCap sets the maximum return-data size accepted (default 16 KiB). Exec
// reverts if the callee returns more, rather than silently truncating.
func (c *CallBuilder) ReturnCap(n int) *CallBuilder { c.returnCap = n; return c }

// Exec performs the call and returns (returnData, status). status is StatusOK
// (0) on success or a negative ABI error code (ErrCrossCallFailed,
// ErrInvalidFunctionName, ErrReentrancyBlocked, …). On success returnData is
// the callee's borsh-encoded return value; decode it with NewDecoder.
func (c *CallBuilder) Exec() ([]byte, int32) {
	fnb := []byte(c.fn)
	out := make([]byte, c.returnCap)
	outLen := int32(c.returnCap)
	var rc int32
	switch c.kind {
	case kindStatic:
		rc = crossCallStatic(ptr(c.target[:]), ptr(fnb), int32(len(fnb)),
			ptr(c.calldata), int32(len(c.calldata)), c.gas, ptr(out), ptrOf32(&outLen))
	case kindDelegate:
		rc = delegateCall(ptr(c.target[:]), ptr(fnb), int32(len(fnb)),
			ptr(c.calldata), int32(len(c.calldata)), c.gas, ptr(out), ptrOf32(&outLen))
	default:
		val := c.value.ToBytesLE()
		rc = crossCall(ptr(c.target[:]), ptr(fnb), int32(len(fnb)),
			ptr(c.calldata), int32(len(c.calldata)), ptr(val[:]), c.gas, ptr(out), ptrOf32(&outLen))
	}
	if rc != StatusOK {
		return nil, rc
	}
	if int(outLen) > c.returnCap {
		Revert("pyde: cross-call return data exceeds ReturnCap")
	}
	if outLen < 0 {
		outLen = 0
	}
	return out[:outLen], StatusOK
}

// ExecOrRevert performs the call, reverting on any non-OK status, and returns
// the callee's borsh-encoded return data.
func (c *CallBuilder) ExecOrRevert() []byte {
	ret, rc := c.Exec()
	if rc != StatusOK {
		Revert("pyde: cross-call failed")
	}
	return ret
}
