// ABI status / error codes (HOST_FN_ABI_SPEC §4).
//
// Host fns that return an `i32` use 0 for success and these negative
// sentinels for failure. The values are the wire contract, so they match
// the Go SDK's `status.go` and the Rust SDK exactly — a sub-call that
// reverts must be reported the same way in every language.
//
// Naming note: a failure here is RECOVERABLE. The host returns a code and
// execution continues, so a contract can inspect it and decide. Critical
// failures (call depth exceeded, code-cache miss) trap the whole
// transaction instead and never surface as a code.

/// The success return of every `i32`-returning host fn.
export const STATUS_OK: i32 = 0;

/// Returned by `sload` / `sload_*` when a slot was never written. Distinct
/// from an error: a missing slot reads as the type's zero value.
export const SLOAD_MISSING: i32 = -1;

// ── §4 core error codes ──────────────────────────────────────────────

export const ERR_INSUFFICIENT_BALANCE: i32 = -3;
export const ERR_REENTRANCY_BLOCKED: i32 = -9;
/// The sub-call trapped or reverted. On this code the host also writes the
/// callee's revert payload into the return buffer.
export const ERR_CROSS_CALL_FAILED: i32 = -10;
export const ERR_VALUE_TRANSFER_NOT_PAYABLE: i32 = -12;
export const ERR_INVALID_FUNCTION_NAME: i32 = -13;

// ── Typed-storage errors (schema / type validation) ──────────────────

export const ERR_SCHEMA_MISSING: i32 = -30;
export const ERR_UNKNOWN_FIELD: i32 = -31;
export const ERR_FIELD_KIND_MISMATCH: i32 = -32;
export const ERR_KEY_TYPE_MISMATCH: i32 = -33;
export const ERR_VALUE_TYPE_MISMATCH: i32 = -34;

// ── Factory (instantiate) errors ─────────────────────────────────────

export const ERR_CTOR_REVERTED: i32 = -40;
export const ERR_TEMPLATE_NOT_CONTRACT: i32 = -43;
export const ERR_CHILD_ADDRESS_TAKEN: i32 = -44;
export const ERR_INIT_ON_CTORLESS: i32 = -45;
export const ERR_PREFIX_COLLISION: i32 = -46;
export const ERR_PER_TX_CAP_REACHED: i32 = -48;

/// A human-readable name for a status code, for revert messages and tests.
/// Unknown codes render as `status(<n>)` rather than being swallowed — an
/// unrecognised negative is still a failure and must not read as success.
export function statusName(code: i32): string {
  switch (code) {
    case STATUS_OK:
      return "ok";
    case SLOAD_MISSING:
      return "sload-missing";
    case ERR_INSUFFICIENT_BALANCE:
      return "insufficient-balance";
    case ERR_REENTRANCY_BLOCKED:
      return "reentrancy-blocked";
    case ERR_CROSS_CALL_FAILED:
      return "cross-call-failed";
    case ERR_VALUE_TRANSFER_NOT_PAYABLE:
      return "value-transfer-not-payable";
    case ERR_INVALID_FUNCTION_NAME:
      return "invalid-function-name";
    case ERR_SCHEMA_MISSING:
      return "schema-missing";
    case ERR_UNKNOWN_FIELD:
      return "unknown-field";
    case ERR_FIELD_KIND_MISMATCH:
      return "field-kind-mismatch";
    case ERR_KEY_TYPE_MISMATCH:
      return "key-type-mismatch";
    case ERR_VALUE_TYPE_MISMATCH:
      return "value-type-mismatch";
    case ERR_CTOR_REVERTED:
      return "ctor-reverted";
    case ERR_TEMPLATE_NOT_CONTRACT:
      return "template-not-contract";
    case ERR_CHILD_ADDRESS_TAKEN:
      return "child-address-taken";
    case ERR_INIT_ON_CTORLESS:
      return "init-on-ctorless";
    case ERR_PREFIX_COLLISION:
      return "prefix-collision";
    case ERR_PER_TX_CAP_REACHED:
      return "per-tx-cap-reached";
    default:
      return "status(" + code.toString() + ")";
  }
}
