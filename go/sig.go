//go:build tinygo

// Post-quantum signatures (§7.7). A pointer-free wrapper over the FALCON-512
// verify host fn — contracts pass byte slices, never offsets.

package pyde

// FalconVerify reports whether sig is a valid FALCON-512 signature of msg under
// publicKey. publicKey is the ~897-byte FALCON-512 public key; sig is the
// variable-length (~660–690 byte) signature. Verification runs host-side, so a
// contract never links a lattice library into its wasm.
//
// Use it for on-chain multisig, meta-transactions, or any scheme that checks a
// signature the transaction sender didn't have to be — the message and key are
// contract inputs, decoupled from tx authorization.
func FalconVerify(publicKey, msg, sig []byte) bool {
	return falconVerify(
		ptr(publicKey),
		ptr(msg), int32(len(msg)),
		ptr(sig), int32(len(sig)),
	) == StatusOK
}
