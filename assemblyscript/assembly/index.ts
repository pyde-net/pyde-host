// @pyde-net/host — the AssemblyScript SDK for Pyde contracts.
//
// The full contract-facing surface: LE codecs, 128-bit math, borsh, and
// pointer-free host-function wrappers (ctx / calldata / exit / hash), plus the
// factory child-address helpers. The raw `pyde::*` declarations live in
// ./host_fns and are re-exported via ./raw for the rare host fn the wrappers
// don't cover yet — import those from "@pyde-net/host/assembly/raw" to avoid
// colliding with the ergonomic wrappers (raw `caller` vs `ctx.caller`).
//
//   import { read } from "@pyde-net/host/assembly/calldata";
//   import { blake3 } from "@pyde-net/host/assembly/hash";
//   import { sstore_scalar } from "@pyde-net/host/assembly/raw";

// Force the abort handler into the compilation so asconfig's
// `use: ["abort=assembly/abort/abort"]` resolves. Side-effect import only —
// `abort` stays out of the public namespace.
import "./abort";

// ── primitives ──────────────────────────────────────────────────────────
export * from "./codec";
export * from "./types";
export * from "./u128";
export * from "./borsh";

// ── host-function wrappers ──────────────────────────────────────────────
export * from "./ctx";
export * from "./calldata";
export * from "./exit";
export * from "./hash";

// ── ABI status codes + cross-contract calls ─────────────────────────────
export * from "./status";
export * from "./call";

// ── factory child-address helpers ───────────────────────────────────────
export * from "./child";
