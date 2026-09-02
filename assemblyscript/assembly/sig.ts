// Post-quantum signatures (HOST_FN_ABI_SPEC §7.7).
//
// A pointer-free wrapper over the FALCON-512 verify host fn: contracts hand
// over byte arrays, never linear-memory offsets.

import { falcon_verify } from "./host_fns";
import { STATUS_OK } from "./status";

/// Whether `sig` is a valid FALCON-512 signature of `msg` under `publicKey`.
///
/// `publicKey` is the ~897-byte FALCON-512 public key; `sig` is the
/// variable-length (~660 to 690 byte) signature. Verification runs
/// host-side, so a contract never links a lattice library into its own wasm
/// — which is what keeps the code size and the gas cost sane.
///
/// This is for signatures the transaction sender did not have to produce:
/// on-chain multisig, meta-transactions, anything where the message and key
/// are contract inputs rather than tx authorization. The chain has already
/// verified the sender's own signature before execution begins.
export function falconVerify(
  publicKey: StaticArray<u8>,
  msg: StaticArray<u8>,
  sig: StaticArray<u8>,
): bool {
  return (
    falcon_verify(
      changetype<usize>(publicKey),
      changetype<usize>(msg),
      msg.length,
      changetype<usize>(sig),
      sig.length,
    ) == STATUS_OK
  );
}
