// Cross-contract calls (HOST_FN_ABI_SPEC §7.8) — a small builder over
// `cross_call` / `cross_call_static` / `delegate_call`.
//
// A sub-call that reverts is RECOVERABLE: the host returns a negative
// status code and the caller keeps running, so a contract can inspect the
// result and decide. Only critical failures (call depth exceeded,
// code-cache miss) trap the whole transaction, and those never reach here.
//
// Shape and semantics mirror the Go SDK's `call.go` deliberately — same
// defaults, same revert-payload forwarding, same "reverts rather than
// truncates" rule on an oversized return — so a contract ported between
// the two behaves identically.
//
//   const ret = Call(token, "transfer").args(cd).execOrRevert();
//
//   const r = Call(token, "transfer").args(cd).exec();
//   if (r.ok) { /* decode r.data */ } else { /* inspect r.status */ }

import { cross_call, cross_call_static, delegate_call } from "./host_fns";
import { Address } from "./types";
import { u128, u128ToBytesLE } from "./u128";
import { revertStr } from "./exit";
import { STATUS_OK, ERR_CROSS_CALL_FAILED, statusName } from "./status";

/// Default gas budget. The engine forwards `min(gas, parent_remaining)`, so
/// a value far above any real budget means "forward everything left".
export const FORWARD_ALL_GAS: i64 = 1 << 62;

/// Default cap on return data copied back from a sub-call, symmetric with
/// the 16 KiB calldata cap. `exec` reverts rather than truncating, because
/// a silently short return would decode as a different value.
export const MAX_RETURN_BYTES: i32 = 16 * 1024;

const KIND_CALL: u8 = 0;
const KIND_STATIC: u8 = 1;
const KIND_DELEGATE: u8 = 2;

/// The outcome of a sub-call.
///
/// `ok` is the only thing worth branching on: any non-zero status is a
/// failure. On a revert, `data` carries the callee's revert payload rather
/// than its return value, which is what lets a caller surface the callee's
/// own message instead of a generic one.
export class CallResult {
  constructor(
    public status: i32,
    public data: StaticArray<u8>,
  ) {}

  get ok(): bool {
    return this.status == STATUS_OK;
  }

  /// True when the callee reverted, as opposed to the call being rejected
  /// before it ran (a bad function name, a blocked reentrant call). Both
  /// are failures; only this one carries a payload from the callee.
  get reverted(): bool {
    return this.status == ERR_CROSS_CALL_FAILED;
  }

  /// The revert payload as text, or "" when there was none. The engine does
  /// not require a payload to be UTF-8, so this is for surfacing a message,
  /// never for control flow.
  get revertMessage(): string {
    if (this.data.length == 0) return "";
    return String.UTF8.decodeUnsafe(changetype<usize>(this.data), this.data.length);
  }
}

/// Builds a cross-contract call. Start with [[Call]], [[StaticCall]], or
/// [[DelegateCall]]; chain `args` / `value` / `gas` / `returnCap`; finish
/// with `exec` or `execOrRevert`.
export class CallBuilder {
  private calldata: StaticArray<u8> = new StaticArray<u8>(0);
  private amount: u128 = u128.Zero;
  private gasLimit: i64 = FORWARD_ALL_GAS;
  private cap: i32 = MAX_RETURN_BYTES;

  constructor(
    private target: Address,
    private fn: string,
    private kind: u8,
  ) {}

  /// The borsh-encoded argument bytes.
  args(calldata: StaticArray<u8>): CallBuilder {
    this.calldata = calldata;
    return this;
  }

  /// Native PYDE (quanta) to attach. Applies only to a plain call; a static
  /// or delegate call takes no value and ignores this.
  value(v: u128): CallBuilder {
    this.amount = v;
    return this;
  }

  /// Cap the gas forwarded. The engine forwards `min(gas, remaining)`;
  /// the default forwards everything left.
  gas(g: i64): CallBuilder {
    this.gasLimit = g;
    return this;
  }

  /// Maximum return-data size accepted. `exec` reverts if the callee
  /// returns more, rather than silently truncating.
  returnCap(n: i32): CallBuilder {
    this.cap = n;
    return this;
  }

