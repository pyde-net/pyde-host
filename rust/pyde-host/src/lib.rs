//! Canonical Pyde host-function declarations for `wasm32` contracts.
//!
//! Re-exports the full `pyde::*` ABI a Rust contract can call into
//! — every host fn declared in [HOST_FN_ABI_SPEC §7](https://book.pyde.network/companion/HOST_FN_ABI_SPEC).
//!
//! ## Pointer convention
//!
//! All `*const u8` / `*mut u8` parameters are 32-bit offsets into
//! linear memory (`wasm32-unknown-unknown` makes pointers 4 bytes).
//! Multi-byte integers cross the boundary in little-endian (matching
//! WASM's native byte order) unless the spec explicitly says
//! otherwise — see HOST_FN_ABI_SPEC §3.2.
//!
//! ## Dead-code elimination
//!
//! Unused declarations are stripped by the linker's DCE — your
//! final `.wasm` only imports what you actually call. Safe to
//! depend on this crate even if you only use a handful of host fns.
//!
//! ## Usage
//!
//! ```ignore
//! #[no_mangle]
//! pub extern "C" fn ping() {
//!     let bytes = b"pong";
//!     unsafe { pyde_host::pyde_return(bytes.as_ptr(), bytes.len() as i32) }
//! }
//! ```

#![no_std]

// The conformance tests mirror the on-chain derivations against the
// reference `pyde-crypto` implementation (dev-dependency) and read the
// shared golden-vector file — both need std, test-only.
#[cfg(test)]
extern crate std;

/// `dlmalloc` as the contract's global allocator. The `#[pyde::entry]`
/// macro emits `Vec`-using code, so contracts that opt into it need
/// a heap. Opt out with `pyde-host = { features = [] }` and provide
/// your own `#[global_allocator]` if you have a tighter constraint.
#[cfg(all(feature = "alloc", target_arch = "wasm32"))]
#[global_allocator]
static PYDE_ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

/// `#[pyde::entry]` — wraps a function in the `() -> ()` WASM shim
/// required by Pyde's entry-point ABI. Re-exported from
/// `pyde-entry-macros` so contract authors only need one crate
/// dependency to write idiomatic Pyde contracts.
///
/// See `pyde-entry-macros`' crate docs for the expansion shape and
/// the consistency-with-`otigen.toml` rationale.
pub use pyde_entry_macros::entry;

/// `pyde::declare_storage!()` — reads `otigen.toml` at compile time
/// and emits a `mod storage` with one typed accessor per declared
/// field. Author writes
///
/// ```ignore
/// pyde::declare_storage!();
///
/// #[pyde::entry]
/// fn deposit(amount: u128) {
///     let from = /* ... */;
///     storage::balances().write(&from, amount);
/// }
/// ```
///
/// See `pyde-storage-macros` crate docs for the expansion shape.
pub use pyde_storage_macros::declare_storage;

/// `pyde::declare_events!()` — reads `otigen.toml` at compile time
/// and emits a `mod events` with one typed struct + `emit()` impl
/// per declared event. Author writes
///
/// ```ignore
/// pyde::declare_events!();
///
/// events::Transfer { from, to, amount }.emit();
/// ```
///
/// instead of building the topics buffer + `Blake3(signature)`
/// constant by hand. See `pyde-events-macros` crate docs for the
/// expansion shape.
pub use pyde_events_macros::declare_events;

// ─────────────────────────────────────────────────────────────────────
// Canonical type aliases for contract authors
// ─────────────────────────────────────────────────────────────────────
//
// Match the type tokens declared in `otigen.toml`:
//
// | TOML token | Rust type (this module) |
// |------------|-------------------------|
// | `address`  | `Address` (= `[u8; 32]`)|
// | `hash32`   | `Hash32`  (= `[u8; 32]`)|
// | `bytes32`  | `Hash32`  (alias)       |
//
// Primitive types (`u8`/`u64`/`u128`/`bool`/etc.) carry their
// native Rust names. The `declare_storage!()` macro emits accessor
// signatures using these aliases so a contract's source reads as
// idiomatic Rust rather than `[u8; 32]` repeated 50 times.

/// 32-byte Pyde address. Same byte layout as the chain's
/// `pyde_engine_types::Address` but kept as a thin alias here so
/// contracts don't pull the heavy engine-types crate in.
pub type Address = [u8; 32];

/// 32-byte hash digest. Used for content hashes, pubkey-derived
/// identifiers, and any other fixed 32-byte value that isn't an
/// account address. `bytes32` in `otigen.toml` resolves to this
/// (Solidity-compat alias).
pub type Hash32 = [u8; 32];

