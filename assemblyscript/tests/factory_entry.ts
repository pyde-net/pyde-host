// Test-only compile entry for tests/factory.test.mjs — NOT part of the
// published package (package.json `files` whitelists assembly/ only).
//
// Drives the factory builder and the account wrappers through plain
// pointer/length exports so the Node test can stub `instantiate`,
// `transfer`, and `balance`. The stub IS the fixture: it returns whatever
// status and payload a case needs, which is exactly the seam these wrappers
// have to get right.

import { New, InstantiateResult } from "../assembly/factory";
import { balanceOf, transfer, tryTransfer } from "../assembly/account";
import { u128 } from "../assembly/u128";
import { Address, Bytes32 } from "../assembly/types";

const TEMPLATE = new StaticArray<u8>(32);
const SALT = new StaticArray<u8>(32);
const ARGS = new StaticArray<u8>(64);
const OUT = new StaticArray<u8>(1024);

let lastStatus: i32 = 0;
let lastCtorReverted: bool = false;
let lastMessage: string = "";
let lastChild = new StaticArray<u8>(32);

export function templatePtr(): usize { return changetype<usize>(TEMPLATE); }
export function saltPtr(): usize { return changetype<usize>(SALT); }
export function outPtr(): usize { return changetype<usize>(OUT); }

function record(r: InstantiateResult): void {
  lastStatus = r.status;
  lastCtorReverted = r.ctorReverted;
  lastMessage = r.revertMessage;
  lastChild = r.child;
}

/// `New(template).salt(s).instantiate()` — no ctor args.
export function doInstantiate(): void {
  record(New(changetype<Address>(TEMPLATE)).salt(changetype<Bytes32>(SALT)).instantiate());
}

/// With constructor args, to prove calldata reaches the host.
export function doInstantiateWithArgs(argsLen: i32): void {
  const cd = new StaticArray<u8>(argsLen);
  if (argsLen > 0) {
    memory.copy(changetype<usize>(cd), changetype<usize>(ARGS), <usize>argsLen);
  }
  record(New(changetype<Address>(TEMPLATE)).salt(changetype<Bytes32>(SALT)).args(cd).instantiate());
}

/// With an endowment, to prove the value pointer carries LE bytes.
export function doInstantiateWithValue(lo: u64): void {
  record(
    New(changetype<Address>(TEMPLATE))
      .salt(changetype<Bytes32>(SALT))
      .value(u128.fromU64(lo))
      .instantiate(),
  );
}

/// `instantiateOrRevert` — traps through the stubbed `revert`.
export function doInstantiateOrRevert(): void {
  New(changetype<Address>(TEMPLATE)).salt(changetype<Bytes32>(SALT)).instantiateOrRevert();
}

export function lastStatusCode(): i32 { return lastStatus; }
export function lastWasCtorRevert(): bool { return lastCtorReverted; }

/// Copy the child address into OUT so the test can read it back.
export function lastChildInto(): i32 {
  memory.copy(changetype<usize>(OUT), changetype<usize>(lastChild), 32);
  return 32;
}

export function lastMessageInto(): i32 {
  const b = String.UTF8.encode(lastMessage);
  const n = b.byteLength < OUT.length ? b.byteLength : OUT.length;
  if (n > 0) memory.copy(changetype<usize>(OUT), changetype<usize>(b), <usize>n);
  return n;
}

// ── account wrappers ─────────────────────────────────────────────────

/// `balanceOf` — writes the low 64 bits out so the test can compare.
export function doBalanceLo(): u64 {
  return balanceOf(changetype<Address>(TEMPLATE)).lo;
}

/// `transfer` — reverts through the stub on a non-OK status.
export function doTransfer(lo: u64): void {
  transfer(changetype<Address>(TEMPLATE), u128.fromU64(lo));
}

/// `tryTransfer` — returns false instead of reverting.
export function doTryTransfer(lo: u64): bool {
  return tryTransfer(changetype<Address>(TEMPLATE), u128.fromU64(lo));
}
