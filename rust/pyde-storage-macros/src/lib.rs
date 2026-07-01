//! `pyde-storage-macros` — backing crate for `pyde::declare_storage!()`.
//!
//! Re-exported from `pyde-host`; contracts never depend on this crate
//! directly. The function-like macro reads `otigen.toml` at compile
//! time, parses the `[state]` schema, and emits a `mod storage` with
//! one typed accessor per field. Authors write
//!
//! ```ignore
//! pyde::declare_storage!();
//!
//! #[pyde::entry]
//! fn deposit(amount: u128) {
//!     let from = /* ... */;
//!     storage::balances().write(&from, amount);
//! }
//! ```
//!
//! and the macro expands into the equivalent `unsafe { pyde::sstore_map1(...) }`
//! call against the typed-storage host fns. The chain side
//! validates field name + value type + key arity + key types and
//! derives the slot internally (`Poseidon2(self_address || field_name
//! || keys...)`), so the macro is purely a typing + DX layer over an
//! already-secure substrate.
//!
//! ## Expansion shape
//!
//! For an otigen.toml `[state]` block like
//!
//! ```toml
//! [state]
//! schema = [
//!     { name = "counter",   type = "uint64" },
//!     { name = "balances",  type = "map",
//!       keys = ["address"], value = "uint128" },
//!     { name = "allowances", type = "map",
//!       keys = ["address", "address"], value = "uint128" },
//! ]
//! ```
//!
//! the macro emits roughly
//!
//! ```ignore
//! pub mod storage {
//!     pub struct CounterField;
//!     impl CounterField {
//!         const FIELD: &'static [u8] = b"counter";
//!         pub fn read(&self) -> u64 { ... }
//!         pub fn write(&self, value: u64) { ... }
//!         pub fn delete(&self) { ... }
//!     }
//!     pub fn counter() -> CounterField { CounterField }
//!
//!     pub struct BalancesField;
//!     impl BalancesField {
//!         const FIELD: &'static [u8] = b"balances";
//!         pub fn read(&self, k1: &::pyde_host::Address) -> u128 { ... }
//!         pub fn write(&self, k1: &::pyde_host::Address, value: u128) { ... }
//!         pub fn delete(&self, k1: &::pyde_host::Address) { ... }
//!     }
//!     pub fn balances() -> BalancesField { BalancesField }
//!
//!     // ... etc
//! }
//! ```

extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use serde::Deserialize;
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────
// otigen.toml — minimal schema needed by the macro
// ─────────────────────────────────────────────────────────────────────
//
// Mirrors `otigen-toml::schema::StateField` but kept here as a private
// `serde::Deserialize` shape so the macro crate doesn't pull the full
// otigen-toml dep tree (otigen-toml drags wasmparser + wasm-encoder
// + the whole bundle pipeline). The two definitions are
// validated against each other by the end-to-end test in
// `examples/counter-rust` — a drift between them surfaces as
// "field not in schema" at bundle time.

#[derive(Deserialize)]
struct ManifestRoot {
    #[serde(default)]
    state: Option<StateBlock>,
}

#[derive(Deserialize)]
struct StateBlock {
    #[serde(default)]
    schema: Vec<RawField>,
}

#[derive(Deserialize)]
struct RawField {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    #[serde(default)]
    keys: Vec<String>,
    #[serde(default)]
    value: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────
// Scalar type vocabulary — mirrors `pyde_engine_types::ScalarType`
// ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
enum Scalar {
    U8,
    U16,
    U32,
    U64,
    U128,
    I8,
    I16,
    I32,
    I64,
    I128,
    Bool,
    Address,
    Hash32,
    Bytes,
    String,
    Vec(Box<Scalar>),
    /// `struct(<TypeName>)` — author-supplied borsh-codable type.
    /// Carried as the bare Rust ident name; the macro emits
    /// `::borsh::to_vec(&value)` / `::borsh::BorshDeserialize::try_from_slice(&buf)`
    /// for the round-trip. The author's contract crate already pulls
    /// in `borsh` (every contract using `#[pyde::entry]` does), so no
    /// new dep is required.
    Custom(String),
}

impl Scalar {
    /// Parse a TOML type token. Mirrors `otigen-abi::build::parse_scalar_type`
    /// so author-facing tokens stay identical between macro + bundle.
    fn parse(token: &str) -> Result<Self, String> {
        let token = token.trim();
        if let Some(inner) = token.strip_prefix("vec(").and_then(|s| s.strip_suffix(')')) {
            return Ok(Scalar::Vec(Box::new(Scalar::parse(inner)?)));
        }
        // `struct(<TypeName>)` — author-defined borsh-codable type.
        // The bare ident inside the parens is the Rust type the
        // accessor returns and accepts; it must be in scope at the
        // `declare_storage!()` call site and `#[derive(BorshSerialize,
        // BorshDeserialize)]`.
        if let Some(inner) = token
            .strip_prefix("struct(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let name = inner.trim();
            if !is_valid_ident(name) {
                return Err(format!(
                    "{token}: `{name}` is not a valid Rust identifier; expected `struct(<TypeName>)`"
                ));
            }
            return Ok(Scalar::Custom(name.to_owned()));
        }
        // `mapping(K -> V)` is the Solidity-shape map syntax for the
        // `type = "map", keys = [...], value = "..."` form. It's only
        // ever a top-level field type, never a scalar-position type
        // (you can't have `vec(mapping(...))`), so reject it cleanly
        // here rather than silently accept.
        if token.starts_with("mapping(") {
            return Err(format!(
                "{token}: use `type = \"map\", keys = [...], value = \"...\"` instead of inline mapping syntax"
            ));
        }
        Ok(match token {
            "u8" | "uint8" => Scalar::U8,
            "u16" | "uint16" => Scalar::U16,
            "u32" | "uint32" => Scalar::U32,
            "u64" | "uint64" => Scalar::U64,
            "u128" | "uint128" => Scalar::U128,
            "i8" | "int8" => Scalar::I8,
            "i16" | "int16" => Scalar::I16,
            "i32" | "int32" => Scalar::I32,
            "i64" | "int64" => Scalar::I64,
            "i128" | "int128" => Scalar::I128,
            "bool" => Scalar::Bool,
            "address" => Scalar::Address,
            "hash32" => Scalar::Hash32,
            // Solidity migration alias — `bytes32` is the same fixed
            // 32-byte slot as `hash32` and resolves to the same
            // `pyde_host::Hash32` type.
            "bytes32" => Scalar::Hash32,
            "bytes" => Scalar::Bytes,
            "string" => Scalar::String,
            other => return Err(format!("unknown scalar type `{other}`")),
        })
    }

