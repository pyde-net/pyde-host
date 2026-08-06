//go:build tinygo

// Backward-compatible low-level aliases.
//
// Before the typed-storage SDK (pyde-host/go v0.1.0-alpha.10) the entire
// host ABI was exported from this package under PascalCase names —
// contracts called pyde.Sload, pyde.SelfAddress, pyde.PydeReturn, and so
// on directly. The typed migration moved the raw bindings to unexported
// plumbing behind ergonomic wrappers, which silently broke every contract
// written against the raw surface.
//
// This file restores those names as thin forwarders to the raw escape
// hatch (package .../go/raw), so pre-typed contracts compile unchanged.
// New code should prefer the ergonomic wrappers (StoreScalar, Self,
// Return, …) or, when byte-exact control is needed, the raw.* package
// directly — these aliases exist for source compatibility.
//
// Only the names that do NOT collide with an ergonomic wrapper live here.
// The five raw fns whose names are taken by higher-level wrappers
// (Balance, Transfer, Caller, Origin, Revert) plus the same-name/
// different-signature context fns (WaveId, ChainId, TxHash, TxValue,
// FalconVerify, DelegateCall) are reachable only as raw.Balance,
// raw.Caller, … — the parent package keeps the ergonomic versions.

package pyde

import "github.com/pyde-net/pyde-host/go/raw"

// ── Raw storage ──────────────────────────────────────────────────────

// Sload is a back-compat alias for raw.Sload.
func Sload(slotPtr, outPtr, outMaxLen int32) int32 { return raw.Sload(slotPtr, outPtr, outMaxLen) }

// Sstore is a back-compat alias for raw.Sstore.
func Sstore(slotPtr, valPtr, valLen int32) { raw.Sstore(slotPtr, valPtr, valLen) }

// Sdelete is a back-compat alias for raw.Sdelete.
func Sdelete(slotPtr int32) { raw.Sdelete(slotPtr) }

// ── Typed storage ────────────────────────────────────────────────────

// SstoreScalar is a back-compat alias for raw.SstoreScalar.
func SstoreScalar(fieldPtr, fieldLen, valuePtr, valueLen int32) int32 {
	return raw.SstoreScalar(fieldPtr, fieldLen, valuePtr, valueLen)
}

// SloadScalar is a back-compat alias for raw.SloadScalar.
func SloadScalar(fieldPtr, fieldLen, outPtr, outMaxLen int32) int32 {
	return raw.SloadScalar(fieldPtr, fieldLen, outPtr, outMaxLen)
}

// SdeleteScalar is a back-compat alias for raw.SdeleteScalar.
func SdeleteScalar(fieldPtr, fieldLen int32) int32 { return raw.SdeleteScalar(fieldPtr, fieldLen) }

// SstoreMap1 is a back-compat alias for raw.SstoreMap1.
func SstoreMap1(fieldPtr, fieldLen, keyPtr, keyLen, valuePtr, valueLen int32) int32 {
	return raw.SstoreMap1(fieldPtr, fieldLen, keyPtr, keyLen, valuePtr, valueLen)
}

// SloadMap1 is a back-compat alias for raw.SloadMap1.
func SloadMap1(fieldPtr, fieldLen, keyPtr, keyLen, outPtr, outMaxLen int32) int32 {
	return raw.SloadMap1(fieldPtr, fieldLen, keyPtr, keyLen, outPtr, outMaxLen)
}

// SdeleteMap1 is a back-compat alias for raw.SdeleteMap1.
func SdeleteMap1(fieldPtr, fieldLen, keyPtr, keyLen int32) int32 {
	return raw.SdeleteMap1(fieldPtr, fieldLen, keyPtr, keyLen)
}

// SstoreMap2 is a back-compat alias for raw.SstoreMap2.
func SstoreMap2(fieldPtr, fieldLen, k1Ptr, k1Len, k2Ptr, k2Len, valuePtr, valueLen int32) int32 {
	return raw.SstoreMap2(fieldPtr, fieldLen, k1Ptr, k1Len, k2Ptr, k2Len, valuePtr, valueLen)
}

// SloadMap2 is a back-compat alias for raw.SloadMap2.
func SloadMap2(fieldPtr, fieldLen, k1Ptr, k1Len, k2Ptr, k2Len, outPtr, outMaxLen int32) int32 {
	return raw.SloadMap2(fieldPtr, fieldLen, k1Ptr, k1Len, k2Ptr, k2Len, outPtr, outMaxLen)
}

// SdeleteMap2 is a back-compat alias for raw.SdeleteMap2.
func SdeleteMap2(fieldPtr, fieldLen, k1Ptr, k1Len, k2Ptr, k2Len int32) int32 {
	return raw.SdeleteMap2(fieldPtr, fieldLen, k1Ptr, k1Len, k2Ptr, k2Len)
}

// SstoreMap3 is a back-compat alias for raw.SstoreMap3.
func SstoreMap3(fieldPtr, fieldLen, k1Ptr, k1Len, k2Ptr, k2Len, k3Ptr, k3Len, valuePtr, valueLen int32) int32 {
	return raw.SstoreMap3(fieldPtr, fieldLen, k1Ptr, k1Len, k2Ptr, k2Len, k3Ptr, k3Len, valuePtr, valueLen)
}

