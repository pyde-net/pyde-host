//go:build tinygo

// Factory instantiation (§7.12) — create a child instance of a deployed
// template contract. The child is addressed child_address(self, template,
// salt) and shares the template's cached code by reference (nothing is copied
// or recompiled). Off-chain, the child address is predictable via the
// go/child helper; on-chain it is returned here.

package pyde

// forwardAllInstantiate is the instantiate gas sentinel: unlike cross_call
// (which min-clamps a large budget), instantiate treats a NEGATIVE gas limit
// as "forward all remaining" (HOST_FN_ABI_SPEC §7.12).
const forwardAllInstantiate int64 = -1

// Factory builds a child instantiation. Start with New; chain
// Salt/Args/Value/Gas; finish with Instantiate or InstantiateOrRevert.
type Factory struct {
	template Address
	salt     Bytes32
	initArgs []byte
	value    U128
	gas      int64
	retCap   int
}

// New starts a child instantiation of the deployed template at `template`.
func New(template Address) *Factory {
	return &Factory{template: template, gas: forwardAllInstantiate, retCap: maxReturnBytes}
}

// Salt sets the 32 opaque caller-derived bytes that (with self + template)
// determine the child address.
func (f *Factory) Salt(s Bytes32) *Factory { f.salt = s; return f }

// Args sets the borsh-encoded constructor calldata (≤ 16 KiB; leave empty for
// a constructor-less template).
func (f *Factory) Args(initCalldata []byte) *Factory { f.initArgs = initCalldata; return f }

// Value sets the native-PYDE endowment (quanta) credited to the child.
func (f *Factory) Value(v U128) *Factory { f.value = v; return f }

// Gas caps the gas for the instantiation + constructor. Default: forward all
// remaining.
func (f *Factory) Gas(g uint64) *Factory { f.gas = int64(g); return f }

// Instantiate creates the child and returns (childAddress, data, status).
// status is StatusOK (0) or a negative code: ErrCtorReverted (-40, atomic
// refund — data holds the child constructor's revert payload), ErrTemplateNotContract
// (-43), ErrChildAddressTaken (-44), ErrInitOnCtorless (-45), ErrPrefixCollision
// (-46), ErrPerTxCapReached (-48), ErrInsufficientBalance (-3). data is empty on
// every non-revert outcome. The child address is written on every path past the
// early cap/bounds checks.
func (f *Factory) Instantiate() (Address, []byte, int32) {
	var child Address
	out := make([]byte, f.retCap)
	outLen := int32(f.retCap)
	val := f.value.ToBytesLE()
	rc := instantiate(
		ptr(f.template[:]), ptr(f.salt[:]),
		ptr(f.initArgs), int32(len(f.initArgs)),
		ptr(val[:]), f.gas,
		ptr(child[:]), ptr(out), ptrOf32(&outLen),
	)
	// On ErrCtorReverted (-40) the engine writes the child constructor's revert
	// payload into out (outLen = its length; 0 for every other outcome). Return
	// it so callers can surface the child's own message.
	n := int(outLen)
	if n < 0 {
		n = 0
	} else if n > f.retCap {
		n = f.retCap
	}
	return child, out[:n], rc
}

// InstantiateOrRevert is like Instantiate but reverts on any non-OK status,
// returning the child address on success.
func (f *Factory) InstantiateOrRevert() Address {
	child, data, rc := f.Instantiate()
	if rc != StatusOK {
		if len(data) > 0 {
			Revert(string(data)) // forward the child constructor's revert message
		}
		Revert("pyde: instantiate failed")
	}
	return child
}
