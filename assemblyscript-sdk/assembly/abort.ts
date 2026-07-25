// Custom `abort` handler — the fix for the env.abort deploy hazard.
//
// ─── The problem ──────────────────────────────────────────────────────
//
// AssemblyScript lowers every `assert(...)`, unchecked-cast failure, and
// out-of-bounds access into a call to a function named `abort`. By
// default that function is an IMPORT from module "env":
//
//     (import "env" "abort" (func ...))
//
// A Pyde contract may import ONLY `pyde.*` — the engine's deploy
// validator rejects any `env` / `wasi_snapshot_preview1` import. So a
// contract that trips a single `assert` would fail to deploy.
//
// ─── The fix ──────────────────────────────────────────────────────────
//
// We supply our OWN `abort` and point the compiler at it with
//
//     "use": ["abort=assembly/abort/abort"]   (asconfig.json)
//
// which replaces the `env.abort` import with a direct call to this
// function. On-chain behavior is now the right one: an assert failure
// turns into a `pyde::revert` carrying the message, so the transaction
// reverts cleanly with a readable reason instead of trapping opaquely.
//
// A downstream contract linking @pyde-net/sdk sets the same `use` line
// (otigen wires this into the contract's asconfig); the SDK owns the
// implementation so every contract gets identical, correct behavior.

import { revert } from "@pyde-net/host/assembly";

// Signature dictated by the AssemblyScript abort ABI: the message and
// source-file references are `string | null`, the position is `u32`.
export function abort(
  message: string | null,
  fileName: string | null,
  line: u32,
  column: u32,
): void {
  if (message !== null) {
    const utf8 = String.UTF8.encode(changetype<string>(message));
    revert(changetype<usize>(utf8), utf8.byteLength);
  }
  // No message (or the revert host fn returned in a non-chain harness):
  // trap so execution cannot fall through past a failed invariant.
  unreachable();
}