    /// Rust type tokens emitted for this scalar (param + return positions).
    fn rust_ty(&self) -> TokenStream2 {
        match self {
            Scalar::U8 => quote! { u8 },
            Scalar::U16 => quote! { u16 },
            Scalar::U32 => quote! { u32 },
            Scalar::U64 => quote! { u64 },
            Scalar::U128 => quote! { u128 },
            Scalar::I8 => quote! { i8 },
            Scalar::I16 => quote! { i16 },
            Scalar::I32 => quote! { i32 },
            Scalar::I64 => quote! { i64 },
            Scalar::I128 => quote! { i128 },
            Scalar::Bool => quote! { bool },
            Scalar::Address => quote! { ::pyde_host::Address },
            Scalar::Hash32 => quote! { ::pyde_host::Hash32 },
            Scalar::Bytes => quote! { ::alloc::vec::Vec<u8> },
            Scalar::String => quote! { ::alloc::string::String },
            Scalar::Vec(inner) => {
                let inner_ty = inner.rust_ty();
                quote! { ::alloc::vec::Vec<#inner_ty> }
            }
            Scalar::Custom(name) => {
                // `declare_storage!()` emits a `pub mod storage { … }`,
                // so the user-named type lives in the parent scope.
                // Prefix with `super::` so the accessor sigs resolve
                // without forcing authors to add `pub use` boilerplate.
                let id = format_ident!("{}", name);
                quote! { super::#id }
            }
        }
    }

    /// Fixed byte width if known at compile time, else `None`
    /// (variable-length: use the two-step length-probe read path).
    fn fixed_width(&self) -> Option<usize> {
        Some(match self {
            Scalar::U8 | Scalar::I8 | Scalar::Bool => 1,
            Scalar::U16 | Scalar::I16 => 2,
            Scalar::U32 | Scalar::I32 => 4,
            Scalar::U64 | Scalar::I64 => 8,
            Scalar::U128 | Scalar::I128 => 16,
            Scalar::Address | Scalar::Hash32 => 32,
            Scalar::Bytes | Scalar::String | Scalar::Vec(_) | Scalar::Custom(_) => return None,
        })
    }
}

/// Check that `s` is a valid Rust identifier (used for `struct(<Name>)`
/// validation). Restricted to ASCII letters / digits / underscore with
/// a non-digit lead — covers every realistic struct name without
/// pulling in `syn::Ident::parse_str`.
fn is_valid_ident(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// ─────────────────────────────────────────────────────────────────────
// Field model — same shape as engine-side StateField after build
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct Field {
    name: String,
    kind: FieldKind,
    value: Scalar,
}

#[derive(Debug)]
enum FieldKind {
    Scalar,
    Map(Vec<Scalar>),
}

fn lower_fields(raw: &[RawField]) -> Result<Vec<Field>, String> {
    let mut out = Vec::with_capacity(raw.len());
    for f in raw {
        let (kind, value) = if f.ty == "map" {
            if f.keys.is_empty() {
                return Err(format!(
                    "field `{}`: map must declare at least one key type",
                    f.name
                ));
            }
            if f.keys.len() > 3 {
                return Err(format!(
                    "field `{}`: map arity capped at 3 (got {}); compose your model with multiple maps",
                    f.name, f.keys.len()
                ));
            }
            let key_types: Result<Vec<Scalar>, String> =
                f.keys.iter().map(|k| Scalar::parse(k)).collect();
            let key_types = key_types?;
            for (i, k) in key_types.iter().enumerate() {
                if matches!(k, Scalar::Vec(_)) {
                    return Err(format!(
                        "field `{}`: map key #{} is `vec(...)`; vec keys aren't supported (their length varies, which would let two different vecs collide on the same slot)",
                        f.name, i + 1
                    ));
                }
                if let Scalar::Custom(name) = k {
                    return Err(format!(
                        "field `{}`: map key #{} is `struct({})`; struct keys aren't supported (variable-length; canonical byte representation isn't guaranteed across borsh versions). Hash your struct into a `hash32` key on the contract side and use that as the map key instead.",
                        f.name, i + 1, name
                    ));
                }
            }
            let v = f
                .value
                .as_ref()
                .ok_or_else(|| format!("field `{}`: map requires `value = \"...\"`", f.name))?;
            (FieldKind::Map(key_types), Scalar::parse(v)?)
        } else {
            (FieldKind::Scalar, Scalar::parse(&f.ty)?)
        };
        // Vec inner type must be fixed-width (or address/hash32) — the
        // macro emits a length-prefix + N×element_bytes wire format
        // that needs constant per-element width. Nested `vec(...)`,
        // `vec(bytes)`, `vec(string)` require manual encoding on the
        // contract side (escape hatch: raw `sstore_scalar` with
        // your own borsh-encoded payload).
        if let Scalar::Vec(inner) = &value {
            if inner.fixed_width().is_none() {
                return Err(format!(
                    "field `{}`: `vec(<variable>)` not supported by declare_storage!() (inner type is variable-width). Either pick a fixed-width inner (e.g., `vec(u64)`, `vec(address)`) or encode manually via raw sstore_scalar.",
                    f.name
                ));
            }
        }
        out.push(Field {
            name: f.name.clone(),
            kind,
            value,
        });
    }
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────
// Codegen
// ─────────────────────────────────────────────────────────────────────

/// `declare_storage!()` — read otigen.toml at compile time, emit `mod storage`.
///
/// Takes no arguments. `otigen.toml` is the single source of truth for
/// the state schema — putting it anywhere else would let the macro and
/// the deployed `pyde.abi` section drift.
#[proc_macro]
pub fn declare_storage(input: TokenStream) -> TokenStream {
    // Reject any args — single source of truth is otigen.toml.
    let input2: TokenStream2 = input.into();
    if !input2.is_empty() {
        return compile_error(
            "declare_storage!() takes no arguments — otigen.toml is the schema source of truth",
        );
    }

    let manifest_dir = match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(d) => PathBuf::from(d),
        Err(_) => {
            return compile_error(
                "CARGO_MANIFEST_DIR not set; declare_storage!() must run under cargo",
            )
        }
    };
    let toml_path = manifest_dir.join("otigen.toml");
    let toml_str = match std::fs::read_to_string(&toml_path) {
        Ok(s) => s,
        Err(e) => return compile_error(&format!(
            "failed to read {}: {e}\nhint: `declare_storage!()` requires an otigen.toml at the crate root",
            toml_path.display()
        )),
    };

    let manifest: ManifestRoot = match toml::from_str(&toml_str) {
        Ok(m) => m,
        Err(e) => return compile_error(&format!("otigen.toml parse error: {e}")),
    };
    let raw = manifest.state.map(|s| s.schema).unwrap_or_default();
    let fields = match lower_fields(&raw) {
        Ok(f) => f,
        Err(e) => return compile_error(&format!("otigen.toml [state]: {e}")),
    };

    let field_items: Vec<TokenStream2> = fields.iter().map(emit_field).collect();

    // `include_bytes!` makes cargo treat otigen.toml as a build input.
    // Without it the macro reads the file only once per fresh build,
    // and edits to the schema don't trigger a recompile. The const is
    // anonymous + unused; only the dep edge matters.
    let toml_path_str = toml_path.to_string_lossy().into_owned();
    let dep_anchor = quote! {
        const _: &[u8] = include_bytes!(#toml_path_str);
    };

    let expanded = quote! {
        pub mod storage {
            #![allow(non_camel_case_types, clippy::module_name_repetitions, dead_code)]
            extern crate alloc;

            #dep_anchor

            // Two-step read for variable-length values: probe the
            // stored length with a zero-length buffer, then allocate
            // exactly that and call again. `actual <= 0` means the
            // slot is unset (or a schema error, which would be a
            // chain/macro drift bug) and we return the type default.
            //
            // Used by `read()` accessors for `bytes`, `string`,
            // and `vec(...)` fields. Fixed-width fields skip this
            // because their length is known at compile time.
            fn __pyde_storage_read_var(
                kernel: unsafe extern "C" fn(*const u8, i32, *const u8, i32, *mut u8, i32) -> i32,
                field_ptr: *const u8,
                field_len: i32,
                key_buf: *const u8,
                key_buf_len: i32,
            ) -> Option<::alloc::vec::Vec<u8>> {
                let mut probe = [0u8; 0];
                let actual = unsafe { kernel(field_ptr, field_len, key_buf, key_buf_len, probe.as_mut_ptr(), 0) };
                if actual <= 0 { return None; }
                let mut buf = ::alloc::vec![0u8; actual as usize];
                let actual2 = unsafe { kernel(field_ptr, field_len, key_buf, key_buf_len, buf.as_mut_ptr(), actual) };
                if actual2 != actual { return None; }
                Some(buf)
            }

            // Scalar (no key) variants delegate to the same helper
            // via a 0-key shim so the codegen for variable scalars +
            // variable map values stays uniform.
            fn __pyde_storage_read_scalar_var(
                field_ptr: *const u8,
                field_len: i32,
            ) -> Option<::alloc::vec::Vec<u8>> {
                let mut probe = [0u8; 0];
                let actual = unsafe { ::pyde_host::raw::sload_scalar(field_ptr, field_len, probe.as_mut_ptr(), 0) };
                if actual <= 0 { return None; }
                let mut buf = ::alloc::vec![0u8; actual as usize];
                let actual2 = unsafe { ::pyde_host::raw::sload_scalar(field_ptr, field_len, buf.as_mut_ptr(), actual) };
                if actual2 != actual { return None; }
                Some(buf)
            }

            #(#field_items)*
        }
    };
    expanded.into()
}

fn compile_error(msg: &str) -> TokenStream {
    let lit = proc_macro2::Literal::string(msg);
    quote! { compile_error!(#lit); }.into()
}

fn emit_field(f: &Field) -> TokenStream2 {
    let field_name = &f.name;
    let field_bytes = proc_macro2::Literal::byte_string(field_name.as_bytes());
    let ctor_ident = format_ident!("{}", field_name);
    let ty_ident = format_ident!("{}Field", to_pascal_case(field_name));

    match &f.kind {
        FieldKind::Scalar => emit_scalar(f, &ty_ident, &ctor_ident, &field_bytes),
        FieldKind::Map(keys) => emit_map(f, keys, &ty_ident, &ctor_ident, &field_bytes),
    }
}

fn emit_scalar(
    f: &Field,
    ty_ident: &Ident,
    ctor_ident: &Ident,
    field_bytes: &proc_macro2::Literal,
) -> TokenStream2 {
    let value_ty = f.value.rust_ty();
    let read_body = read_body_scalar(&f.value);
    let write_body = write_body(&f.value, /* is_map */ false, 0);

    quote! {
        pub struct #ty_ident;

        impl #ty_ident {
            const FIELD: &'static [u8] = #field_bytes;

            pub fn read(&self) -> #value_ty {
                #read_body
            }

            pub fn write(&self, value: #value_ty) {
                #write_body
            }

            pub fn delete(&self) {
                unsafe {
                    ::pyde_host::raw::sdelete_scalar(
                        Self::FIELD.as_ptr(),
                        Self::FIELD.len() as i32,
                    );
                }
            }
        }

        pub fn #ctor_ident() -> #ty_ident { #ty_ident }
    }
}

fn emit_map(
    f: &Field,
    keys: &[Scalar],
    ty_ident: &Ident,
    ctor_ident: &Ident,
    field_bytes: &proc_macro2::Literal,
) -> TokenStream2 {
    let value_ty = f.value.rust_ty();
    let arity = keys.len();
    let key_idents: Vec<Ident> = (0..arity).map(|i| format_ident!("k{}", i + 1)).collect();
    let key_tys: Vec<TokenStream2> = keys.iter().map(|k| k.rust_ty()).collect();

    // Each key encodes to bytes via the same shape borsh would use for
    // its standalone value: fixed primitives as little-endian raw bytes,
    // address/hash32 as the 32-byte array, bytes/string/vec as
    // length-prefixed (borsh). Per-key concatenation is on the chain
    // (`Poseidon2(addr || field || k1_bytes || k2_bytes || ...)`).
    let key_encodes: Vec<TokenStream2> = (0..arity)
        .map(|i| {
            let id = &key_idents[i];
            let enc_ident = format_ident!("k{}_bytes", i + 1);
            let body = encode_key_to_bytes(&keys[i], id);
            quote! { let #enc_ident: ::alloc::vec::Vec<u8> = #body; }
        })
        .collect();

    let kernel_args = match arity {
        1 => quote! {
            (k1_bytes.as_ptr(), k1_bytes.len() as i32)
        },
        2 => quote! {
            (k1_bytes.as_ptr(), k1_bytes.len() as i32,
             k2_bytes.as_ptr(), k2_bytes.len() as i32)
        },
        3 => quote! {
            (k1_bytes.as_ptr(), k1_bytes.len() as i32,
             k2_bytes.as_ptr(), k2_bytes.len() as i32,
             k3_bytes.as_ptr(), k3_bytes.len() as i32)
        },
        _ => unreachable!("arity validated to 1..=3"),
    };

    let sstore_ident = format_ident!("sstore_map{}", arity);
    let sload_ident = format_ident!("sload_map{}", arity);
    let sdelete_ident = format_ident!("sdelete_map{}", arity);

    let read_body = read_body_map(&f.value, arity);
    let write_body = write_body(&f.value, /* is_map */ true, arity);

    let params: Vec<TokenStream2> = key_idents
        .iter()
        .zip(key_tys.iter())
        .map(|(id, ty)| quote! { #id: &#ty })
        .collect();

    let _ = (sstore_ident, sload_ident, sdelete_ident, kernel_args); // used inside read/write/delete bodies

    let delete_call = match arity {
        1 => quote! {
            ::pyde_host::raw::sdelete_map1(
                Self::FIELD.as_ptr(), Self::FIELD.len() as i32,
                k1_bytes.as_ptr(), k1_bytes.len() as i32,
            )
        },
        2 => quote! {
            ::pyde_host::raw::sdelete_map2(
                Self::FIELD.as_ptr(), Self::FIELD.len() as i32,
                k1_bytes.as_ptr(), k1_bytes.len() as i32,
                k2_bytes.as_ptr(), k2_bytes.len() as i32,
            )
        },
        3 => quote! {
            ::pyde_host::raw::sdelete_map3(
                Self::FIELD.as_ptr(), Self::FIELD.len() as i32,
                k1_bytes.as_ptr(), k1_bytes.len() as i32,
                k2_bytes.as_ptr(), k2_bytes.len() as i32,
                k3_bytes.as_ptr(), k3_bytes.len() as i32,
            )
        },
        _ => unreachable!(),
    };

    quote! {
        pub struct #ty_ident;

        impl #ty_ident {
            const FIELD: &'static [u8] = #field_bytes;

            pub fn read(&self, #(#params),*) -> #value_ty {
                #(#key_encodes)*
                #read_body
            }

            pub fn write(&self, #(#params),* , value: #value_ty) {
                #(#key_encodes)*
                #write_body
            }

            pub fn delete(&self, #(#params),*) {
                #(#key_encodes)*
                unsafe { #delete_call; }
            }
        }

        pub fn #ctor_ident() -> #ty_ident { #ty_ident }
    }
}

/// Encode a single key value to bytes for slot-derivation input.
///
/// The chain hashes raw key bytes — there's no encoding negotiation. We
/// pick the unambiguous canonical form for each scalar:
///   - fixed-width primitives → little-endian raw bytes
///   - address/hash32 → the 32-byte array (no length prefix)
///   - bytes/string/vec → borsh (length-prefixed; distinguishes empty
///     from absent and varies the slot under concatenation)
fn encode_key_to_bytes(ty: &Scalar, id: &Ident) -> TokenStream2 {
    match ty {
        Scalar::U8
        | Scalar::U16
        | Scalar::U32
        | Scalar::U64
        | Scalar::U128
        | Scalar::I8
        | Scalar::I16
        | Scalar::I32
        | Scalar::I64
        | Scalar::I128 => {
            quote! { (#id.to_le_bytes()).to_vec() }
        }
        Scalar::Bool => quote! { ::alloc::vec![if *#id { 1u8 } else { 0u8 }] },
        Scalar::Address | Scalar::Hash32 => quote! { (*#id).to_vec() },
        Scalar::Bytes | Scalar::String | Scalar::Vec(_) => {
            // Variable-width: lean on borsh for the length prefix so
            // concatenation in slot derivation stays injective. Authors
            // who want raw bytes can model as `address`/`hash32`.
            quote! { {
                use ::alloc::vec::Vec;
                let mut out: Vec<u8> = Vec::new();
                // Manual length-prefix: little-endian u32 length + bytes.
                // We don't import borsh inside the macro output (would
                // force every contract to depend on borsh just for
                // declare_storage!) — handcraft the same wire shape.
                let bytes_ref: &[u8] = #id.as_ref();
                out.extend_from_slice(&(bytes_ref.len() as u32).to_le_bytes());
                out.extend_from_slice(bytes_ref);
                out
            } }
        }
        Scalar::Custom(_) => {
            unreachable!("struct(...) map keys rejected in lower_fields")
        }
    }
}

fn read_body_scalar(ty: &Scalar) -> TokenStream2 {
    match ty.fixed_width() {
        Some(n) => fixed_read_scalar(ty, n),
        None => variable_read_scalar(ty),
    }
}

fn read_body_map(ty: &Scalar, arity: usize) -> TokenStream2 {
    match ty.fixed_width() {
        Some(n) => fixed_read_map(ty, n, arity),
        None => variable_read_map(ty, arity),
    }
}

fn fixed_read_scalar(ty: &Scalar, n: usize) -> TokenStream2 {
    let nlit = proc_macro2::Literal::usize_unsuffixed(n);
    let decode = decode_fixed(ty);
    quote! {
        let mut buf = [0u8; #nlit];
        let actual = unsafe {
            ::pyde_host::raw::sload_scalar(
                Self::FIELD.as_ptr(),
                Self::FIELD.len() as i32,
                buf.as_mut_ptr(),
                buf.len() as i32,
            )
        };
        if actual <= 0 { return Default::default(); }
        #decode
    }
}

fn fixed_read_map(ty: &Scalar, n: usize, arity: usize) -> TokenStream2 {
    let nlit = proc_macro2::Literal::usize_unsuffixed(n);
    let decode = decode_fixed(ty);
    let load_call = match arity {
        1 => quote! {
            ::pyde_host::raw::sload_map1(
                Self::FIELD.as_ptr(), Self::FIELD.len() as i32,
                k1_bytes.as_ptr(), k1_bytes.len() as i32,
                buf.as_mut_ptr(), buf.len() as i32,
            )
        },
        2 => quote! {
            ::pyde_host::raw::sload_map2(
                Self::FIELD.as_ptr(), Self::FIELD.len() as i32,
                k1_bytes.as_ptr(), k1_bytes.len() as i32,
                k2_bytes.as_ptr(), k2_bytes.len() as i32,
                buf.as_mut_ptr(), buf.len() as i32,
            )
        },
        3 => quote! {
            ::pyde_host::raw::sload_map3(
                Self::FIELD.as_ptr(), Self::FIELD.len() as i32,
                k1_bytes.as_ptr(), k1_bytes.len() as i32,
                k2_bytes.as_ptr(), k2_bytes.len() as i32,
                k3_bytes.as_ptr(), k3_bytes.len() as i32,
                buf.as_mut_ptr(), buf.len() as i32,
            )
        },
        _ => unreachable!(),
    };
    quote! {
        let mut buf = [0u8; #nlit];
        let actual = unsafe { #load_call };
        if actual <= 0 { return Default::default(); }
        #decode
    }
}

fn decode_fixed(ty: &Scalar) -> TokenStream2 {
    match ty {
        Scalar::U8 => quote! { buf[0] },
        Scalar::U16 => quote! { u16::from_le_bytes(buf) },
        Scalar::U32 => quote! { u32::from_le_bytes(buf) },
        Scalar::U64 => quote! { u64::from_le_bytes(buf) },
        Scalar::U128 => quote! { u128::from_le_bytes(buf) },
        Scalar::I8 => quote! { buf[0] as i8 },
        Scalar::I16 => quote! { i16::from_le_bytes(buf) },
        Scalar::I32 => quote! { i32::from_le_bytes(buf) },
        Scalar::I64 => quote! { i64::from_le_bytes(buf) },
        Scalar::I128 => quote! { i128::from_le_bytes(buf) },
        Scalar::Bool => quote! { buf[0] != 0 },
        Scalar::Address | Scalar::Hash32 => quote! { buf },
        _ => unreachable!("decode_fixed called on variable-width type"),
    }
}

fn variable_read_scalar(ty: &Scalar) -> TokenStream2 {
    let decode = decode_variable(ty);
    quote! {
        let Some(buf) = __pyde_storage_read_scalar_var(
            Self::FIELD.as_ptr(),
            Self::FIELD.len() as i32,
        ) else {
            return Default::default();
        };
        #decode
    }
}

fn variable_read_map(ty: &Scalar, arity: usize) -> TokenStream2 {
    let decode = decode_variable(ty);
    // Variable-length map reads concatenate keys into one buffer then
    // hand it to the appropriate `sload_mapN`. We can't use the
    // probe helper directly (its signature is sload_scalar-shaped)
    // so inline the 2-step probe for each arity.
    let probe_call = match arity {
        1 => quote! {
            ::pyde_host::raw::sload_map1(
                Self::FIELD.as_ptr(), Self::FIELD.len() as i32,
                k1_bytes.as_ptr(), k1_bytes.len() as i32,
                buf.as_mut_ptr(), buf.len() as i32,
            )
        },
        2 => quote! {
            ::pyde_host::raw::sload_map2(
                Self::FIELD.as_ptr(), Self::FIELD.len() as i32,
                k1_bytes.as_ptr(), k1_bytes.len() as i32,
                k2_bytes.as_ptr(), k2_bytes.len() as i32,
                buf.as_mut_ptr(), buf.len() as i32,
            )
        },
        3 => quote! {
            ::pyde_host::raw::sload_map3(
                Self::FIELD.as_ptr(), Self::FIELD.len() as i32,
                k1_bytes.as_ptr(), k1_bytes.len() as i32,
                k2_bytes.as_ptr(), k2_bytes.len() as i32,
                k3_bytes.as_ptr(), k3_bytes.len() as i32,
                buf.as_mut_ptr(), buf.len() as i32,
            )
        },
        _ => unreachable!(),
    };
    quote! {
        let mut buf: [u8; 0] = [];
        let actual = unsafe { #probe_call };
        if actual <= 0 { return Default::default(); }
        let mut buf: ::alloc::vec::Vec<u8> = ::alloc::vec![0u8; actual as usize];
        let _ = unsafe { #probe_call };
        #decode
    }
}

fn decode_variable(ty: &Scalar) -> TokenStream2 {
    match ty {
        Scalar::Bytes => quote! { buf },
        Scalar::String => quote! {
            ::alloc::string::String::from_utf8(buf).unwrap_or_default()
        },
        Scalar::Custom(name) => {
            // Borsh round-trip — author guaranteed `#[derive(BorshSerialize,
            // BorshDeserialize)]` on the type. Decode failure (e.g. slot
            // bytes from a contract version with a different schema)
            // falls back to `Default::default()`, mirroring how every
            // other variable-width decode handles malformed bytes.
            // Type is `super::<Name>` because the storage mod lives one
            // level below the parent scope where the author declared it.
            let id = format_ident!("{}", name);
            quote! {
                <super::#id as ::borsh::BorshDeserialize>::try_from_slice(&buf)
                    .unwrap_or_default()
            }
        }
        Scalar::Vec(inner) => {
            // Wire format: u32_le(len) + len * fixed_width(inner) bytes.
            // Mirrors `borsh::to_vec(&Vec<T>)` for primitive T, so a
            // contract that ever wants to escape the macro can decode
            // raw stored bytes via borsh and get the same answer.
            let width = inner
                .fixed_width()
                .expect("vec inner validated as fixed-width in lower_fields");
            let width_lit = proc_macro2::Literal::usize_unsuffixed(width);
            let elem_decode = decode_vec_element(inner);
            quote! {{
                if buf.len() < 4 { return Default::default(); }
                let __len_bytes: [u8; 4] = [buf[0], buf[1], buf[2], buf[3]];
                let __len = u32::from_le_bytes(__len_bytes) as usize;
                let __needed = 4usize.saturating_add(__len.saturating_mul(#width_lit));
                if buf.len() < __needed { return Default::default(); }
                let mut __out = ::alloc::vec::Vec::with_capacity(__len);
                let mut __cursor: usize = 4;
                for _ in 0..__len {
                    #elem_decode
                    __cursor += #width_lit;
                }
                __out
            }}
        }
        _ => unreachable!("decode_variable called on fixed-width type"),
    }
}

/// Per-element decode emitted inside a `for ... in 0..len` loop. Reads
/// from `buf[__cursor..__cursor + width]` and pushes onto `__out`. The
/// cursor is bumped by the caller after each iteration.
fn decode_vec_element(ty: &Scalar) -> TokenStream2 {
    match ty {
        Scalar::U8 => quote! { __out.push(buf[__cursor]); },
        Scalar::I8 => quote! { __out.push(buf[__cursor] as i8); },
        Scalar::Bool => quote! { __out.push(buf[__cursor] != 0); },
        Scalar::U16 => quote! {
            let mut __b = [0u8; 2]; __b.copy_from_slice(&buf[__cursor..__cursor + 2]);
            __out.push(u16::from_le_bytes(__b));
        },
        Scalar::U32 => quote! {
            let mut __b = [0u8; 4]; __b.copy_from_slice(&buf[__cursor..__cursor + 4]);
            __out.push(u32::from_le_bytes(__b));
        },
        Scalar::U64 => quote! {
            let mut __b = [0u8; 8]; __b.copy_from_slice(&buf[__cursor..__cursor + 8]);
            __out.push(u64::from_le_bytes(__b));
        },
        Scalar::U128 => quote! {
            let mut __b = [0u8; 16]; __b.copy_from_slice(&buf[__cursor..__cursor + 16]);
            __out.push(u128::from_le_bytes(__b));
        },
        Scalar::I16 => quote! {
            let mut __b = [0u8; 2]; __b.copy_from_slice(&buf[__cursor..__cursor + 2]);
            __out.push(i16::from_le_bytes(__b));
        },
        Scalar::I32 => quote! {
            let mut __b = [0u8; 4]; __b.copy_from_slice(&buf[__cursor..__cursor + 4]);
            __out.push(i32::from_le_bytes(__b));
        },
        Scalar::I64 => quote! {
            let mut __b = [0u8; 8]; __b.copy_from_slice(&buf[__cursor..__cursor + 8]);
            __out.push(i64::from_le_bytes(__b));
        },
        Scalar::I128 => quote! {
            let mut __b = [0u8; 16]; __b.copy_from_slice(&buf[__cursor..__cursor + 16]);
            __out.push(i128::from_le_bytes(__b));
        },
        Scalar::Address | Scalar::Hash32 => quote! {
            let mut __b = [0u8; 32]; __b.copy_from_slice(&buf[__cursor..__cursor + 32]);
            __out.push(__b);
        },
        Scalar::Bytes | Scalar::String | Scalar::Vec(_) | Scalar::Custom(_) => {
            unreachable!("vec inner validated as fixed-width in lower_fields")
        }
    }
}

fn write_body(ty: &Scalar, is_map: bool, arity: usize) -> TokenStream2 {
    let encode = encode_for_write(ty);
    let store_call = if !is_map {
        quote! {
            ::pyde_host::raw::sstore_scalar(
                Self::FIELD.as_ptr(), Self::FIELD.len() as i32,
                __value_bytes.as_ptr(), __value_bytes.len() as i32,
            )
        }
    } else {
        match arity {
            1 => quote! {
                ::pyde_host::raw::sstore_map1(
                    Self::FIELD.as_ptr(), Self::FIELD.len() as i32,
                    k1_bytes.as_ptr(), k1_bytes.len() as i32,
                    __value_bytes.as_ptr(), __value_bytes.len() as i32,
                )
            },
            2 => quote! {
                ::pyde_host::raw::sstore_map2(
                    Self::FIELD.as_ptr(), Self::FIELD.len() as i32,
                    k1_bytes.as_ptr(), k1_bytes.len() as i32,
                    k2_bytes.as_ptr(), k2_bytes.len() as i32,
                    __value_bytes.as_ptr(), __value_bytes.len() as i32,
                )
            },
            3 => quote! {
                ::pyde_host::raw::sstore_map3(
                    Self::FIELD.as_ptr(), Self::FIELD.len() as i32,
                    k1_bytes.as_ptr(), k1_bytes.len() as i32,
                    k2_bytes.as_ptr(), k2_bytes.len() as i32,
                    k3_bytes.as_ptr(), k3_bytes.len() as i32,
                    __value_bytes.as_ptr(), __value_bytes.len() as i32,
                )
            },
            _ => unreachable!(),
        }
    };
    quote! {
        #encode
        unsafe { #store_call; }
    }
}

fn encode_for_write(ty: &Scalar) -> TokenStream2 {
    match ty {
        Scalar::U8 | Scalar::I8 => quote! {
            let __value_bytes: [u8; 1] = [value as u8];
        },
        Scalar::U16
        | Scalar::U32
        | Scalar::U64
        | Scalar::U128
        | Scalar::I16
        | Scalar::I32
        | Scalar::I64
        | Scalar::I128 => quote! {
            let __value_bytes = value.to_le_bytes();
        },
        Scalar::Bool => quote! {
            let __value_bytes: [u8; 1] = [if value { 1u8 } else { 0u8 }];
        },
        Scalar::Address | Scalar::Hash32 => quote! {
            let __value_bytes: [u8; 32] = value;
        },
        Scalar::Bytes => quote! {
            let __value_bytes: ::alloc::vec::Vec<u8> = value;
        },
        Scalar::String => quote! {
            let __value_bytes: ::alloc::vec::Vec<u8> = value.into_bytes();
        },
        Scalar::Vec(inner) => {
            // Wire format: u32_le(len) + len * fixed_width(inner) bytes.
            // Same shape `borsh::to_vec(&Vec<T>)` emits for primitive T.
            let elem_enc = encode_vec_element(inner);
            quote! {
                let mut __value_bytes: ::alloc::vec::Vec<u8> = ::alloc::vec::Vec::new();
                __value_bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
                for __elem in value.iter() {
                    #elem_enc
                }
            }
        }
        Scalar::Custom(_) => quote! {
            // Borsh serialise — author guaranteed `#[derive(BorshSerialize,
            // BorshDeserialize)]` on the type. Encode failure is treated
            // the same way the rest of the macro treats it: zero-length
            // payload. In practice borsh's primitive impls never fail
            // (they only return errors for `io::Error`-shaped sinks,
            // not for in-memory `Vec`s).
            let __value_bytes: ::alloc::vec::Vec<u8> = ::borsh::to_vec(&value)
                .unwrap_or_default();
        },
    }
}

/// Per-element encode emitted inside a `for __elem in value.iter()` loop.
/// `__elem` is a borrowed reference to one element of the Vec.
fn encode_vec_element(ty: &Scalar) -> TokenStream2 {
    match ty {
        Scalar::U8 => quote! { __value_bytes.push(*__elem); },
        Scalar::I8 => quote! { __value_bytes.push(*__elem as u8); },
        Scalar::Bool => quote! { __value_bytes.push(if *__elem { 1u8 } else { 0u8 }); },
        Scalar::U16
        | Scalar::U32
        | Scalar::U64
        | Scalar::U128
        | Scalar::I16
        | Scalar::I32
        | Scalar::I64
        | Scalar::I128 => quote! {
            __value_bytes.extend_from_slice(&__elem.to_le_bytes());
        },
        Scalar::Address | Scalar::Hash32 => quote! {
            __value_bytes.extend_from_slice(__elem);
        },
        Scalar::Bytes | Scalar::String | Scalar::Vec(_) | Scalar::Custom(_) => {
            unreachable!("vec inner validated as fixed-width in lower_fields")
        }
    }
}

fn to_pascal_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper = true;
    for c in s.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

// ─────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_case_roundtrips() {
        assert_eq!(to_pascal_case("counter"), "Counter");
        assert_eq!(to_pascal_case("total_supply"), "TotalSupply");
        assert_eq!(to_pascal_case("erc20_balances"), "Erc20Balances");
        assert_eq!(to_pascal_case("x"), "X");
    }

    #[test]
    fn parse_primitive_scalars() {
        assert!(matches!(Scalar::parse("u8").unwrap(), Scalar::U8));
        assert!(matches!(Scalar::parse("uint8").unwrap(), Scalar::U8));
        assert!(matches!(Scalar::parse("u128").unwrap(), Scalar::U128));
        assert!(matches!(Scalar::parse("uint128").unwrap(), Scalar::U128));
        assert!(matches!(Scalar::parse("i64").unwrap(), Scalar::I64));
        assert!(matches!(Scalar::parse("bool").unwrap(), Scalar::Bool));
        assert!(matches!(Scalar::parse("address").unwrap(), Scalar::Address));
        assert!(matches!(Scalar::parse("hash32").unwrap(), Scalar::Hash32));
        assert!(matches!(Scalar::parse("bytes32").unwrap(), Scalar::Hash32));
        assert!(matches!(Scalar::parse("bytes").unwrap(), Scalar::Bytes));
        assert!(matches!(Scalar::parse("string").unwrap(), Scalar::String));
    }

    #[test]
    fn parse_vec_recursive() {
        let ty = Scalar::parse("vec(u64)").unwrap();
        let Scalar::Vec(inner) = ty else {
            panic!("not vec")
        };
        assert!(matches!(*inner, Scalar::U64));

        let ty = Scalar::parse("vec(address)").unwrap();
        let Scalar::Vec(inner) = ty else {
            panic!("not vec")
        };
        assert!(matches!(*inner, Scalar::Address));
    }

    #[test]
    fn parse_rejects_mapping_inline_syntax() {
        // Authors who try to use Solidity's `mapping(K -> V)` form get
        // a pointed error directing them at the canonical `type =
        // "map", keys = [...]` shape.
        let err = Scalar::parse("mapping(address -> uint128)").unwrap_err();
        assert!(err.contains("type = \"map\""), "got: {err}");
    }

    #[test]
    fn parse_rejects_unknown() {
        let err = Scalar::parse("uint27").unwrap_err();
        assert!(err.contains("uint27"));
    }

    #[test]
    fn fixed_widths_match_engine_side() {
        // Pin: macro's compile-time widths must equal
        // engine's `ScalarType::fixed_byte_width()` so the host
        // accepts the value-type check.
        assert_eq!(Scalar::U8.fixed_width(), Some(1));
        assert_eq!(Scalar::U16.fixed_width(), Some(2));
        assert_eq!(Scalar::U32.fixed_width(), Some(4));
        assert_eq!(Scalar::U64.fixed_width(), Some(8));
        assert_eq!(Scalar::U128.fixed_width(), Some(16));
        assert_eq!(Scalar::I128.fixed_width(), Some(16));
        assert_eq!(Scalar::Bool.fixed_width(), Some(1));
        assert_eq!(Scalar::Address.fixed_width(), Some(32));
        assert_eq!(Scalar::Hash32.fixed_width(), Some(32));
        assert_eq!(Scalar::Bytes.fixed_width(), None);
        assert_eq!(Scalar::String.fixed_width(), None);
        assert_eq!(Scalar::Vec(Box::new(Scalar::U8)).fixed_width(), None);
    }

    #[test]
    fn lower_fields_scalar() {
        let raw = vec![RawField {
            name: "counter".into(),
            ty: "uint64".into(),
            keys: vec![],
            value: None,
        }];
        let fields = lower_fields(&raw).unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "counter");
        assert!(matches!(fields[0].kind, FieldKind::Scalar));
        assert!(matches!(fields[0].value, Scalar::U64));
    }

    #[test]
    fn lower_fields_map_arity_2() {
        let raw = vec![RawField {
            name: "allowances".into(),
            ty: "map".into(),
            keys: vec!["address".into(), "address".into()],
            value: Some("uint128".into()),
        }];
        let fields = lower_fields(&raw).unwrap();
        let Field {
            kind: FieldKind::Map(keys),
            value,
            ..
        } = &fields[0]
        else {
            panic!("not a map");
        };
        assert_eq!(keys.len(), 2);
        assert!(matches!(keys[0], Scalar::Address));
        assert!(matches!(keys[1], Scalar::Address));
        assert!(matches!(value, Scalar::U128));
    }

    #[test]
    fn lower_fields_rejects_arity_4() {
        let raw = vec![RawField {
            name: "too_deep".into(),
            ty: "map".into(),
            keys: vec!["u64".into(); 4],
            value: Some("u64".into()),
        }];
        let err = lower_fields(&raw).unwrap_err();
        assert!(err.contains("arity capped at 3"));
    }

    #[test]
    fn lower_fields_rejects_map_without_keys() {
        let raw = vec![RawField {
            name: "no_keys".into(),
            ty: "map".into(),
            keys: vec![],
            value: Some("u64".into()),
        }];
        let err = lower_fields(&raw).unwrap_err();
        assert!(err.contains("at least one key"));
    }

    #[test]
    fn lower_fields_rejects_vec_of_variable() {
        // vec(bytes), vec(string), vec(vec(...)) all rejected — the
        // length-prefix wire format needs a constant per-element width.
        for bad in ["vec(bytes)", "vec(string)", "vec(vec(u64))"] {
            let raw = vec![RawField {
                name: "x".into(),
                ty: bad.into(),
                keys: vec![],
                value: None,
            }];
            let err = lower_fields(&raw).unwrap_err();
            assert!(err.contains("not supported"), "got: {err} for {bad}");
        }
    }

    #[test]
    fn lower_fields_accepts_vec_of_fixed() {
        for ok in [
            "vec(u8)",
            "vec(u64)",
            "vec(u128)",
            "vec(i64)",
            "vec(bool)",
            "vec(address)",
            "vec(hash32)",
            "vec(bytes32)",
        ] {
            let raw = vec![RawField {
                name: "x".into(),
                ty: ok.into(),
                keys: vec![],
                value: None,
            }];
            let fields = lower_fields(&raw).expect(ok);
            assert!(matches!(fields[0].value, Scalar::Vec(_)));
        }
    }

    #[test]
    fn lower_fields_rejects_vec_keyed_map() {
        // A vec-typed map key has variable length; two different vecs
        // could collide on the same slot. Reject upfront with a clear
        // error rather than silently constructing a collision-prone
        // map.
        let raw = vec![RawField {
            name: "bad".into(),
            ty: "map".into(),
            keys: vec!["vec(u64)".into()],
            value: Some("u64".into()),
        }];
        let err = lower_fields(&raw).unwrap_err();
        assert!(err.contains("vec keys aren't supported"), "got: {err}");
    }

    #[test]
    fn parse_struct_extracts_type_name() {
        let Scalar::Custom(name) = Scalar::parse("struct(Position)").unwrap() else {
            panic!("not Custom");
        };
        assert_eq!(name, "Position");

        // Whitespace inside parens is tolerated.
        let Scalar::Custom(name) = Scalar::parse("struct( UserProfile )").unwrap() else {
            panic!("not Custom");
        };
        assert_eq!(name, "UserProfile");

        // Underscores + digits in trailing positions are valid Rust idents.
        let Scalar::Custom(name) = Scalar::parse("struct(_legacy_Type2)").unwrap() else {
            panic!("not Custom");
        };
        assert_eq!(name, "_legacy_Type2");
    }

    #[test]
    fn parse_struct_rejects_invalid_ident() {
        // Leading digit not allowed.
        let err = Scalar::parse("struct(0invalid)").unwrap_err();
        assert!(err.contains("not a valid Rust identifier"), "got: {err}");

        // Path syntax not allowed — author must `use` the type first.
        let err = Scalar::parse("struct(crate::Foo)").unwrap_err();
        assert!(err.contains("not a valid Rust identifier"), "got: {err}");

        // Empty struct() rejected.
        let err = Scalar::parse("struct()").unwrap_err();
        assert!(err.contains("not a valid Rust identifier"), "got: {err}");
    }

    #[test]
    fn custom_is_variable_width() {
        // Borsh-encoded struct has variable byte length; the macro's
        // probe-then-read path applies (same as bytes/string/vec).
        assert_eq!(Scalar::Custom("Position".into()).fixed_width(), None);
    }

    #[test]
    fn lower_fields_accepts_struct_scalar() {
        let raw = vec![RawField {
            name: "owner_profile".into(),
            ty: "struct(UserProfile)".into(),
            keys: vec![],
            value: None,
        }];
        let fields = lower_fields(&raw).expect("struct scalar accepted");
        assert!(matches!(fields[0].kind, FieldKind::Scalar));
        match &fields[0].value {
            Scalar::Custom(name) => assert_eq!(name, "UserProfile"),
            other => panic!("expected Custom, got {other:?}"),
        }
    }

    #[test]
    fn lower_fields_accepts_struct_map_value() {
        let raw = vec![RawField {
            name: "profiles".into(),
            ty: "map".into(),
            keys: vec!["address".into()],
            value: Some("struct(UserProfile)".into()),
        }];
        let fields = lower_fields(&raw).expect("struct map value accepted");
        let Field {
            kind: FieldKind::Map(keys),
            value,
            ..
        } = &fields[0]
        else {
            panic!("not a map");
        };
        assert!(matches!(keys[0], Scalar::Address));
        match value {
            Scalar::Custom(name) => assert_eq!(name, "UserProfile"),
            other => panic!("expected Custom, got {other:?}"),
        }
    }

    #[test]
    fn lower_fields_rejects_struct_as_map_key() {
        // Variable-length keys can't be safely encoded for slot
        // derivation without exposing borsh-version compatibility
        // surface to the slot layout. Authors hash to a hash32
        // themselves and use that as the key.
        let raw = vec![RawField {
            name: "bad".into(),
            ty: "map".into(),
            keys: vec!["struct(Position)".into()],
            value: Some("u64".into()),
        }];
        let err = lower_fields(&raw).unwrap_err();
        assert!(
            err.contains("struct(Position)") && err.contains("struct keys aren't supported"),
            "got: {err}"
        );
    }

    #[test]
    fn lower_fields_rejects_vec_of_struct() {
        // vec(struct(X)) — same family as vec(bytes)/vec(string):
        // variable-width inner can't be packed into the macro's
        // length-prefix-plus-fixed-stride wire format.
        let raw = vec![RawField {
            name: "positions".into(),
            ty: "vec(struct(Position))".into(),
            keys: vec![],
            value: None,
        }];
        let err = lower_fields(&raw).unwrap_err();
        assert!(err.contains("not supported"), "got: {err}");
    }

    #[test]
    fn is_valid_ident_covers_realistic_cases() {
        assert!(is_valid_ident("UserProfile"));
        assert!(is_valid_ident("Foo"));
        assert!(is_valid_ident("_private"));
        assert!(is_valid_ident("T2"));
        assert!(is_valid_ident("longer_snake_case_name"));
        assert!(!is_valid_ident(""));
        assert!(!is_valid_ident("2bad"));
        assert!(!is_valid_ident("has space"));
        assert!(!is_valid_ident("has-dash"));
        assert!(!is_valid_ident("path::Type"));
    }

    #[test]
    fn lower_fields_rejects_map_without_value() {
        let raw = vec![RawField {
            name: "no_value".into(),
            ty: "map".into(),
            keys: vec!["u64".into()],
            value: None,
        }];
        let err = lower_fields(&raw).unwrap_err();
        assert!(err.contains("requires `value"));
    }
}