/// Raw FFI host-function declarations — the chain-facing ABI as a
/// flat `extern "C"` surface. Every Pyde host function in
/// `HOST_FN_ABI_SPEC.md` lives here.
///
/// Contracts rarely call into `raw::*` directly. The ergonomic
/// wrappers at the crate root (`pyde::ctx::*`, `pyde::calldata::*`,
/// `pyde::hash::*`, `pyde::emit_event`, `pyde::revert`, ...)
/// allocate the buffers and decode the bytes for you. Drop down
/// into `raw::*` only when you need byte-exact control (an
/// escape hatch for advanced contracts) or when you're building
/// a higher-level abstraction the wrappers don't cover.
///
/// Every function here is `unsafe`: the host validates pointers
/// against the contract's linear memory and traps on
/// out-of-bounds, so misusing a pointer turns into a trap —
/// not a memory-safety violation in the host — but it still kills
/// the contract instance. Don't pass uninitialized buffers.
pub mod raw {
    #[link(wasm_import_module = "pyde")]
    extern "C" {
        // ─────────────────────────────────────────────────────────────────
        // §7.1 Storage (variable-length values)
        // ─────────────────────────────────────────────────────────────────
        //
        // Storage values are **variable-length** (capped at 16 KB per
        // slot). Slot keys are always 32 bytes — derive them with
        // `hash_poseidon2(self_address || field || key)` or any other
        // scheme that gives you collision-free 32-byte hashes.

        /// Read a storage slot's value into `out_ptr`.
        ///
        /// Writes up to `out_max_len` bytes; **returns the actual length**
        /// (so callers can detect truncation and re-call with a larger
        /// buffer). Returns `-1` for a missing slot (distinct from
        /// length-0 value).
        ///
        /// gas: 100 base + 1 per byte copied.
        pub fn sload(slot_ptr: *const u8, out_ptr: *mut u8, out_max_len: i32) -> i32;

        /// Write `val_len` bytes from `val_ptr` to the slot at `slot_ptr`.
        /// `val_len` is capped at 16 KB; larger values trap.
        ///
        /// gas: 5,000 base + 32 per byte. Returns nothing (`ERR_FORBIDDEN`
        /// from a `view`-attributed function traps).
        pub fn sstore(slot_ptr: *const u8, val_ptr: *const u8, val_len: i32);

        /// Delete a storage slot (sets it back to "never written"). A
        /// subsequent `sload` returns `-1`.
        ///
        /// gas: 5,000 base. No refund (PIP-4 `gas-no-refund`).
        pub fn sdelete(slot_ptr: *const u8);

        // ─────────────────────────────────────────────────────────────────
        // §7.1 Typed storage — schema-enforced + slot derivation host-side
        // ─────────────────────────────────────────────────────────────────
        //
        // The new safe path. The contract passes the field name + keys;
        // the host:
        //   1. Looks up the field in the deployed schema
        //      (`otigen.toml`'s `[state.*]` blocks → `ContractAbi.state_schema`).
        //   2. Validates the key + value byte lengths against the
        //      declared types.
        //   3. Derives the slot internally as
        //      `Poseidon2(self_address || field_name || keys...)` so the
        //      contract can't write to a slot derived from another
        //      contract's address.
        //   4. Reads / writes / deletes against the contract's storage.
        //
        // Return codes (i32):
        //
        // | Code | Meaning |
        // |------|---------|
        // | ≥ 0  | success (sstore/sdelete = 0; sload = actual value length) |
        // | -1   | sload: slot missing (no value ever written) |
        // | -30  | host context lacked a schema (defensive; shouldn't happen) |
        // | -31  | field name not declared in the contract's schema |
        // | -32  | host-fn arity doesn't match the declared field kind |
        // | -33  | key byte length doesn't match declared key type |
        // | -34  | value byte length doesn't match declared value type |
        //
        // Gas: same as the raw `sstore`/`sload`/`sdelete` plus a 20-gas
        // surcharge for schema lookup + type validation.

        // ── Scalar fields (no keys) ──────────────────────────────────────

        /// Write `value_ptr[..value_len]` to the declared scalar field
        /// `field_ptr[..field_len]`. The host derives the slot as
        /// `Poseidon2(self_address || field_name)` and verifies the value
        /// byte length matches the field's declared type.
        pub fn sstore_scalar(
            field_ptr: *const u8,
            field_len: i32,
            value_ptr: *const u8,
            value_len: i32,
        ) -> i32;

        /// Read a scalar field. Writes up to `out_max_len` bytes from the
        /// slot's value into `out_ptr`. Returns the actual value length
        /// (so callers can detect truncation) or `-1` if the slot has
        /// never been written.
        pub fn sload_scalar(
            field_ptr: *const u8,
            field_len: i32,
            out_ptr: *mut u8,
            out_max_len: i32,
        ) -> i32;

        /// Delete a scalar field's value. Subsequent `sload_scalar`
        /// returns `-1`.
        pub fn sdelete_scalar(field_ptr: *const u8, field_len: i32) -> i32;

        // ── Map fields (1 key) ───────────────────────────────────────────

        /// Write to a 1-key map field. Slot derives as
        /// `Poseidon2(self_address || field_name || key)`.
        pub fn sstore_map1(
            field_ptr: *const u8,
            field_len: i32,
            key_ptr: *const u8,
            key_len: i32,
            value_ptr: *const u8,
            value_len: i32,
        ) -> i32;

        /// Read from a 1-key map field. Same semantics as `sload_scalar`
        /// but slot derivation includes the key.
        pub fn sload_map1(
            field_ptr: *const u8,
            field_len: i32,
            key_ptr: *const u8,
            key_len: i32,
            out_ptr: *mut u8,
            out_max_len: i32,
        ) -> i32;

        /// Delete a 1-key map entry.
        pub fn sdelete_map1(
            field_ptr: *const u8,
            field_len: i32,
            key_ptr: *const u8,
            key_len: i32,
        ) -> i32;

        // ── Map fields (2 keys) ──────────────────────────────────────────

        /// Write to a 2-key map field (e.g. `allowances[owner][spender]`).
        /// Slot derives as
        /// `Poseidon2(self_address || field_name || key1 || key2)`.
        pub fn sstore_map2(
            field_ptr: *const u8,
            field_len: i32,
            k1_ptr: *const u8,
            k1_len: i32,
            k2_ptr: *const u8,
            k2_len: i32,
            value_ptr: *const u8,
            value_len: i32,
        ) -> i32;

        /// Read from a 2-key map field.
        pub fn sload_map2(
            field_ptr: *const u8,
            field_len: i32,
            k1_ptr: *const u8,
            k1_len: i32,
            k2_ptr: *const u8,
            k2_len: i32,
            out_ptr: *mut u8,
            out_max_len: i32,
        ) -> i32;

        /// Delete a 2-key map entry.
        pub fn sdelete_map2(
            field_ptr: *const u8,
            field_len: i32,
            k1_ptr: *const u8,
            k1_len: i32,
            k2_ptr: *const u8,
            k2_len: i32,
        ) -> i32;

        // ── Map fields (3 keys) ──────────────────────────────────────────

        /// Write to a 3-key map field. Slot derives as
        /// `Poseidon2(self_address || field_name || k1 || k2 || k3)`.
        pub fn sstore_map3(
            field_ptr: *const u8,
            field_len: i32,
            k1_ptr: *const u8,
            k1_len: i32,
            k2_ptr: *const u8,
            k2_len: i32,
            k3_ptr: *const u8,
            k3_len: i32,
            value_ptr: *const u8,
            value_len: i32,
        ) -> i32;

        /// Read from a 3-key map field.
        pub fn sload_map3(
            field_ptr: *const u8,
            field_len: i32,
            k1_ptr: *const u8,
            k1_len: i32,
            k2_ptr: *const u8,
            k2_len: i32,
            k3_ptr: *const u8,
            k3_len: i32,
            out_ptr: *mut u8,
            out_max_len: i32,
        ) -> i32;

        /// Delete a 3-key map entry.
        pub fn sdelete_map3(
            field_ptr: *const u8,
            field_len: i32,
            k1_ptr: *const u8,
            k1_len: i32,
            k2_ptr: *const u8,
            k2_len: i32,
            k3_ptr: *const u8,
            k3_len: i32,
        ) -> i32;

        // ─────────────────────────────────────────────────────────────────
        // §7.2 Account & balance
        // ─────────────────────────────────────────────────────────────────

        /// Read another account's native-PYDE balance.
        ///
        /// `addr_ptr`: pointer to a 32-byte address.
        /// `balance_out_ptr`: pointer to a 16-byte buffer (u128 LE).
        /// gas: 100 base.
        pub fn balance(addr_ptr: *const u8, balance_out_ptr: *mut u8);

        /// Transfer native PYDE to another account from this contract's
        /// balance.
        ///
        /// `to_ptr`: 32-byte recipient.
        /// `amount_ptr`: pointer to a 16-byte u128 amount (LE).
        /// gas: 7,000 base. Reverts with `ERR_INSUFFICIENT_BALANCE` if
        /// caller's balance < amount.
        pub fn transfer(to_ptr: *const u8, amount_ptr: *const u8) -> i32;

        // ─────────────────────────────────────────────────────────────────
        // §7.3 Execution context
        // ─────────────────────────────────────────────────────────────────

        /// Caller of THIS function (the immediate caller). For top-level
        /// transactions, equal to `origin`. For nested `cross_call`s,
        /// the calling contract.
        ///
        /// gas: 5 base.
        pub fn caller(addr_out_ptr: *mut u8) -> i32;

        /// Originator of the transaction — the externally-owned account
        /// that signed the tx, regardless of call nesting. Use sparingly:
        /// `tx.origin` checks are the source of the classic phishing
        /// footgun. Prefer `caller()` for authorization.
        ///
        /// gas: 5 base.
        pub fn origin(addr_out_ptr: *mut u8) -> i32;

        /// This contract's own address. Use as the first input to
        /// `hash_poseidon2` when deriving storage slots — keeps each
        /// contract's storage namespaced and collision-free.
        ///
        /// gas: 5 base.
        pub fn self_address(addr_out_ptr: *mut u8) -> i32;

        /// Current wave id (u64). Pyde's consensus-round counter,
        /// monotonically increasing.
        ///
        /// gas: 2 base.
        pub fn wave_id() -> i64;

        /// Canonical timestamp of the wave being committed, in seconds
        /// since Unix epoch. Committee-attested, identical across all
        /// validators. Use this instead of `std::time::SystemTime`
        /// (which doesn't exist in `no_std` WASM anyway).
        ///
        /// gas: 2 base.
        pub fn wave_timestamp() -> i64;

        /// Chain identifier (1 = mainnet, 31337 = devnet).
        ///
        /// gas: 2 base.
        pub fn chain_id() -> i64;

        // ─────────────────────────────────────────────────────────────────
        // §7.4 Transaction context
        // ─────────────────────────────────────────────────────────────────

        /// 32-byte Blake3 hash of the currently-executing transaction.
        ///
        /// gas: 5 base.
        pub fn tx_hash(hash_out_ptr: *mut u8);

        /// PYDE value attached to the current call (u128 LE in 16 bytes).
        /// Always zero for non-`payable` functions.
        ///
        /// gas: 5 base.
        pub fn tx_value(value_out_ptr: *mut u8);

        /// Remaining gas (fuel) in the current call frame.
        ///
        /// gas: 2 base.
        pub fn tx_gas_remaining() -> i64;

        /// Total byte-length of this invocation's calldata buffer.
        ///
        /// gas: 2 base.
        pub fn calldata_size() -> i32;

        /// Copy calldata bytes into `out_ptr`, capped by the
        /// little-endian `u32` limit at `out_len_ptr`. The host
        /// writes the actual length back to `out_len_ptr` (4 bytes
        /// LE) and returns the same value as i32. Pattern:
        ///
        /// ```ignore
        /// let n = unsafe { pyde::calldata_size() } as usize;
        /// let mut buf = vec![0u8; n];
        /// let mut limit = (n as u32).to_le_bytes();
        /// unsafe { pyde::calldata_copy(buf.as_mut_ptr(), limit.as_mut_ptr()); }
        /// ```
        ///
        /// gas: 1 base + 1 per byte copied. The host gates against
        /// over-large limits internally; contracts that need to
        /// partially read can pass a smaller limit.
        pub fn calldata_copy(out_ptr: *mut u8, out_len_ptr: *mut u8) -> i32;

        // ─────────────────────────────────────────────────────────────────
        // §7.5 Events
        // ─────────────────────────────────────────────────────────────────

        /// Append an event log entry to the transaction receipt.
        ///
        /// `topics_count`: 1..=4. Topic[0] is conventionally
        /// `Blake3(canonical_event_signature)`. Indexed fields go in
        /// topics[1..]; non-indexed payload goes in `data`.
        ///
        /// gas: 100 base + 50 × topics_count + 8 per data byte.
        pub fn emit_event(
            topics_ptr: *const u8,
            topics_count: i32,
            data_ptr: *const u8,
            data_len: i32,
        ) -> i32;

        // ─────────────────────────────────────────────────────────────────
        // §7.6 Hashing primitives
        // ─────────────────────────────────────────────────────────────────

        /// Blake3 hash. Use for general-purpose hashing — address
        /// derivation, event topic-0, content-addressed storage.
        ///
        /// gas: 15 base + 3 per word (8 bytes).
        pub fn hash_blake3(in_ptr: *const u8, in_len: i32, out_ptr: *mut u8);

        /// Poseidon2 hash. ZK-friendly but more expensive than Blake3 in
        /// native execution. **Pyde's canonical storage slot derivation**:
        /// `slot = Poseidon2(self_address || field || key)`.
        ///
        /// gas: 100 base + 30 per word.
        pub fn hash_poseidon2(in_ptr: *const u8, in_len: i32, out_ptr: *mut u8);

        /// Keccak256 hash. Provided for cross-chain interop (verifying
        /// Ethereum Merkle Patricia proofs, etc.). Pyde itself doesn't
        /// use Keccak natively.
        ///
        /// gas: 30 base + 6 per word.
        pub fn hash_keccak256(in_ptr: *const u8, in_len: i32, out_ptr: *mut u8);

        // ─────────────────────────────────────────────────────────────────
        // §7.7 Post-quantum cryptography
        // ─────────────────────────────────────────────────────────────────

        /// Verify a FALCON-512 signature. Pyde uses FALCON for tx
        /// signatures + on-chain identity. This host fn lets contracts
        /// verify additional FALCON sigs (e.g., off-chain authorizations,
        /// proofs of message authorship).
        ///
        /// `pk_ptr`: pointer to ~897 bytes (FALCON-512 public key).
        /// `msg_ptr` / `msg_len`: arbitrary message.
        /// `sig_ptr` / `sig_len`: signature bytes (variable, ~660-690).
        ///
        /// Returns 0 if valid, `ERR_SIGNATURE_INVALID` otherwise.
        pub fn falcon_verify(
            pk_ptr: *const u8,
            msg_ptr: *const u8,
            msg_len: i32,
            sig_ptr: *const u8,
            sig_len: i32,
        ) -> i32;

        // ─────────────────────────────────────────────────────────────────
        // §7.8 Cross-contract calls
        // ─────────────────────────────────────────────────────────────────

        /// Synchronous call into another contract.
        ///
        /// gas: 1,000 base + 8 per calldata byte + sub-call's gas_used.
        /// Sub-call runs in a nested overlay — its state changes merge
        /// on success or roll back on revert.
        pub fn cross_call(
            target_ptr: *const u8,
            fn_name_ptr: *const u8,
            fn_name_len: i32,
            calldata_ptr: *const u8,
            calldata_len: i32,
            value_ptr: *const u8,
            gas_limit: i64,
            return_data_out_ptr: *mut u8,
            return_data_out_len_ptr: *mut i32,
        ) -> i32;

        /// View-only variant of `cross_call`. Target must be a `view`
        /// function (engine returns `ERR_FORBIDDEN` otherwise). Sub-call
        /// is FREE for the caller — see HOST_FN_ABI_SPEC §7.8 "View calls
        /// are free."
        ///
        /// gas: 50 base for the dispatch only.
        pub fn cross_call_static(
            target_ptr: *const u8,
            fn_name_ptr: *const u8,
            fn_name_len: i32,
            calldata_ptr: *const u8,
            calldata_len: i32,
            gas_limit: i64,
            return_data_out_ptr: *mut u8,
            return_data_out_len_ptr: *mut i32,
        ) -> i32;

        /// Execute target's code in THIS contract's storage context.
        /// Used by proxies / upgradeable contracts. See HOST_FN_ABI_SPEC
        /// §7.8 for the security model — target's code can corrupt our
        /// storage if its slot layout diverges.
        ///
        /// gas: 1,200 base + 8 per calldata byte + sub-call gas_used.
        pub fn delegate_call(
            target_ptr: *const u8,
            fn_name_ptr: *const u8,
            fn_name_len: i32,
            calldata_ptr: *const u8,
            calldata_len: i32,
            gas_limit: i64,
            return_data_out_ptr: *mut u8,
            return_data_out_len_ptr: *mut i32,
        ) -> i32;

        // ─────────────────────────────────────────────────────────────────
        // §7.9 Halt operations
        // ─────────────────────────────────────────────────────────────────

        /// Set this call's return data and exit successfully. Useful for
        /// functions that return variable-length data (the WASM ABI
        /// return value is a single primitive; this lets you "return"
        /// bytes via the caller's `return_data_out` buffer).
        ///
        /// Note: this is the host fn named "return" in the spec.
        /// Renamed here because `return` is a Rust keyword.
        #[link_name = "return"]
        pub fn pyde_return(data_ptr: *const u8, data_len: i32) -> !;

        /// Revert the current call frame. All state changes since the
        /// call started are discarded. The reason bytes surface as the
        /// failure payload to the caller (or to the transaction receipt
        /// if this is the top-level call).
        pub fn revert(reason_ptr: *const u8, reason_len: i32) -> !;

        // ─────────────────────────────────────────────────────────────────
        // §7.10 Explicit gas metering
        // ─────────────────────────────────────────────────────────────────

        /// Charge `amount` units of gas explicitly. Used by contracts
        /// that perform off-fuel work (synchronous loops bounded by
        /// external data) and want the cost visible in receipts.
        ///
        /// gas: 2 base + `amount`.
        pub fn consume_gas(amount: i64);

        // ─────────────────────────────────────────────────────────────────
        // §7.11 VRF beacon
        // ─────────────────────────────────────────────────────────────────

        /// Current wave's committee-derived VRF beacon (32 bytes).
        /// Deterministic, public randomness. NB: publicly predictable
        /// within a wave, so do not use it where you need randomness an
        /// adversary cannot anticipate.
        ///
        /// gas: 50 base.
        pub fn beacon_get(out_ptr: *mut u8) -> i32;

        // ─────────────────────────────────────────────────────────────────
        // §7.12 Factory instantiation
        // ─────────────────────────────────────────────────────────────────

        /// Create a child instance of the DEPLOYED template contract at
        /// `template_addr_ptr` (32 bytes), addressed by
        /// `child_address(self, template, salt)` — by reference: the
        /// child shares the template's cached code, nothing is copied
        /// or recompiled. `salt_ptr` → 32 opaque caller-derived bytes;
        /// `init_*` → borsh ctor args (≤ 16 384 bytes; empty for
        /// ctor-less templates); `value_ptr` → 16-byte LE u128
        /// endowment; `gas_limit` < 0 = forward all remaining.
        ///
        /// `child_addr_out_ptr` (32 bytes) is written on every path
        /// past the early cap/bounds checks (0, -40, -43, -44, -45,
        /// -46, -3 — NOT -48). Return data carries the ctor's return
        /// value on 0 and its revert payload VERBATIM on -40.
        ///
        /// Returns 0, or: -40 ctor reverted (ATOMIC refund — no child,
        /// endowment back); -43 template not a contract; -44 child
        /// address occupied by a NON-mergeable account (balance-only
        /// EOA shells merge instead); -45 nonempty init on a ctor-less
        /// template; -46 PIP-2 prefix collision; -48 per-tx cap (64);
        /// -3 balance < endowment. Traps from view/static frames and
        /// at depth ≥ 1024. Prefer the [`crate::prepare`] builder.
        ///
        /// gas: 20 000 base + 8 per init byte + the ctor's own gas.
        pub fn instantiate(
            template_addr_ptr: *const u8,
            salt_ptr: *const u8,
            init_calldata_ptr: *const u8,
            init_calldata_len: i32,
            value_ptr: *const u8,
            gas_limit: i64,
            child_addr_out_ptr: *mut u8,
            return_data_out_ptr: *mut u8,
            return_data_out_len_ptr: *mut u32,
        ) -> i32;
    }
} // end pub mod raw

