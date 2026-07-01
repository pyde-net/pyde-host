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
        pub fn balance(addr_ptr: *const u8, balance_out_ptr: *mut u8) -> i32;

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
        pub fn consume_gas(amount: i64) -> i32;

        // ─────────────────────────────────────────────────────────────────
        // §7.11 VRF beacon
        // ─────────────────────────────────────────────────────────────────

        /// Current wave's committee-derived VRF beacon (32 bytes).
        /// Deterministic, public randomness. NB: publicly predictable
        /// within a wave — use threshold encryption if you need
        /// adversary-private randomness.
        ///
        /// gas: 50 base.
        pub fn beacon_get(out_ptr: *mut u8) -> i32;
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
pub fn consume_gas(amount: u64) -> i32 {
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
    /// common ERC20/721-shape return; larger payloads should
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
