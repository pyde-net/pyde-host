// importprobe is a minimal contract that exercises the Phase-2 wrapper
// surface (ctx, calldata/args, hash, exit) so the import-purity test can
// build it with TinyGo and assert the resulting wasm imports ONLY the
// `pyde` module. It is test fixture, not a shipped example.
package main

import pyde "github.com/pyde-net/pyde-host/go"

//go:wasmexport probe
func probe() {
	// calldata / args
	a := pyde.Args()
	minValue := a.U128()

	// context accessors
	if pyde.Caller().IsZero() {
		pyde.Revert("zero caller")
	}
	pyde.Require(pyde.Origin().Equal(pyde.Caller()) || true, "origin")
	if pyde.TxValue().Lt(minValue) {
		pyde.Revert("insufficient value")
	}

	// misc context (pull the remaining §7.3/§7.4/§7.11 imports in)
	_ = pyde.WaveId() + pyde.WaveTimestamp() + pyde.ChainId() + pyde.GasRemaining()
	_ = pyde.TxHash()
	_ = pyde.Beacon()

	// hashing
	digest := pyde.Blake3(pyde.Self().Bytes())
	_ = pyde.Poseidon2(digest[:])
	_ = pyde.Keccak256(digest[:])

	// return
	pyde.ReturnU128(pyde.TxValue())
}

func main() {}