// ─────────────────────────────────────────────────────────────────────
// Ergonomic wrappers — safe, idiomatic surfaces over `raw::*`
// ─────────────────────────────────────────────────────────────────────
//
// These are the surfaces contract authors should reach for first. Each
// one allocates the output buffer, calls into the matching raw host
// fn, and returns the decoded value. Zero overhead: in release builds
// the wrapper inlines into the same instruction stream as the manual
// extern call.
//
// Sub-module breakdown:
//
// | Module / fn               | Replaces                                  |
// |---------------------------|-------------------------------------------|
// | `ctx::caller()`           | `let mut a = [0;32]; unsafe { raw::caller(a.as_mut_ptr()); }` |
// | `ctx::self_address()`     | same pattern                              |
// | `ctx::origin()`           | same pattern                              |
// | `ctx::tx_hash()`          | same pattern                              |
// | `ctx::tx_value()`         | 16-byte buffer + LE decode                |
// | `ctx::wave_id()` etc.     | i64-cast at call site                     |
// | `calldata::read()`        | size probe + alloc + copy                 |
// | `hash::blake3(&[u8])`     | input ptr + output ptr                    |
// | `emit_event(topics, data)`| manual topics-buffer assembly             |
// | `revert(&str)`            | bytes form                                |
// | `return_(&[u8])`          | bytes form                                |

extern crate alloc;

/// Execution + transaction context.
pub mod ctx {
    use super::{Address, Hash32};

    /// Caller of the current call frame (the immediate caller).
    /// For top-level transactions this equals `origin()`.
    #[inline]
    #[must_use]
    pub fn caller() -> Address {
        let mut a: Address = [0u8; 32];
        unsafe {
            crate::raw::caller(a.as_mut_ptr());
        }
        a
    }

    /// The currently-executing contract's own address.
    #[inline]
    #[must_use]
    pub fn self_address() -> Address {
        let mut a: Address = [0u8; 32];
        unsafe {
            crate::raw::self_address(a.as_mut_ptr());
        }
        a
    }

    /// Originator of the transaction — the externally-owned
    /// account that signed it, regardless of call nesting.
    /// Use sparingly: `tx.origin` checks are the source of the
    /// classic phishing footgun. Prefer [`caller`] for
    /// authorization.
    #[inline]
    #[must_use]
    pub fn origin() -> Address {
        let mut a: Address = [0u8; 32];
        unsafe {
            crate::raw::origin(a.as_mut_ptr());
        }
        a
    }

    /// Hash of the currently-executing transaction.
    #[inline]
    #[must_use]
    pub fn tx_hash() -> Hash32 {
        let mut h: Hash32 = [0u8; 32];
        unsafe {
            crate::raw::tx_hash(h.as_mut_ptr());
        }
        h
    }

    /// PYDE value attached to the current call (u128). Always
    /// zero for non-`payable` functions.
    #[inline]
    #[must_use]
    pub fn tx_value() -> u128 {
        let mut b = [0u8; 16];
        unsafe {
            crate::raw::tx_value(b.as_mut_ptr());
        }
        u128::from_le_bytes(b)
    }

    /// Gas remaining in the current call frame.
    #[inline]
    #[must_use]
    pub fn tx_gas_remaining() -> u64 {
        // The raw extern returns i64; the chain never produces a
        // negative gas value, so the cast is lossless.
        unsafe { crate::raw::tx_gas_remaining() as u64 }
    }

    /// Current wave id — Pyde's monotonically-increasing
    /// consensus-round counter.
    #[inline]
    #[must_use]
    pub fn wave_id() -> u64 {
        unsafe { crate::raw::wave_id() as u64 }
    }

    /// Canonical wave timestamp, seconds since Unix epoch.
    /// Committee-attested, identical across all validators.
    #[inline]
    #[must_use]
    pub fn wave_timestamp() -> u64 {
        unsafe { crate::raw::wave_timestamp() as u64 }
    }

    /// Chain id (1 = mainnet, 31337 = devnet).
    #[inline]
    #[must_use]
    pub fn chain_id() -> u64 {
        unsafe { crate::raw::chain_id() as u64 }
    }

    /// Current wave's committee-derived VRF beacon (32 bytes).
    /// Deterministic, publicly predictable within the wave.
    #[inline]
    #[must_use]
    pub fn beacon() -> Hash32 {
        let mut h: Hash32 = [0u8; 32];
        unsafe {
            crate::raw::beacon_get(h.as_mut_ptr());
        }
        h
    }
}

/// Calldata helpers — wraps the size-probe + read pattern.
pub mod calldata {
    use alloc::vec;
    use alloc::vec::Vec;

    /// Byte length of the current invocation's calldata.
    #[inline]
    #[must_use]
    pub fn size() -> usize {
        unsafe { crate::raw::calldata_size() as usize }
    }