  /// Perform the call. Never traps on a sub-call failure — inspect
  /// `result.ok` and `result.status`.
  exec(): CallResult {
    const fnBytes = String.UTF8.encode(this.fn);
    const fnLen = fnBytes.byteLength;
    const out = new StaticArray<u8>(this.cap);
    // The host writes the ACTUAL return length back through this pointer,
    // so it is seeded with the buffer size and read afterwards.
    const outLen = new StaticArray<i32>(1);
    outLen[0] = this.cap;

    let rc: i32;
    if (this.kind == KIND_STATIC) {
      rc = cross_call_static(
        changetype<usize>(this.target),
        changetype<usize>(fnBytes),
        fnLen,
        changetype<usize>(this.calldata),
        this.calldata.length,
        this.gasLimit,
        changetype<usize>(out),
        changetype<usize>(outLen),
      );
    } else if (this.kind == KIND_DELEGATE) {
      rc = delegate_call(
        changetype<usize>(this.target),
        changetype<usize>(fnBytes),
        fnLen,
        changetype<usize>(this.calldata),
        this.calldata.length,
        this.gasLimit,
        changetype<usize>(out),
        changetype<usize>(outLen),
      );
    } else {
      const val = u128ToBytesLE(this.amount);
      rc = cross_call(
        changetype<usize>(this.target),
        changetype<usize>(fnBytes),
        fnLen,
        changetype<usize>(this.calldata),
        this.calldata.length,
        changetype<usize>(val),
        this.gasLimit,
        changetype<usize>(out),
        changetype<usize>(outLen),
      );
    }

    // The host writes a length only on the two paths where the callee
    // actually ran: its return value on success, its revert payload on
    // ERR_CROSS_CALL_FAILED. Every other status is a refusal decided
    // BEFORE the callee ran — a bad function name, a blocked reentrant
    // call — and on those the host returns without touching the pointer,
    // leaving it at the capacity seeded above. Reading it there would
    // hand back a whole bufferful of zero bytes as if the callee had sent
    // a revert message, which is both wrong and enormous.
    if (rc != STATUS_OK && rc != ERR_CROSS_CALL_FAILED) {
      return new CallResult(rc, new StaticArray<u8>(0));
    }

    let n = outLen[0];
    if (n < 0) n = 0;

    if (rc == STATUS_OK) {
      // Truncating here would hand back bytes that borsh-decode to a
      // different value than the callee returned. Fail loudly instead.
      if (n > this.cap) {
        revertStr("pyde: cross-call return data exceeds returnCap");
      }
      return new CallResult(STATUS_OK, copyOf(out, n));
    }
    if (n > this.cap) n = this.cap;
    return new CallResult(rc, copyOf(out, n));
  }

  /// Perform the call and revert on any non-OK status, returning the
  /// callee's borsh-encoded return data.
  ///
  /// A callee revert message is forwarded verbatim, so the failure a user
  /// sees is the one the callee actually raised rather than a generic
  /// wrapper. Only when there is no payload does this synthesize a reason,
  /// and that reason names the status so the code is never lost.
  execOrRevert(): StaticArray<u8> {
    const r = this.exec();
    if (!r.ok) {
      const msg = r.revertMessage;
      if (msg.length > 0) {
        revertStr(msg);
      }
      revertStr("pyde: cross-call failed: " + statusName(r.status));
    }
    return r.data;
  }
}

/// Copy the first `n` bytes out of `src`. The call buffer is allocated at
/// the cap, so the result has to be trimmed to what the host actually
/// wrote before it reaches the caller.
function copyOf(src: StaticArray<u8>, n: i32): StaticArray<u8> {
  const dst = new StaticArray<u8>(n);
  if (n > 0) {
    memory.copy(changetype<usize>(dst), changetype<usize>(src), <usize>n);
  }
  return dst;
}

/// A mutating cross-contract call to `target.fn`.
export function Call(target: Address, fn: string): CallBuilder {
  return new CallBuilder(target, fn, KIND_CALL);
}

/// A read-only cross-contract call. `target.fn` must be a view function;
/// the sub-call cannot mutate state, move value, or emit events.
export function StaticCall(target: Address, fn: string): CallBuilder {
  return new CallBuilder(target, fn, KIND_STATIC);
}

/// A delegate call: the target's code runs in THIS contract's storage
/// context, which is the proxy / upgradeable pattern. Value does not apply.
export function DelegateCall(target: Address, fn: string): CallBuilder {
  return new CallBuilder(target, fn, KIND_DELEGATE);
}
