// importprobe is a minimal contract that exercises the SDK wrapper surface
// (ctx, calldata/args, hash, exit, storage, events, account, call, factory)
// so the import-purity test can build it with TinyGo and assert the resulting
// wasm imports ONLY the `pyde` module. It is a test fixture, not an example.
package main

import pyde "github.com/pyde-net/pyde-host/go"

//go:wasmexport probe
func probe() {
	// calldata / args
	a := pyde.Args()
	minValue := a.U128()
	to := a.Address()

	// context accessors
	if pyde.Caller().IsZero() {
		pyde.Revert("zero caller")
	}
	pyde.Require(pyde.Origin().Equal(pyde.Caller()) || true, "origin")
	if pyde.TxValue().Lt(minValue) {
		pyde.Revert("insufficient value")
	}
	_ = pyde.WaveId() + pyde.WaveTimestamp() + pyde.ChainId() + pyde.GasRemaining()
	_ = pyde.TxHash()
	_ = pyde.Beacon()

	// hashing
	digest := pyde.Blake3(pyde.Self().Bytes())
	_ = pyde.Poseidon2(digest[:])
	_ = pyde.Keccak256(digest[:])

	// storage (scalar + map)
	pyde.StoreScalar("total", pyde.NewEncoder().U128(minValue).Finish())
	total, _ := pyde.LoadScalar("total")
	pyde.StoreMap1("balances", to.Bytes(), pyde.NewEncoder().U128(minValue).Finish())
	bal, _ := pyde.LoadMap1("balances", to.Bytes())
	pyde.DeleteMap1("balances", to.Bytes())
	_ = total
	_ = bal

	// events
	pyde.Emit(
		[]pyde.Bytes32{pyde.EventTopic0("Probe(address,u128)")},
		pyde.NewEncoder().Address(to).U128(minValue).Finish(),
	)

	// account
	if pyde.Balance(pyde.Self()).Gte(minValue) {
		pyde.Transfer(to, minValue)
	}
	_ = pyde.TryTransfer(to, pyde.U128From(1))

	// cross-call + static + delegate
	ret, st := pyde.Call(to, "ping").Args(digest[:]).Value(pyde.U128From(0)).Exec()
	_ = ret
	_ = st
	_ = pyde.StaticCall(to, "view").ExecOrRevert()
	_ = pyde.DelegateCall(to, "impl").Args(nil).ExecOrRevert()

	// factory
	child, _ := pyde.New(to).Salt(digest).Args(nil).Instantiate()

	// return
	pyde.ReturnAddress(child)
}

func main() {}