    /// Read the entire calldata buffer into an owned `Vec<u8>`.
    /// Use when you want the bytes for borsh decoding; for stream
    /// reading, fall through to `raw::calldata_copy`.
    #[inline]
    #[must_use]
    pub fn read() -> Vec<u8> {
        let n = size();
        let mut buf = vec![0u8; n];
        if n > 0 {
            let mut limit = (n as u32).to_le_bytes();
            unsafe {
                crate::raw::calldata_copy(buf.as_mut_ptr(), limit.as_mut_ptr());
            }
        }
        buf
    }
}

/// Hashing primitives.
pub mod hash {
    use super::Hash32;

    /// Blake3 hash of the input bytes.
    #[inline]
    #[must_use]
    pub fn blake3(input: &[u8]) -> Hash32 {
        let mut out: Hash32 = [0u8; 32];
        unsafe {
            crate::raw::hash_blake3(input.as_ptr(), input.len() as i32, out.as_mut_ptr());
        }
        out
    }

    /// Poseidon2 hash. Use for hash chains and slot derivation
    /// when you need a hash that ZK proofs can verify cheaply.
    #[inline]
    #[must_use]
    pub fn poseidon2(input: &[u8]) -> Hash32 {
        let mut out: Hash32 = [0u8; 32];
        unsafe {
            crate::raw::hash_poseidon2(input.as_ptr(), input.len() as i32, out.as_mut_ptr());
        }
        out
    }

    /// Keccak256 hash. Provided for EVM-shaped interop (Merkle
    /// proofs against an Ethereum bridge, etc.) — for new code,
    /// prefer [`blake3`] or [`poseidon2`].
    #[inline]
    #[must_use]
    pub fn keccak256(input: &[u8]) -> Hash32 {
        let mut out: Hash32 = [0u8; 32];
        unsafe {
            crate::raw::hash_keccak256(input.as_ptr(), input.len() as i32, out.as_mut_ptr());
        }
        out
    }
}

/// Emit a log event from the current call frame.
///
/// `topics` carries 0..=4 32-byte indexed topics; the chain
/// indexes events by `topics[0]` (conventionally
/// `Blake3(event_signature)`) so subscribers can filter cheaply.
/// `data` is the opaque non-indexed payload — borsh-encoded
/// for typed events, raw bytes for low-level emitters.
///
/// For typed events declared in `otigen.toml`, the
/// [`declare_events!()`](crate::declare_events) macro generates
/// a struct + `emit()` impl that fills in the topic[0] hash for
/// you — prefer that over calling here directly.
#[inline]
pub fn emit_event(topics: &[Hash32], data: &[u8]) {
    // 4 topics × 32 bytes = 128 bytes max. Stack-allocate to keep
    // the alloc-free hot path; contracts emit thousands of events
    // a tx in worst-case workloads.
    debug_assert!(topics.len() <= 4, "emit_event: max 4 topics");
    let mut buf = [0u8; 128];
    for (i, t) in topics.iter().enumerate() {
        buf[i * 32..i * 32 + 32].copy_from_slice(t);
    }
    let topics_ptr = if topics.is_empty() {
        core::ptr::null()
    } else {
        buf.as_ptr()
    };
    unsafe {
        crate::raw::emit_event(
            topics_ptr,
            topics.len() as i32,
            data.as_ptr(),
            data.len() as i32,
        );
    }
}

/// Revert the current call frame with a UTF-8 message.
///
/// All state changes since the call started are discarded. The
/// message bytes surface as the failure payload to the caller
/// (or to the receipt for top-level calls). This function does
/// not return.
#[inline]
pub fn revert(msg: &str) -> ! {
    unsafe { crate::raw::revert(msg.as_ptr(), msg.len() as i32) }
}

/// Return `data` from the current call frame. The bytes flow
/// into the receipt's `return_data` for top-level calls, and
/// into the caller's return slot for `cross_call`. Does not
/// return.
#[inline]
pub fn return_(data: &[u8]) -> ! {
    unsafe { crate::raw::pyde_return(data.as_ptr(), data.len() as i32) }
}

/// Charge `amount` units of gas explicitly. Useful for surfacing
/// off-fuel computation cost (synchronous loops bounded by
/// external data) in receipts.
#[inline]
pub fn consume_gas(amount: u64) {
    unsafe { crate::raw::consume_gas(amount as i64) }
}

/// Cross-contract calls with `Result<T, CallError>` ergonomics.
///
/// The raw `pyde::raw::cross_call` host fn returns an `i32` status
/// code + writes the callee's return bytes into a contract-owned
/// buffer. This module wraps that surface so authors get typed
/// failure modes (rather than guessing what `-10` vs `-13` means)
/// and typed return decoding (the generic `<T: BorshDeserialize>`
/// auto-decodes the callee's `pyde::return_(bytes)` payload).
///
/// ## Why this matters
///
/// Ethereum's model: callee revert → caller's whole frame reverts
/// silently unless wrapped in Solidity 0.6+'s `try { ... } catch
/// { ... }`. Authors forget to wrap, and the "silent propagate"
/// surface has driven real losses.
///
/// Pyde's model (via this wrapper): cross-call returns
/// `Result<T, CallError>`. The Rust type system enforces
/// handling — you can't accidentally ignore a `Result` without
/// an explicit `.unwrap()` / `let _ =`. The failure variant
/// surfaces *why* the call failed (insufficient balance,
/// non-payable target, callee revert with message, ...) so the
/// caller can pick a recovery strategy.
///
/// ```ignore
/// match nft.call::<bool>("transfer_from", &calldata) {
///     Ok(true)  => { /* token moved */ }
///     Ok(false) => { /* contract returned false — paused, etc. */ }
///     Err(CallError::Reverted(bytes)) => {
///         // Revert message bytes — decode + handle, or propagate.
///         pyde::revert(core::str::from_utf8(&bytes).unwrap_or("nft call reverted"));
///     }
///     Err(CallError::InvalidFunction) => {
///         pyde::revert("target doesn't expose transfer_from");
///     }
///     Err(_) => {
///         pyde::revert("nft call failed");
///     }
/// }
/// ```
///
/// ## Buffer sizing
///
/// `execute` / `execute_with_value` / `execute_static` allocate a
/// 4 KB return-data buffer up-front. If the callee's return
/// exceeds that, the wrapper returns
/// `CallError::ReturnDataTooLarge { actual }` rather than
/// re-executing the callee (which would double the side effects).
/// Authors with larger returns can drop down to `pyde::raw::
/// cross_call` and size their own buffer.
pub mod call {
    extern crate alloc;

    use alloc::vec;
    use alloc::vec::Vec;
    use borsh::BorshDeserialize;

    use super::Address;

    /// Default size for the return-data buffer the wrapper
    /// allocates on the stack-equivalent (heap, technically —
    /// `Vec` allocates via `dlmalloc`). 4 KB covers every
    /// common token-shape return; larger payloads should
    /// drop to `pyde::raw::cross_call` and size their own
    /// buffer (or stage data through storage).
    pub const DEFAULT_RETURN_BUFFER_BYTES: usize = 4096;

    /// Forward the parent frame's full remaining gas to the
    /// callee. The chain still caps at the parent's actual
    /// remaining, so passing `i64::MAX` is the canonical "use
    /// whatever's left" signal.
    pub const FORWARD_ALL_GAS: i64 = i64::MAX;