// SloadMap3 is a back-compat alias for raw.SloadMap3.
func SloadMap3(fieldPtr, fieldLen, k1Ptr, k1Len, k2Ptr, k2Len, k3Ptr, k3Len, outPtr, outMaxLen int32) int32 {
	return raw.SloadMap3(fieldPtr, fieldLen, k1Ptr, k1Len, k2Ptr, k2Len, k3Ptr, k3Len, outPtr, outMaxLen)
}

// SdeleteMap3 is a back-compat alias for raw.SdeleteMap3.
func SdeleteMap3(fieldPtr, fieldLen, k1Ptr, k1Len, k2Ptr, k2Len, k3Ptr, k3Len int32) int32 {
	return raw.SdeleteMap3(fieldPtr, fieldLen, k1Ptr, k1Len, k2Ptr, k2Len, k3Ptr, k3Len)
}

// ── Context / tx ─────────────────────────────────────────────────────

// SelfAddress is a back-compat alias for raw.SelfAddress (raw pointer
// form). The ergonomic equivalent is Self, which returns an Address.
func SelfAddress(addrOutPtr int32) int32 { return raw.SelfAddress(addrOutPtr) }

// TxGasRemaining is a back-compat alias for raw.TxGasRemaining. The
// ergonomic equivalent is GasRemaining.
func TxGasRemaining() int64 { return raw.TxGasRemaining() }

// CalldataSize is a back-compat alias for raw.CalldataSize.
func CalldataSize() int32 { return raw.CalldataSize() }

// CalldataCopy is a back-compat alias for raw.CalldataCopy.
func CalldataCopy(outPtr, outLenPtr int32) int32 { return raw.CalldataCopy(outPtr, outLenPtr) }

// ── Events ───────────────────────────────────────────────────────────

// EmitEvent is a back-compat alias for raw.EmitEvent. The ergonomic
// equivalent is Emit.
func EmitEvent(topicsPtr, topicsCount, dataPtr, dataLen int32) int32 {
	return raw.EmitEvent(topicsPtr, topicsCount, dataPtr, dataLen)
}

// ── Hashing ──────────────────────────────────────────────────────────

// HashBlake3 is a back-compat alias for raw.HashBlake3 (raw pointer
// form). The ergonomic equivalent is Blake3.
func HashBlake3(inPtr, inLen, outPtr int32) { raw.HashBlake3(inPtr, inLen, outPtr) }

// HashPoseidon2 is a back-compat alias for raw.HashPoseidon2. The
// ergonomic equivalent is Poseidon2.
func HashPoseidon2(inPtr, inLen, outPtr int32) { raw.HashPoseidon2(inPtr, inLen, outPtr) }

// HashKeccak256 is a back-compat alias for raw.HashKeccak256. The
// ergonomic equivalent is Keccak256.
func HashKeccak256(inPtr, inLen, outPtr int32) { raw.HashKeccak256(inPtr, inLen, outPtr) }

// ── Cross-contract ───────────────────────────────────────────────────

// CrossCall is a back-compat alias for raw.CrossCall. The ergonomic
// equivalent is the Call builder.
func CrossCall(targetPtr, fnNamePtr, fnNameLen, calldataPtr, calldataLen, valuePtr int32, gasLimit int64, returnDataOutPtr, returnDataOutLenPtr int32) int32 {
	return raw.CrossCall(targetPtr, fnNamePtr, fnNameLen, calldataPtr, calldataLen, valuePtr, gasLimit, returnDataOutPtr, returnDataOutLenPtr)
}

// CrossCallStatic is a back-compat alias for raw.CrossCallStatic. The
// ergonomic equivalent is the StaticCall builder.
func CrossCallStatic(targetPtr, fnNamePtr, fnNameLen, calldataPtr, calldataLen int32, gasLimit int64, returnDataOutPtr, returnDataOutLenPtr int32) int32 {
	return raw.CrossCallStatic(targetPtr, fnNamePtr, fnNameLen, calldataPtr, calldataLen, gasLimit, returnDataOutPtr, returnDataOutLenPtr)
}

// ── Halt / gas / beacon / factory ────────────────────────────────────

// PydeReturn is a back-compat alias for raw.PydeReturn (raw pointer
// form). The ergonomic equivalents are Return / ReturnU64 / ….
func PydeReturn(dataPtr, dataLen int32) { raw.PydeReturn(dataPtr, dataLen) }

// ConsumeGas is a back-compat alias for raw.ConsumeGas.
func ConsumeGas(amount int64) { raw.ConsumeGas(amount) }

// BeaconGet is a back-compat alias for raw.BeaconGet (raw pointer form).
// The ergonomic equivalent is Beacon.
func BeaconGet(outPtr int32) int32 { return raw.BeaconGet(outPtr) }

// Instantiate is a back-compat alias for raw.Instantiate. The ergonomic
// equivalent is the New factory builder.
func Instantiate(templatePtr, saltPtr, initCalldataPtr, initCalldataLen, valuePtr int32, gasLimit int64, childAddrOutPtr, returnDataOutPtr, returnDataOutLenPtr int32) int32 {
	return raw.Instantiate(templatePtr, saltPtr, initCalldataPtr, initCalldataLen, valuePtr, gasLimit, childAddrOutPtr, returnDataOutPtr, returnDataOutLenPtr)
}
