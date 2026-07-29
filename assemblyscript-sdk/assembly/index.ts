// @pyde-net/sdk — pointer-free AssemblyScript SDK for Pyde contracts.
//
// Public surface, re-exported per module. `abort` (assembly/abort.ts) is
// intentionally NOT re-exported: it is the compiler's abort handler,
// wired via asconfig `use: ["abort=assembly/abort/abort"]`, not part of
// the contract-facing API.
//
// Generic names like `read` (calldata) and `size` live at the top level
// for convenience; a contract that prefers the namespaced Rust feel can
// import a submodule directly, e.g.
//   import { read } from "@pyde-net/sdk/assembly/calldata";
//   import { blake3 } from "@pyde-net/sdk/assembly/hash";

// Force the abort handler into the compilation so asconfig's
// `use: ["abort=assembly/abort/abort"]` can resolve it. Side-effect
// import only — `abort` stays out of the public namespace.
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