    /// Why a cross-call failed, in typed form. Matches the
    /// chain's i32 error codes in `HOST_FN_ABI_SPEC` §7.4 +
    /// surfaces the callee's revert payload when applicable.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum CallError {
        /// The caller's balance can't cover the attached `value`.
        /// Chain status code: -3.
        InsufficientBalance,
        /// Target frame `(target_address, function_name)` is
        /// already on the call stack and the target function
        /// isn't `REENTRANT`. Chain status code: -9.
        ReentrancyBlocked,
        /// Callee trapped (unreachable, out-of-gas, host-fn
        /// error) or called `pyde::revert(msg)`. The `Vec<u8>`
        /// is the revert payload — whatever bytes the callee
        /// passed to `revert` — or empty on a generic trap.
        /// Chain status code: -10.
        Reverted(Vec<u8>),
        /// Caller attached `value > 0` against a target function
        /// that isn't `payable`. Chain status code: -12.
        NonPayable,
        /// Callee's function name didn't resolve in the target's
        /// deployed ABI. Chain status code: -13.
        InvalidFunction,
        /// Callee returned bytes that don't decode as the
        /// caller's expected type `T`. Distinct from
        /// `Reverted` — the callee succeeded but produced a
        /// shape mismatch.
        DecodeError,
        /// Callee returned more bytes than the default 4 KB
        /// buffer can hold. The `actual` field tells you how
        /// much; drop to `pyde::raw::cross_call` and size your
        /// own buffer to recover.
        ReturnDataTooLarge {
            /// Actual byte length the callee returned.
            actual: usize,
        },
    }

    impl CallError {
        /// If the callee reverted with a UTF-8 message, decode
        /// it. Returns `None` for non-revert failure modes or
        /// for revert bytes that aren't valid UTF-8.
        #[must_use]
        pub fn revert_message(&self) -> Option<alloc::string::String> {
            match self {
                CallError::Reverted(b) => alloc::string::String::from_utf8(b.clone()).ok(),
                _ => None,
            }
        }
    }

    fn err_from_status(status: i32) -> Option<CallError> {
        match status {
            0 => None,
            -3 => Some(CallError::InsufficientBalance),
            -9 => Some(CallError::ReentrancyBlocked),
            -10 => Some(CallError::Reverted(Vec::new())), // bytes filled below
            -12 => Some(CallError::NonPayable),
            -13 => Some(CallError::InvalidFunction),
            _ => Some(CallError::Reverted(Vec::new())),
        }
    }

    /// Cross-contract call to `target` with no PYDE value attached.
    ///
    /// Returns the callee's borsh-decoded return value (typed via
    /// `T`), or a typed failure mode.
    ///
    /// `T = ()` works for entries that declare `outputs = []` —
    /// borsh's empty-tuple decoder accepts zero bytes.
    pub fn execute<T: BorshDeserialize>(
        target: &Address,
        function: &str,
        calldata: &[u8],
    ) -> Result<T, CallError> {
        execute_inner(target, function, calldata, 0, Mode::Standard)
    }

    /// Same as [`execute`] but transfers `value` PYDE alongside
    /// the call. The target function must be `payable`; otherwise
    /// the call fails with [`CallError::NonPayable`].
    pub fn execute_with_value<T: BorshDeserialize>(
        target: &Address,
        function: &str,
        calldata: &[u8],
        value: u128,
    ) -> Result<T, CallError> {
        execute_inner(target, function, calldata, value, Mode::Standard)
    }

    /// Static / read-only cross-call. The callee inherits the
    /// caller's `view_mode` flag, so any `sstore` / `sdelete` /
    /// `transfer` / `emit_event` inside the callee traps with
    /// `ERR_FORBIDDEN`. Use for view-fn fan-outs (price-oracle
    /// reads, registry lookups) where you want the chain to
    /// enforce read-only behavior.
    pub fn execute_static<T: BorshDeserialize>(
        target: &Address,
        function: &str,
        calldata: &[u8],
    ) -> Result<T, CallError> {
        execute_inner(target, function, calldata, 0, Mode::Static)
    }

    /// Delegate-call into `target` — runs the target's code in
    /// THIS contract's context. `self_address`, storage slots,
    /// `caller`, and `tx_value` all stay as they were in the
    /// parent frame; only the executed code switches. The
    /// canonical use case is upgradeable proxies: a thin proxy
    /// stores `logic_address` + delegate-calls everything else
    /// into the logic contract, so logic upgrades preserve the
    /// proxy's state.
    ///
    /// Failure modes are the same `CallError` taxonomy as
    /// [`execute`]; the chain's reentrancy guard treats a
    /// delegate-call as a frame on the call stack just like
    /// `cross_call`.
    pub fn execute_delegate<T: BorshDeserialize>(
        target: &Address,
        function: &str,
        calldata: &[u8],
    ) -> Result<T, CallError> {
        execute_inner(target, function, calldata, 0, Mode::Delegate)
    }

    /// Delegate-call into `target` and return the callee's raw
    /// `pyde::return_(...)` bytes verbatim — no borsh-decode.
    ///
    /// The proxy pattern: a thin contract receives any
    /// `forward(function, calldata)` call, delegate-calls into
    /// `logic`, and hands the bytes back to its own caller. The
    /// proxy itself can't know the return type at compile time
    /// (different logic functions return different shapes), so the
    /// typed [`execute_delegate`] wrapper is the wrong fit — it
    /// would re-interpret the logic's already-encoded bytes as a
    /// concrete `T`.
    ///
    /// Use [`execute_delegate`] for typed call sites that DO know
    /// the return shape (`logic.set_value(42) -> ()`,
    /// `logic.get_value() -> u64`). Use `execute_delegate_raw` for
    /// the type-erased forwarder layer.
    ///
    /// Failure modes match [`execute_delegate`]; on
    /// [`CallError::Reverted`] the chain has copied the logic's
    /// revert payload into the variant's `Vec<u8>` so the proxy
    /// can pass the bytes straight back via [`crate::revert`].
    pub fn execute_delegate_raw(
        target: &Address,
        function: &str,
        calldata: &[u8],
    ) -> Result<Vec<u8>, CallError> {
        execute_inner_raw(target, function, calldata, 0, Mode::Delegate)
    }

    /// Which sub-call variant the wrapper dispatches into.
    #[derive(Clone, Copy)]
    enum Mode {
        Standard,
        Static,
        Delegate,
    }

    fn execute_inner<T: BorshDeserialize>(
        target: &Address,
        function: &str,
        calldata: &[u8],
        value: u128,
        mode: Mode,
    ) -> Result<T, CallError> {
        let buf = execute_inner_raw(target, function, calldata, value, mode)?;
        T::try_from_slice(&buf).map_err(|_| CallError::DecodeError)
    }

    /// Shared inner: runs the cross-call, sizes the buffer,
    /// handles the truncation + revert-payload paths, returns the
    /// raw `pyde::return_(...)` bytes on success.
    fn execute_inner_raw(
        target: &Address,
        function: &str,
        calldata: &[u8],
        value: u128,
        mode: Mode,
    ) -> Result<Vec<u8>, CallError> {
        let mut buf: Vec<u8> = vec![0u8; DEFAULT_RETURN_BUFFER_BYTES];
        let mut len_bytes: [u8; 4] = (buf.len() as u32).to_le_bytes();

        let status = match mode {
            Mode::Static => unsafe {
                crate::raw::cross_call_static(
                    target.as_ptr(),
                    function.as_ptr(),
                    function.len() as i32,
                    calldata.as_ptr(),
                    calldata.len() as i32,
                    FORWARD_ALL_GAS,
                    buf.as_mut_ptr(),
                    len_bytes.as_mut_ptr() as *mut i32,
                )
            },
            Mode::Delegate => unsafe {
                crate::raw::delegate_call(
                    target.as_ptr(),
                    function.as_ptr(),
                    function.len() as i32,
                    calldata.as_ptr(),
                    calldata.len() as i32,
                    FORWARD_ALL_GAS,
                    buf.as_mut_ptr(),
                    len_bytes.as_mut_ptr() as *mut i32,
                )
            },
            Mode::Standard => {
                let value_bytes = value.to_le_bytes();
                unsafe {
                    crate::raw::cross_call(
                        target.as_ptr(),
                        function.as_ptr(),
                        function.len() as i32,
                        calldata.as_ptr(),
                        calldata.len() as i32,
                        value_bytes.as_ptr(),
                        FORWARD_ALL_GAS,
                        buf.as_mut_ptr(),
                        len_bytes.as_mut_ptr() as *mut i32,
                    )
                }
            }
        };

        // The host wrote the actual length back into the
        // `len_bytes` slot regardless of success/failure.
        let actual = u32::from_le_bytes(len_bytes) as usize;
        let truncated = actual > buf.len();

        if let Some(mut err) = err_from_status(status) {
            // For `Reverted`, the chain wrote the revert payload
            // into our buffer; copy it into the variant.
            if matches!(err, CallError::Reverted(_)) {
                let copy = actual.min(buf.len());
                let mut bytes = Vec::with_capacity(copy);
                bytes.extend_from_slice(&buf[..copy]);
                err = CallError::Reverted(bytes);
            }
            return Err(err);
        }

        if truncated {
            return Err(CallError::ReturnDataTooLarge { actual });
        }
        buf.truncate(actual);
        Ok(buf)
    }
}

// ─────────────────────────────────────────────────────────────────────
// Factory pattern — child-address derivation + salt helpers
// ─────────────────────────────────────────────────────────────────────
//
// A factory contract creates child instances of a deployed TEMPLATE
// contract via `pyde::instantiate` (by reference — the template's code
// is shared, never copied). The child's address is a pure function of
// `(parent, template, salt)`, so anyone — the factory on-chain, a
// wallet off-chain, an indexer — can compute where a child lives
// BEFORE it exists (counterfactual addressing, CREATE2-style but
// salt-only: Pyde has no creation nonce anywhere).
//
// This block is the Rust half of the ONE canonical derivation that the
// engine (`pyde_engine_account::child_address`), all four language
// bindings, both off-chain SDKs, and the CLI must reproduce
// byte-identically. The shared drift guard is `vectors/
// child_address.json` at the repo root — every implementation's CI
// replays the same golden set, anchored on the engine's pinned KAT.

/// Domain-separator prefix for child (factory-instantiated) addresses.
/// Exactly 11 bytes — keeps child addresses structurally disjoint from
/// every other address family (EOA, contract-name, system).
pub const CHILD_ADDRESS_PREFIX: &[u8; 11] = b"pyde-child:";

/// Byte length of the child-address preimage: `11 + 32 + 32 + 32`.
/// Fixed-width with no length prefixes or separators — required for
/// hash injectivity.
pub const CHILD_PREIMAGE_LEN: usize = 107;

/// Assemble the canonical 107-byte child-address preimage:
///
/// ```text
/// "pyde-child:" ‖ parent (32 B) ‖ template (32 B) ‖ salt (32 B)
/// ```
///
/// Pure byte assembly — no hashing, no host calls — so conformance
/// tests can pin the layout without a chain. Contracts normally want
/// [`child_address`] instead.
#[inline]
#[must_use]
pub fn child_preimage(
    parent: &Address,
    template: &Address,
    salt: &[u8; 32],
) -> [u8; CHILD_PREIMAGE_LEN] {
    let mut p = [0u8; CHILD_PREIMAGE_LEN];
    p[..11].copy_from_slice(CHILD_ADDRESS_PREFIX);
    p[11..43].copy_from_slice(parent);
    p[43..75].copy_from_slice(template);
    p[75..107].copy_from_slice(salt);
    p
}

/// Address of the child a factory would (or did) create for
/// `(template, salt)` — `Poseidon2` of [`child_preimage`].
///
/// - `parent` — the FACTORY's own address. On-chain, pass
///   [`ctx::self_address()`]; the engine derives from the executing
///   frame, so a contract can only ever mint into its OWN namespace.
/// - `template` — the deployed template contract's address (an
///   address, NOT a code hash).
/// - `salt` — opaque 32 bytes, caller-derived; see [`Salt`].
///
/// Same inputs → same address, forever. Hashes via the
/// `hash_poseidon2` host fn (cheap — see the spec's gas catalog),
/// which is the same Poseidon2 the engine's authoritative derivation
/// uses — the two cannot disagree.
#[inline]
#[must_use]
pub fn child_address(parent: &Address, template: &Address, salt: &[u8; 32]) -> Address {
    hash::poseidon2(&child_preimage(parent, template, salt))
}

