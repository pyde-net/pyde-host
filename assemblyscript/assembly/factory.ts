// Factory instantiation (HOST_FN_ABI_SPEC §7.12) — create a child instance
// of a deployed template contract.
//
// The child is addressed `Poseidon2("pyde-child:" ‖ factory ‖ template ‖
// salt)` and shares the template's already-cached code by reference: nothing
// is copied and nothing is recompiled. Because the address is a pure
// function of those three inputs, a factory can hand it out BEFORE the child
// exists, and the same salt always targets the same child.
//
//   const child = New(template).salt(s).instantiateOrRevert();
//
//   const r = New(template).salt(s).args(ctorArgs).instantiate();
//   if (r.ok) { /* r.child is live, endowed, constructed */ }
//
// Shape and semantics mirror the Go SDK's `factory.go`, so a contract ported
// between the two behaves identically.

import { instantiate as h_instantiate } from "./host_fns";
import { Address, Bytes32 } from "./types";
import { u128, u128ToBytesLE } from "./u128";
import { newAddress } from "./types";
import { revertStr } from "./exit";
import { STATUS_OK, ERR_CTOR_REVERTED, statusName } from "./status";
import { MAX_RETURN_BYTES } from "./call";

/// Gas sentinel: forward everything the parent frame has left.
///
/// Verified against `engine/crates/wasm-exec/src/host_fns/instantiate.rs`:
/// the limit is resolved as
/// `u64::try_from(gas).unwrap_or(remaining).min(remaining)`, so a negative
/// value and an over-large positive one both end up forwarding all
/// remaining gas. `cross_call` resolves it with the identical expression.
///
/// `-1` is chosen to match the Rust and Go SDKs, which both use it here.
/// Note those two use a large positive sentinel for cross-calls instead
/// (`i64::MAX` and `1 << 62`), so the vocabulary is inconsistent ACROSS
/// host fns while the engine behaviour is not.
export const FORWARD_ALL_CTOR_GAS: i64 = -1;

/// The outcome of an instantiation.
///
/// Every failure is atomic: there are no half-born children, and an
/// endowment is refunded rather than lost. `child` is written on every path
/// past the early bounds checks, so it is readable even on failure — useful
/// for reporting which address collided.
export class InstantiateResult {
  constructor(
    public status: i32,
    public child: Address,
    public data: StaticArray<u8>,
  ) {}

  get ok(): bool {
    return this.status == STATUS_OK;
  }

  /// True when the child's CONSTRUCTOR ran and reverted, as opposed to the
  /// instantiation being refused before it started (template not a contract,
  /// address taken, per-tx cap). Only this case carries a payload from the
  /// child.
  get ctorReverted(): bool {
    return this.status == ERR_CTOR_REVERTED;
  }

  /// The constructor's revert payload as text, or "" when there was none.
  /// The engine does not require it to be UTF-8, so this is for surfacing a
  /// message, never for control flow.
  get revertMessage(): string {
    if (this.data.length == 0) return "";
    return String.UTF8.decodeUnsafe(changetype<usize>(this.data), this.data.length);
  }
}

/// Builds a child instantiation. Start with [[New]]; chain `salt` / `args` /
/// `value` / `gas` / `returnCap`; finish with `instantiate` or
/// `instantiateOrRevert`.
export class Factory {
  private saltBytes: Bytes32 = new StaticArray<u8>(32);
  private initArgs: StaticArray<u8> = new StaticArray<u8>(0);
  private endowment: u128 = u128.Zero;
  private gasLimit: i64 = FORWARD_ALL_CTOR_GAS;
  private cap: i32 = MAX_RETURN_BYTES;

  constructor(private template: Address) {}

  /// The 32 opaque caller-derived bytes that, with this contract's address
  /// and the template's, determine the child address.
  ///
  /// A RANDOM salt is an anti-pattern: it throws away counterfactual
  /// addressing and idempotent re-creation, which are the two things child
  /// addressing exists to give you. Derive it from an identity instead — a
  /// counter, a user id, a market pair — by hashing its borsh encoding.
  salt(s: Bytes32): Factory {
    this.saltBytes = s;
    return this;
  }

  /// Borsh-encoded constructor calldata, at most 16 KiB. Leave it unset for
  /// a template with no constructor: passing args to a constructor-less
  /// template is rejected as `ERR_INIT_ON_CTORLESS` rather than ignored.
  args(initCalldata: StaticArray<u8>): Factory {
    this.initArgs = initCalldata;
    return this;
  }

  /// Native-PYDE endowment (quanta) credited to the child.
  ///
  /// This funds the ACCOUNT the way deploy value does — there is no payable
  /// gate on it — and the constructor sees it through `txValue()`. Refunded
  /// in full if the constructor reverts.
  value(v: u128): Factory {
    this.endowment = v;
    return this;
  }

  /// Gas budget for the instantiation plus the child's constructor.
  /// Default forwards everything remaining.
  gas(limit: i64): Factory {
    this.gasLimit = limit;
    return this;
  }

  /// Maximum constructor return / revert data accepted.
  returnCap(n: i32): Factory {
    this.cap = n;
    return this;
  }

  /// Create the child. Never traps on a failed instantiation — inspect
  /// `result.ok` and `result.status`.
  instantiate(): InstantiateResult {
    const child = newAddress();
    const out = new StaticArray<u8>(this.cap);
    // The host writes the ACTUAL length back through this pointer, so it is
    // seeded with the buffer size and read afterwards.
    const outLen = new StaticArray<i32>(1);
    outLen[0] = this.cap;
    const val = u128ToBytesLE(this.endowment);

    const rc = h_instantiate(
      changetype<usize>(this.template),
      changetype<usize>(this.saltBytes),
      changetype<usize>(this.initArgs),
      this.initArgs.length,
      changetype<usize>(val),
      this.gasLimit,
      changetype<usize>(child),
      changetype<usize>(out),
      changetype<usize>(outLen),
    );

    let n = outLen[0];
    if (n < 0) n = 0;
    if (n > this.cap) n = this.cap;

    const data = new StaticArray<u8>(n);
    if (n > 0) {
      memory.copy(changetype<usize>(data), changetype<usize>(out), <usize>n);
    }
    return new InstantiateResult(rc, child, data);
  }

  /// Create the child, reverting on any failure, and return its address.
  ///
  /// A constructor revert message is forwarded verbatim, so the failure a
  /// user sees is the one the child actually raised. Only when there is no
  /// payload does this synthesize a reason, and that reason names the status
  /// so the code is never lost.
  instantiateOrRevert(): Address {
    const r = this.instantiate();
    if (!r.ok) {
      const msg = r.revertMessage;
      if (msg.length > 0) {
        revertStr(msg);
      }
      revertStr("pyde: instantiate failed: " + statusName(r.status));
    }
    return r.child;
  }
}

/// Start a child instantiation of the deployed template at `template`.
///
/// The template is just any contract already on chain — deploy one, take its
/// address, pass it here. Each child is a first-class contract with its own
/// address and its own isolated storage.
export function New(template: Address): Factory {
  return new Factory(template);
}