/// Salt constructors for [`child_address`] / `pyde::instantiate`.
///
/// A salt is exactly 32 opaque bytes at the wire; deriving those bytes
/// is entirely the caller's job (the engine never interprets them).
/// Two disciplined patterns cover practically every factory:
///
/// - **Identity salt** — hash the child's identity so the address is
///   deterministic + counterfactual: [`Salt::of`] /
///   [`Salt::of_unordered_pair`]. One pool per token pair, one escrow
///   per (buyer, seller, id), ...
/// - **Sequential salt** — the factory keeps its own `u64` counter in
///   its own storage and hashes it: `Salt::of(&counter)`, increment
///   after. (NOT an engine nonce — Pyde has none.)
///
/// "Random" salts are an anti-pattern: deterministically-unique only,
/// or you lose counterfactual addressing and idempotent re-creation.
pub struct Salt;

impl Salt {
    /// Identity salt: `Poseidon2(borsh(value))`.
    ///
    /// `value` is any borsh-serializable identity — a tuple of ctor
    /// params, a counter, a string tag. Encodes ONCE as a single
    /// borsh value (for tuples: the frameless concatenation of the
    /// fields, exactly matching `#[pyde::entry]`'s calldata
    /// convention), then hashes via the `hash_poseidon2` host fn.
    #[inline]
    #[must_use]
    pub fn of<T: borsh::BorshSerialize>(value: &T) -> [u8; 32] {
        hash::poseidon2(&Self::encoding(value))
    }

    /// Identity salt for an UNORDERED address pair — the two
    /// addresses are sorted before encoding, so `(a, b)` and
    /// `(b, a)` yield the same salt (and therefore the same child).
    ///
    /// This is the UniswapV2 `sortTokens` footgun handled once, here:
    /// without sorting, the "same" pair passed in different orders
    /// silently forks into two different children.
    #[inline]
    #[must_use]
    pub fn of_unordered_pair(a: &Address, b: &Address) -> [u8; 32] {
        hash::poseidon2(&Self::unordered_pair_encoding(a, b))
    }

    /// The exact borsh bytes [`Salt::of`] hashes — exposed so
    /// off-chain implementations and the conformance vectors can
    /// reproduce salts byte-identically without a chain.
    #[inline]
    #[must_use]
    pub fn encoding<T: borsh::BorshSerialize>(value: &T) -> alloc::vec::Vec<u8> {
        borsh::to_vec(value).expect("salt: borsh encoding failed")
    }

    /// The exact bytes [`Salt::of_unordered_pair`] hashes: the two
    /// addresses in ascending byte order, concatenated (borsh of a
    /// fixed-size array pair = the raw 64 bytes, no framing).
    #[inline]
    #[must_use]
    pub fn unordered_pair_encoding(a: &Address, b: &Address) -> alloc::vec::Vec<u8> {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        Self::encoding(&(lo, hi))
    }
}

/// Why a `pyde::instantiate` failed, in typed form. Maps the chain's
/// i32 codes (HOST_FN_ABI_SPEC §7.12) + surfaces the payloads that
/// make each failure actionable.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstantiateError {
    /// The child's constructor reverted. The WHOLE instantiate
    /// unwound atomically — no child account, no child storage, the
    /// endowment refunded — so this costs gas only, never value.
    /// `payload` is the ctor's revert payload, verbatim.
    /// Chain code: -40.
    CtorReverted {
        /// The child ctor's revert payload (its `revert(msg)` bytes).
        payload: alloc::vec::Vec<u8>,
    },
    /// A NON-mergeable account already sits at the derived child
    /// address — this `(template, salt)` was already instantiated
    /// (or the address is otherwise taken). `addr` is that address,
    /// straight from the host fn's out-param: the idempotent-factory
    /// idiom is one match arm, zero re-derivation. Chain code: -44.
    Exists {
        /// The (occupied) derived child address.
        addr: Address,
    },
    /// The template address is not a code-bearing contract.
    /// Chain code: -43.
    TemplateNotFound,
    /// Non-empty init calldata against a template with no
    /// constructor. Ctor-less templates: don't call `.args()`.
    /// Chain code: -45.
    CtorMismatch,
    /// PIP-2 16-byte prefix-registry collision. Chain code: -46.
    PrefixCollision,
    /// `MAX_INSTANTIATES_PER_TX` (64) exceeded for this transaction.
    /// Chain code: -48.
    CapExceeded,
    /// The factory's balance can't cover the endowment.
    /// Chain code: -3.
    InsufficientBalance,
    /// A code this wrapper doesn't recognize (newer chain than SDK).
    Unexpected(
        /// The raw i32 status the host fn returned.
        i32,
    ),
}

impl InstantiateError {
    /// If the child ctor reverted with a UTF-8 message, decode it.
    #[must_use]
    pub fn revert_message(&self) -> Option<alloc::string::String> {
        match self {
            InstantiateError::CtorReverted { payload } => {
                alloc::string::String::from_utf8(payload.clone()).ok()
            }
            _ => None,
        }
    }
}

/// Start an [`Instantiate`] — create a child instance of the deployed
/// template contract at `template`, addressed by `salt`.
///
/// `prepare` is PURE: it only fills an in-memory builder — nothing
/// touches the chain until [`Instantiate::instantiate`] runs, so an
/// unfired builder is free. Configure with [`args`](Instantiate::args)
/// / [`value`](Instantiate::value) / [`gas`](Instantiate::gas)
/// (in-place, so conditional configuration reads naturally), then
/// execute:
///
/// ```ignore
/// // one-liner
/// let child = pyde::prepare(&template, &salt)
///     .args(&(token0, token1, fee_rate))
///     .value(1_000_000_000)
///     .instantiate()?;
///
/// // prepare now, decide later
/// let mut prep = pyde::prepare(&template, &salt);
/// prep.args(&(token0, token1));
/// if needs_funding {
///     prep.value(1_000_000_000);
/// }
/// let child = prep.instantiate()?;
///
/// // idempotent factory: -44 hands the address back
/// let pool = match pyde::prepare(&template, &salt).instantiate() {
///     Ok(addr) | Err(pyde::InstantiateError::Exists { addr }) => addr,
///     Err(e) => pyde::revert(e.revert_message().as_deref().unwrap_or("instantiate failed")),
/// };
/// ```
#[inline]
#[must_use]
pub fn prepare(template: &Address, salt: &[u8; 32]) -> Instantiate {
    Instantiate {
        template: *template,
        salt: *salt,
        init: alloc::vec::Vec::new(),
        value: 0,
        gas: FORWARD_ALL_CTOR_GAS,
    }
}

/// Forward the frame's full remaining gas to the child constructor —
/// `pyde::instantiate`'s convention is NEGATIVE = forward-all (unlike
/// `cross_call`'s `i64::MAX`).
pub const FORWARD_ALL_CTOR_GAS: i64 = -1;

/// A prepared-but-unfired `pyde::instantiate` — built by
/// [`prepare`], executed by [`Instantiate::instantiate`].
///
/// Owns everything it holds ([`args`](Self::args) encodes eagerly
/// into owned bytes), so it can be stored, moved across scopes, and
/// fired inside a branch.
#[derive(Debug, Clone)]
pub struct Instantiate {
    template: Address,
    salt: [u8; 32],
    init: alloc::vec::Vec<u8>,
    value: u128,
    gas: i64,
}

impl Instantiate {
    /// Constructor args as raw typed values — encoded EAGERLY into
    /// one owned borsh tuple (frameless field concatenation, exactly
    /// what the child's `#[pyde::entry]` ctor decodes). Pass the
    /// values themselves, never pre-encoded blobs (a `Vec<u8>`
    /// would get borsh's length prefix and corrupt the calldata).
    /// Ctor-less template? Don't call this at all (avoids -45).
    #[inline]
    pub fn args<T: borsh::BorshSerialize>(&mut self, args: &T) -> &mut Self {
        self.init = borsh::to_vec(args).expect("instantiate: borsh encoding failed");
        self
    }

    /// Escape hatch: pre-encoded init calldata, verbatim — for
    /// dynamic templates where the arg shape isn't known at compile
    /// time. Most factories want [`args`](Self::args).
    #[inline]
    pub fn raw_args(&mut self, init: &[u8]) -> &mut Self {
        self.init = init.to_vec();
        self
    }

    /// Endowment (quanta) to fund the child account with. Funds the
    /// ACCOUNT (like deploy value) — no payable gate; the ctor sees
    /// it via `pyde::ctx::tx_value()`. Refunded if the ctor reverts.
    #[inline]
    pub fn value(&mut self, quanta: u128) -> &mut Self {
        self.value = quanta;
        self
    }

    /// Gas budget for the child constructor. Default: forward all
    /// remaining ([`FORWARD_ALL_CTOR_GAS`]).
    #[inline]
    pub fn gas(&mut self, limit: i64) -> &mut Self {
        self.gas = limit;
        self
    }

    /// Execute the instantiate — the ONLY method that touches the
    /// chain. Returns the child's address; the child is live,
    /// endowed, and constructed. See [`InstantiateError`] for the
    /// failure taxonomy (all failures are atomic: no half-born
    /// children, endowment never lost).
    ///
    /// The ctor's RETURN VALUE (if any) is discarded — constructors
    /// rarely return data, and the address is the meaningful result.
    /// The rare caller that needs it drops to [`raw::instantiate`].
    pub fn instantiate(&self) -> Result<Address, InstantiateError> {
        use alloc::vec;

        let mut child: Address = [0u8; 32];
        let mut ret: alloc::vec::Vec<u8> = vec![0u8; call::DEFAULT_RETURN_BUFFER_BYTES];
        let mut ret_len: [u8; 4] = (ret.len() as u32).to_le_bytes();
        let value_bytes = self.value.to_le_bytes();

        let status = unsafe {
            raw::instantiate(
                self.template.as_ptr(),
                self.salt.as_ptr(),
                self.init.as_ptr(),
                self.init.len() as i32,
                value_bytes.as_ptr(),
                self.gas,
                child.as_mut_ptr(),
                ret.as_mut_ptr(),
                ret_len.as_mut_ptr() as *mut u32,
            )
        };

        match status {
            0 => Ok(child),
            -40 => {
                // Ctor revert payload was copied into our buffer
                // (truncated to capacity if oversized — the child is
                // gone either way, the payload is advisory).
                let actual = u32::from_le_bytes(ret_len) as usize;
                ret.truncate(actual.min(call::DEFAULT_RETURN_BUFFER_BYTES));
                Err(InstantiateError::CtorReverted { payload: ret })
            }
            -44 => Err(InstantiateError::Exists { addr: child }),
            -43 => Err(InstantiateError::TemplateNotFound),
            -45 => Err(InstantiateError::CtorMismatch),
            -46 => Err(InstantiateError::PrefixCollision),
            -48 => Err(InstantiateError::CapExceeded),
            -3 => Err(InstantiateError::InsufficientBalance),
            other => Err(InstantiateError::Unexpected(other)),
        }
    }
}

/// Conformance suite for the child-address derivation + salt helpers.
///
/// Everything here runs HOST-SIDE (no chain): the preimage assembly
/// and salt encodings are pure functions of this crate; the Poseidon2
/// step is mirrored via the reference `pyde-crypto` crate — the exact
/// implementation the engine's `hash_poseidon2` host fn binds, so a
/// pass here means the on-chain composition computes the same bytes.
///
/// The golden vectors live at `vectors/child_address.json` (repo
/// root) and are the SHARED artifact every language binding + SDK CI
/// replays. Regenerate with:
///
/// ```text
/// cargo test -p pyde-host regenerate_golden_vectors -- --ignored
/// ```
#[cfg(test)]
mod conformance {
    use std::string::String;
    use std::vec::Vec;
    use std::{format, vec};

    use super::{child_preimage, Salt, CHILD_ADDRESS_PREFIX, CHILD_PREIMAGE_LEN};

    /// Host-side mirror of the `hash_poseidon2` host fn — same crate,
    /// same version (`=`-pinned) the engine links.
    fn p2(data: &[u8]) -> [u8; 32] {
        pyde_crypto::poseidon2::poseidon2_hash(data).into()
    }

    /// Where the shared golden vectors live: `<repo>/vectors/`.
    fn vectors_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vectors/child_address.json")
    }

    // ── the typed truth the vectors are generated from ──────────────

    struct Case {
        name: &'static str,
        parent: [u8; 32],
        template: [u8; 32],
        /// `Some(borsh bytes)` for identity salts (`salt = Poseidon2(bytes)`);
        /// `None` for raw opaque salts.
        salt_source_borsh: Option<Vec<u8>>,
        /// Normative description of the typed value behind
        /// `salt_source_borsh` — so non-Rust implementers don't have to
        /// guess widths/shapes from the bytes (u64-vs-two-u32s etc.).
        source_typed: Option<&'static str>,
        /// For the unordered-pair case only: the two arguments AS
        /// PASSED (deliberately unsorted) — a replayer must implement
        /// the ascending-bytewise sort to reproduce the salt from
        /// these.
        pair_args: Option<([u8; 32], [u8; 32])>,
        salt: [u8; 32],
    }

    fn identity_case(
        name: &'static str,
        parent: [u8; 32],
        template: [u8; 32],
        source: Vec<u8>,
        typed: &'static str,
    ) -> Case {
        let salt = p2(&source);
        Case {
            name,
            parent,
            template,
            salt_source_borsh: Some(source),
            source_typed: Some(typed),
            pair_args: None,
            salt,
        }
    }

    fn cases() -> Vec<Case> {
        let incrementing: [u8; 32] = core::array::from_fn(|i| i as u8);
        let decrementing: [u8; 32] = core::array::from_fn(|i| 31 - i as u8);
        let raw = |name, parent, template, salt| Case {
            name,
            parent,
            template,
            salt_source_borsh: None,
            source_typed: None,
            pair_args: None,
            salt,
        };
        // The unordered-pair case: arguments recorded AS PASSED
        // (deliberately reversed) — the canonical encoding must come
        // out sorted ascending (the sortTokens footgun pin), and a
        // replayer must implement the sort to get from pair_args to
        // the salt.
        let (pair_a, pair_b) = ([0xBB; 32], [0xAA; 32]);
        let mut pair = identity_case(
            "salt-of-unordered-pair",
            [0x11; 32],
            [0x22; 32],
            Salt::unordered_pair_encoding(&pair_a, &pair_b),
            "of_unordered_pair(a, b): sort the two 32-byte values ascending \
             BYTEWISE (lexicographic), concatenate (raw 64 bytes, no framing), \
             hash. pair_args holds (a, b) as passed — unsorted on purpose.",
        );
        pair.pair_args = Some((pair_a, pair_b));
        // Sign-boundary pair: the differing byte crosses 0x7f/0x80,
        // where SIGNED and UNSIGNED byte order disagree (0x80 = -128
        // as i8 but 128 as u8). The 0xAA/0xBB pair above can't tell a
        // signed comparator from an unsigned one; this vector exists
        // to catch exactly that regression in any binding.
        let (sign_a, sign_b) = ([0x80; 32], [0x7F; 32]);
        let mut pair_sign = identity_case(
            "salt-of-unordered-pair-sign-boundary",
            [0x33; 32],
            [0x44; 32],
            Salt::unordered_pair_encoding(&sign_a, &sign_b),
            "of_unordered_pair(a=0x80 x 32, b=0x7f x 32): the sort is \
             UNSIGNED bytewise, so 0x7f sorts FIRST. A signed (i8) \
             comparator reverses these two — this vector exists to catch \
             exactly that.",
        );
        pair_sign.pair_args = Some((sign_a, sign_b));
        vec![
            // Raw opaque salts — pin the preimage assembly itself.
            raw("engine-kat-anchor", [0x11; 32], [0x22; 32], [0x33; 32]),
            raw("all-zero", [0x00; 32], [0x00; 32], [0x00; 32]),
            raw("all-max", [0xFF; 32], [0xFF; 32], [0xFF; 32]),
            // Asymmetric parent/template — catches field-order swaps
            // and byte-order flips that symmetric patterns can't.
            raw("field-order-guard", incrementing, decrementing, [0x55; 32]),
            // Identity salts — pin `Salt::of` (borsh → Poseidon2),
            // including the empty-input Poseidon2 edge case.
            identity_case(
                "salt-of-unit-empty-borsh",
                [0x11; 32],
                [0x22; 32],
                Salt::encoding(&()),
                "the unit value () — borsh encodes to ZERO bytes; pins the \
                 empty-input Poseidon2 rule (single zero field element, no \
                 length element)",
            ),
            identity_case(
                "salt-of-counter-0",
                [0xA1; 32],
                [0xB2; 32],
                Salt::encoding(&0u64),
                "0u64 — borsh: 8-byte little-endian (a u64 counter, NOT two \
                 u32s)",
            ),
            identity_case(
                "salt-of-counter-1",
                [0xA1; 32],
                [0xB2; 32],
                Salt::encoding(&1u64),
                "1u64 — borsh: 8-byte little-endian",
            ),
            identity_case(
                "salt-of-i128-neg-one",
                [0xC3; 32],
                [0xD4; 32],
                Salt::encoding(&(-1i128)),
                "-1i128 — borsh: 16-byte little-endian two's complement (16 \
                 x 0xff; deliberately byte-aliases u128::MAX — signedness is \
                 a TYPE property, not a wire property)",
            ),
            identity_case(
                "salt-of-i128-min",
                [0xC3; 32],
                [0xD4; 32],
                Salt::encoding(&i128::MIN),
                "i128::MIN — borsh: 16-byte little-endian two's complement \
                 (15 x 0x00 then 0x80)",
            ),
            identity_case(
                "salt-of-string",
                [0xE5; 32],
                [0xF6; 32],
                Salt::encoding(&"amm-pool-v1"),
                "the string \"amm-pool-v1\" — borsh string: u32 LE byte \
                 length (11) then the UTF-8 bytes",
            ),
            pair,
            pair_sign,
            identity_case(
                "salt-of-mixed-tuple",
                [0x77; 32],
                [0x88; 32],
                Salt::encoding(&([0xCC_u8; 32], 42_u64, true)),
                "the tuple ([0xcc; 32], 42u64, true) — borsh tuple is \
                 FRAMELESS field concatenation: raw 32 bytes, then 8-byte LE \
                 42, then 0x01 for true",
            ),
        ]
    }

    // ── JSON shape of the shared vector file ────────────────────────

    #[derive(serde::Serialize, serde::Deserialize)]
    struct VectorFile {
        #[serde(rename = "_meta")]
        meta: Meta,
        vectors: Vec<VectorEntry>,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Meta {
        description: String,
        derivation: String,
        poseidon2: String,
        salt_rule: String,
        anchor: String,
        regenerate: String,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct VectorEntry {
        name: String,
        parent: String,
        template: String,
        /// Normative description of the typed value the borsh bytes
        /// encode — so implementers never guess widths/shapes.
        #[serde(skip_serializing_if = "Option::is_none")]
        salt_source_typed: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        salt_source_borsh: Option<String>,
        /// Unordered-pair case only: the two args AS PASSED (unsorted);
        /// a replayer must sort ascending bytewise to reach the salt.
        #[serde(skip_serializing_if = "Option::is_none")]
        salt_source_pair_args: Option<[String; 2]>,
        salt: String,
        preimage: String,
        child_address: String,
    }

    fn render(case: &Case) -> VectorEntry {
        let preimage = child_preimage(&case.parent, &case.template, &case.salt);
        VectorEntry {
            name: case.name.into(),
            parent: hex::encode(case.parent),
            template: hex::encode(case.template),
            salt_source_typed: case.source_typed.map(String::from),
            salt_source_borsh: case.salt_source_borsh.as_ref().map(hex::encode),
            salt_source_pair_args: case
                .pair_args
                .map(|(a, b)| [hex::encode(a), hex::encode(b)]),
            salt: hex::encode(case.salt),
            preimage: hex::encode(preimage),
            child_address: hex::encode(p2(&preimage)),
        }
    }

    fn render_file() -> VectorFile {
        VectorFile {
            meta: Meta {
                description: "Golden conformance vectors for the Pyde factory \
                              child-address derivation. Every implementation (engine, 4 \
                              language bindings, rust/ts SDKs, CLI) must reproduce every \
                              vector byte-for-byte."
                    .into(),
                derivation: "child_address = Poseidon2(\"pyde-child:\" || parent[32] || \
                             template[32] || salt[32]); preimage is 107 bytes fixed-width, \
                             no length prefixes, no separators."
                    .into(),
                poseidon2: "pyde-crypto Poseidon2: PaddingFreeSponge over Goldilocks, \
                            WIDTH=8 RATE=4 OUT=4, HL round constants; input packed as \
                            7-byte LE chunks (one field element each) plus a trailing \
                            input-length element (empty input = single zero element, no \
                            length element); output = 4 canonical u64, little-endian, \
                            32 bytes."
                    .into(),
                salt_rule: "Vectors with salt_source_borsh are identity salts: salt = \
                            Poseidon2(salt_source_borsh), where salt_source_borsh is the \
                            borsh encoding of the typed value described in \
                            salt_source_typed (Salt::of). Vectors without it use the raw \
                            salt bytes verbatim. Unordered pairs: sort the two 32-byte \
                            values ascending BYTEWISE (lexicographic), then concatenate \
                            (raw 64 bytes, no framing) — salt_source_pair_args records \
                            the args deliberately UNSORTED so replaying from them \
                            requires implementing the sort."
                    .into(),
                anchor: "engine-kat-anchor reproduces the engine's pinned KAT in \
                         pyde-net/engine crates/account/src/address.rs \
                         (child_address_kat_pins_derivation)."
                    .into(),
                regenerate: "cargo test -p pyde-host regenerate_golden_vectors -- --ignored".into(),
            },
            vectors: cases().iter().map(render).collect(),
        }
    }

    // ── the tests ───────────────────────────────────────────────────

    /// `prepare` is pure and the builder owns its bytes: `.args`
    /// encodes EAGERLY, so the source values can die before
    /// `.instantiate()` fires (the store-then-fire-later pattern).
    #[test]
    fn builder_encodes_args_eagerly_and_owned() {
        let mut prep = super::prepare(&[0xAA; 32], &[0x11; 32]);
        {
            let tmp = ([0xCC_u8; 32], 42_u64, true);
            prep.args(&tmp);
        } // tmp dropped — the builder must hold owned bytes
        assert_eq!(prep.init, Salt::encoding(&([0xCC_u8; 32], 42_u64, true)));
        assert_eq!(prep.value, 0, "no endowment unless asked");
        assert_eq!(prep.gas, super::FORWARD_ALL_CTOR_GAS, "forward-all default");
        assert_eq!(prep.template, [0xAA; 32]);
        assert_eq!(prep.salt, [0x11; 32]);
    }

    /// Setters chain AND mutate in place, so conditional
    /// configuration (`if x { prep.value(v); }`) works exactly as
    /// ratified.
    #[test]
    fn builder_chains_and_configures_in_place() {
        let mut prep = super::prepare(&[0xAA; 32], &[0x11; 32]);
        prep.args(&(7_u64,)).value(500).gas(200_000);
        assert_eq!(prep.init, Salt::encoding(&(7_u64,)));
        assert_eq!(prep.value, 500);
        assert_eq!(prep.gas, 200_000);
        if prep.value < 1_000 {
            prep.value(1_000);
        }
        assert_eq!(prep.value, 1_000);
        prep.raw_args(&[1, 2, 3]);
        assert_eq!(
            prep.init,
            alloc::vec![1, 2, 3],
            "raw_args replaces verbatim"
        );
    }

    /// The engine's pinned KAT, reproduced independently. If this
    /// trips, the preimage layout or Poseidon2 binding drifted — fix
    /// THAT; never re-pin.
    #[test]
    fn engine_kat_anchor_pins_derivation() {
        const EXPECTED: [u8; 32] = [
            0x35, 0x4a, 0xb9, 0xa5, 0x8e, 0x3f, 0xb7, 0x6b, 0x48, 0x43, 0x90, 0xa2, 0xef, 0x27,
            0x75, 0x94, 0x04, 0x2e, 0x12, 0xfd, 0x0b, 0x74, 0x34, 0x3e, 0x5b, 0xf3, 0x4d, 0xba,
            0x49, 0x2f, 0x3d, 0xfe,
        ];
        let got = p2(&child_preimage(&[0x11; 32], &[0x22; 32], &[0x33; 32]));
        assert_eq!(
            got,
            EXPECTED,
            "child_address drifted from the engine KAT; got {}",
            hex::encode(got),
        );
    }

    #[test]
    fn preimage_layout_is_fixed_width() {
        let (p, t, s) = ([0xAB; 32], [0xCD; 32], [0xEF; 32]);
        let pre = child_preimage(&p, &t, &s);
        assert_eq!(pre.len(), CHILD_PREIMAGE_LEN);
        assert_eq!(&pre[..11], CHILD_ADDRESS_PREFIX);
        assert_eq!(&pre[11..43], &p);
        assert_eq!(&pre[43..75], &t);
        assert_eq!(&pre[75..107], &s);
    }

    #[test]
    fn unordered_pair_encoding_sorts() {
        let (a, b) = ([0xAA_u8; 32], [0xBB_u8; 32]);
        let forward = Salt::unordered_pair_encoding(&a, &b);
        let reversed = Salt::unordered_pair_encoding(&b, &a);
        assert_eq!(forward, reversed, "pair order must not matter");
        assert_eq!(forward.len(), 64, "two raw 32-byte arrays, no framing");
        assert_eq!(&forward[..32], &a, "ascending order: lower address first");
        assert_eq!(&forward[32..], &b);
    }

    /// The committed vector file must equal what the typed cases
    /// produce — guards the file against manual edits AND the code
    /// against drift, in one assertion.
    #[test]
    fn golden_vectors_hold() {
        let raw = std::fs::read_to_string(vectors_path()).expect(
            "vectors/child_address.json missing — run \
             `cargo test -p pyde-host regenerate_golden_vectors -- --ignored`",
        );
        let on_disk: VectorFile = serde_json::from_str(&raw).expect("vector file parses");
        let expected = render_file();
        assert_eq!(
            on_disk.vectors.len(),
            expected.vectors.len(),
            "vector count mismatch",
        );
        for (got, want) in on_disk.vectors.iter().zip(expected.vectors.iter()) {
            for (field, g, w) in [
                ("parent", &got.parent, &want.parent),
                ("template", &got.template, &want.template),
                ("salt", &got.salt, &want.salt),
                ("preimage", &got.preimage, &want.preimage),
                ("child_address", &got.child_address, &want.child_address),
            ] {
                assert_eq!(g, w, "vector `{}`: field `{field}` drifted", want.name);
            }
            assert_eq!(
                got.salt_source_borsh, want.salt_source_borsh,
                "vector `{}`: field `salt_source_borsh` drifted",
                want.name,
            );
            assert_eq!(
                got.salt_source_typed, want.salt_source_typed,
                "vector `{}`: field `salt_source_typed` drifted",
                want.name,
            );
            assert_eq!(
                got.salt_source_pair_args, want.salt_source_pair_args,
                "vector `{}`: field `salt_source_pair_args` drifted",
                want.name,
            );
            assert_eq!(got.name, want.name, "vector name/order drifted");
        }
    }

    /// Replays EVERY unordered-pair vector FROM its unsorted args —
    /// the path a non-Rust implementer takes — proving the ascending-
    /// bytewise sort is load-bearing (skipping it produces a different
    /// salt).
    #[test]
    fn unordered_pair_vectors_replay_from_unsorted_args() {
        let file = render_file();
        let pairs: Vec<_> = file
            .vectors
            .iter()
            .filter(|v| v.salt_source_pair_args.is_some())
            .collect();
        assert!(
            pairs.len() >= 2,
            "expected the plain AND sign-boundary pair vectors",
        );
        for v in pairs {
            let args = v.salt_source_pair_args.as_ref().unwrap();
            let mut a = [0u8; 32];
            let mut b = [0u8; 32];
            a.copy_from_slice(&hex::decode(&args[0]).unwrap());
            b.copy_from_slice(&hex::decode(&args[1]).unwrap());
            // Correct replay: sort ascending bytewise, concatenate, hash.
            let sorted = Salt::unordered_pair_encoding(&a, &b);
            assert_eq!(
                hex::encode(&sorted),
                *v.salt_source_borsh.as_ref().unwrap(),
                "vector `{}`",
                v.name,
            );
            assert_eq!(hex::encode(p2(&sorted)), v.salt, "vector `{}`", v.name);
            // Naive replay (argument order, no sort) MUST diverge — the
            // recorded args are unsorted precisely so this is detectable.
            let mut naive = Vec::with_capacity(64);
            naive.extend_from_slice(&a);
            naive.extend_from_slice(&b);
            assert_ne!(
                hex::encode(p2(&naive)),
                v.salt,
                "vector `{}` failed to exercise the sort: args must be unsorted",
                v.name,
            );
        }
    }

    /// The sort is UNSIGNED bytewise: 0x7f sorts before 0x80. A
    /// signed (i8) comparator reverses them — the exact regression
    /// the sign-boundary golden vector guards across all bindings.
    #[test]
    fn unordered_pair_sort_is_unsigned_at_the_sign_boundary() {
        let enc = Salt::unordered_pair_encoding(&[0x80; 32], &[0x7F; 32]);
        assert_eq!(&enc[..32], &[0x7F; 32], "0x7f first: unsigned order");
        assert_eq!(&enc[32..], &[0x80; 32]);
    }

    /// Dev tool, not a test: rewrites the golden-vector file from the
    /// typed cases. Run explicitly after an INTENTIONAL derivation
    /// change (which must also update the engine KAT + this module's
    /// anchor test + every language binding).
    #[test]
    #[ignore]
    fn regenerate_golden_vectors() {
        let file = render_file();
        let json = format!(
            "{}\n",
            serde_json::to_string_pretty(&file).expect("serialize vectors"),
        );
        let path = vectors_path();
        std::fs::create_dir_all(path.parent().expect("vectors dir")).expect("mkdir vectors");
        std::fs::write(&path, json).expect("write vectors");
        std::println!("wrote {}", path.display());
    }
}
