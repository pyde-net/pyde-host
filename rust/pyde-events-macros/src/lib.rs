//! `pyde-events-macros` — backing crate for `pyde::declare_events!()`.
//!
//! Re-exported from `pyde-host`; contracts never depend on this crate
//! directly. The function-like macro reads `otigen.toml` at compile
//! time, parses every `[events.NAME]` block, computes the canonical
//! topic-0 hash (`Blake3(signature)`) for each, and emits a typed
//! struct + `emit()` impl under `mod events`. Authors write
//!
//! ```ignore
//! pyde::declare_events!();
//!
//! events::Transfer { from, to, amount }.emit();
//! ```
//!
//! and the macro expands into the equivalent
//! `pyde::emit_event(&[TOPIC0, from, to], &borsh::to_vec(&amount).unwrap())`
//! call — same wire bytes as a hand-rolled emission, but the topic-0
//! hash + indexed-topic layout + data encoding are all derived from
//! `otigen.toml` so a typo at any layer is a compile error.
//!
//! ## Expansion shape
//!
//! For an `otigen.toml` `[events.Transfer]` block like
//!
//! ```toml
//! [events.Transfer]
//! signature = "Transfer(address,address,uint128)"
//! fields = [
//!     { name = "from",   type = "address",  indexed = true },
//!     { name = "to",     type = "address",  indexed = true },
//!     { name = "amount", type = "uint128" },
//! ]
//! ```
//!
//! the macro emits roughly
//!
//! ```ignore
//! pub mod events {
//!     pub struct Transfer {
//!         pub from: ::pyde_host::Address,
//!         pub to: ::pyde_host::Address,
//!         pub amount: u128,
//!     }
//!     impl Transfer {
//!         pub const TOPIC_0: ::pyde_host::Hash32 = [0x71, 0xfb, /* ... */];
//!         pub fn emit(&self) {
//!             let topics = [Self::TOPIC_0, self.from, self.to];
//!             let data = self.amount.to_le_bytes();
//!             ::pyde_host::emit_event(&topics, &data);
//!         }
//!     }
//! }
//! ```
//!
//! ## Restrictions in v1
//!
//! - Indexed fields must be `address`, `hash32`, or `bytes32` (the
//!   32-byte fixed-width types). Hashing a variable-width indexed
//!   value to fit it into a 32-byte topic is a separate convention
//!   the chain doesn't yet pick a side on.
//! - Non-indexed fields support all primitives (`u8`..`u128`,
//!   `i8`..`i128`, `bool`, `address`, `hash32`, `bytes32`, `bytes`,
//!   `string`). Multiple non-indexed fields are encoded as a
//!   borsh tuple in declaration order.
//! - Max 4 topics (chain limit, see `HOST_FN_ABI_SPEC` §7.5). With
//!   topic[0] reserved for the signature hash, that leaves 3
//!   indexed fields max per event.

extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────
// otigen.toml — minimal shape needed by this macro
// ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ManifestRoot {
    #[serde(default)]
    events: BTreeMap<String, RawEvent>,
}

#[derive(Deserialize)]
struct RawEvent {
    signature: String,
    #[serde(default)]
    fields: Vec<RawField>,
}

#[derive(Clone, Deserialize)]
struct RawField {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    #[serde(default)]
    indexed: bool,
}

// ─────────────────────────────────────────────────────────────────────
// Field-type vocabulary
// ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
enum FieldTy {
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
}

impl FieldTy {
    fn parse(token: &str) -> Result<Self, String> {
        Ok(match token.trim() {
            "u8" | "uint8" => FieldTy::U8,
            "u16" | "uint16" => FieldTy::U16,
            "u32" | "uint32" => FieldTy::U32,
            "u64" | "uint64" => FieldTy::U64,
            "u128" | "uint128" => FieldTy::U128,
            "i8" | "int8" => FieldTy::I8,
            "i16" | "int16" => FieldTy::I16,
            "i32" | "int32" => FieldTy::I32,
            "i64" | "int64" => FieldTy::I64,
            "i128" | "int128" => FieldTy::I128,
            "bool" => FieldTy::Bool,
            "address" => FieldTy::Address,
            "hash32" | "bytes32" => FieldTy::Hash32,
            "bytes" => FieldTy::Bytes,
            "string" => FieldTy::String,
            other => return Err(format!("unsupported event field type `{other}`")),
        })
    }

    /// Canonical Solidity-style spelling used for topic-0 hashing.
    /// Distinct from the `Display` impl below (which uses
    /// short names `u64`/`hash32` for compile-error messaging
    /// and codegen-internal debug output).
    ///
    /// Topic-0 is `Blake3(Name(<canonical_ty>, ...))`. Authors
    /// can spell the `signature` field in `otigen.toml` either
    /// `Foo(u64)` or `Foo(uint64)`; both produce the same
    /// topic-0 because we re-derive the signature from typed
    /// fields here instead of hashing the author's prose. SDKs
    /// reconstructing topic-0 from the typed ABI converge on
    /// this same string.
    fn canonical_str(&self) -> &'static str {
        match self {
            FieldTy::U8 => "uint8",
            FieldTy::U16 => "uint16",
            FieldTy::U32 => "uint32",
            FieldTy::U64 => "uint64",
            FieldTy::U128 => "uint128",
            FieldTy::I8 => "int8",
            FieldTy::I16 => "int16",
            FieldTy::I32 => "int32",
            FieldTy::I64 => "int64",
            FieldTy::I128 => "int128",
            FieldTy::Bool => "bool",
            FieldTy::Address => "address",
            FieldTy::Hash32 => "bytes32",
            FieldTy::Bytes => "bytes",
            FieldTy::String => "string",
        }
    }

    fn rust_ty(&self) -> TokenStream2 {
        match self {
            FieldTy::U8 => quote! { u8 },
            FieldTy::U16 => quote! { u16 },
            FieldTy::U32 => quote! { u32 },
            FieldTy::U64 => quote! { u64 },
            FieldTy::U128 => quote! { u128 },
            FieldTy::I8 => quote! { i8 },
            FieldTy::I16 => quote! { i16 },
            FieldTy::I32 => quote! { i32 },
            FieldTy::I64 => quote! { i64 },
            FieldTy::I128 => quote! { i128 },
            FieldTy::Bool => quote! { bool },
            FieldTy::Address => quote! { ::pyde_host::Address },
            FieldTy::Hash32 => quote! { ::pyde_host::Hash32 },
            FieldTy::Bytes => quote! { ::alloc::vec::Vec<u8> },
            FieldTy::String => quote! { ::alloc::string::String },
        }
    }

    /// Whether this type is allowed as an indexed (topic) field.
    ///
    /// v1 accepts:
    ///   - 32-byte fixed types (`address`, `hash32`, `bytes32`) →
    ///     stored as the raw 32 bytes.
    ///   - Fixed-width primitives (`u8`..`u128`, `i8`..`i128`,
    ///     `bool`) → stored as little-endian bytes right-padded
    ///     with zeros to 32 bytes. Matches Pyde's LE-everywhere
    ///     convention; clients filter by the same padded shape.
    ///
    /// Variable-width types (`bytes`, `string`) are still
    /// rejected — indexing them would need a hashing rule (e.g.
    /// `Blake3(value)`) the chain hasn't committed to yet.
    fn is_topic_allowed(&self) -> bool {
        matches!(
            self,
            FieldTy::U8
                | FieldTy::U16
                | FieldTy::U32
                | FieldTy::U64
                | FieldTy::U128
                | FieldTy::I8
                | FieldTy::I16
                | FieldTy::I32
                | FieldTy::I64
                | FieldTy::I128
                | FieldTy::Bool
                | FieldTy::Address
                | FieldTy::Hash32
        )
    }
}

// ─────────────────────────────────────────────────────────────────────
// Field model
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct Field {
    name: String,
    ty: FieldTy,
    indexed: bool,
}

#[derive(Debug)]
struct EventModel {
    name: String,
    /// Canonical Solidity-style signature derived from typed
    /// fields, e.g. `Transfer(address,address,uint128)`. This is
    /// what `topic_0` hashes, and what SDKs reconstructing
    /// topic-0 from the typed ABI converge on. Independent of
    /// however the author spelled the literal in `otigen.toml`.
    signature: String,
    /// Author's literal `signature = "..."` from `otigen.toml`,
    /// preserved verbatim for the generated docs only. Not used
    /// for hashing; not used by SDKs.
    signature_lit: String,
    topic_0: [u8; 32],
    fields: Vec<Field>,
}

/// Build the canonical signature string for an event from its
/// typed fields: `Name(canonical_ty1,canonical_ty2,...)`. This
/// is the wire-stable form SDKs reconstruct from the ABI's
/// `(name, params)` pair, and the input to `Blake3` for the
/// chain's `topic[0]`.
fn canonical_signature(name: &str, fields: &[Field]) -> String {
    let mut s = String::with_capacity(name.len() + fields.len() * 8 + 2);
    s.push_str(name);
    s.push('(');
    for (i, f) in fields.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(f.ty.canonical_str());
    }
    s.push(')');
    s
}

fn lower_event(name: &str, raw: &RawEvent) -> Result<EventModel, String> {
    let mut fields = Vec::with_capacity(raw.fields.len());
    let mut indexed_count = 0;
    for f in &raw.fields {
        let ty =
            FieldTy::parse(&f.ty).map_err(|e| format!("event `{name}` field `{}`: {e}", f.name))?;
        if f.indexed {
            if !ty.is_topic_allowed() {
                return Err(format!(
                    "event `{name}` field `{}`: type `{}` cannot be indexed (only address/hash32/bytes32 are 32-byte fixed-width — variable-width indexed values would need a hashing rule the chain hasn't committed to yet)",
                    f.name, f.ty
                ));
            }
            indexed_count += 1;
            if indexed_count > 3 {
                return Err(format!(
                    "event `{name}`: too many indexed fields ({indexed_count}); max 3 (topic[0] is reserved for the signature hash; chain caps total topics at 4)"
                ));
            }
        }
        fields.push(Field {
            name: f.name.clone(),
            ty,
            indexed: f.indexed,
        });
    }
    // Topic-0 must be derived from the canonical signature so
    // the chain's hash matches what SDKs reconstruct from the
    // typed ABI alone. The author's literal `raw.signature`
    // becomes documentary only — kept in `signature_lit` for
    // the generated module docs.
    let signature = canonical_signature(name, &fields);
    let topic_0 = blake3::hash(signature.as_bytes()).as_bytes().to_owned();
    Ok(EventModel {
        name: name.to_string(),
        signature,
        signature_lit: raw.signature.clone(),
        topic_0,
        fields,
    })
}

// ─────────────────────────────────────────────────────────────────────
// Codegen
// ─────────────────────────────────────────────────────────────────────

/// `declare_events!()` — read otigen.toml at compile time, emit
/// `mod events`. Takes no arguments.
#[proc_macro]
pub fn declare_events(input: TokenStream) -> TokenStream {
    let input2: TokenStream2 = input.into();
    if !input2.is_empty() {
        return compile_error("declare_events!() takes no arguments — otigen.toml is the event schema source of truth");
    }

    let manifest_dir = match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(d) => PathBuf::from(d),
        Err(_) => {
            return compile_error(
                "CARGO_MANIFEST_DIR not set; declare_events!() must run under cargo",
            )
        }
    };
    let toml_path = manifest_dir.join("otigen.toml");
    let toml_str = match std::fs::read_to_string(&toml_path) {
        Ok(s) => s,
        Err(e) => return compile_error(&format!(
            "failed to read {}: {e}\nhint: `declare_events!()` requires an otigen.toml at the crate root",
            toml_path.display()
        )),
    };

    let manifest: ManifestRoot = match toml::from_str(&toml_str) {
        Ok(m) => m,
        Err(e) => return compile_error(&format!("otigen.toml parse error: {e}")),
    };

    let mut events: Vec<EventModel> = Vec::with_capacity(manifest.events.len());
    for (name, raw) in &manifest.events {
        match lower_event(name, raw) {
            Ok(ev) => events.push(ev),
            Err(e) => return compile_error(&format!("otigen.toml [events]: {e}")),
        }
    }

    let event_items: Vec<TokenStream2> = events.iter().map(emit_event_struct).collect();

    // Make cargo re-run the proc-macro when otigen.toml changes.
    let toml_path_str = toml_path.to_string_lossy().into_owned();
    let dep_anchor = quote! {
        const _: &[u8] = include_bytes!(#toml_path_str);
    };

    let expanded = quote! {
        pub mod events {
            #![allow(non_camel_case_types, clippy::module_name_repetitions, dead_code)]
            extern crate alloc;

            #dep_anchor

            #(#event_items)*
        }
    };
    expanded.into()
}

fn compile_error(msg: &str) -> TokenStream {
    let lit = proc_macro2::Literal::string(msg);
    quote! { compile_error!(#lit); }.into()
}

fn emit_event_struct(ev: &EventModel) -> TokenStream2 {
    let struct_ident = format_ident!("{}", ev.name);
    let signature_lit = proc_macro2::Literal::string(&ev.signature);
    let signature_author_lit = proc_macro2::Literal::string(&ev.signature_lit);

    let topic_0_bytes: Vec<TokenStream2> = ev
        .topic_0
        .iter()
        .map(|b| {
            let lit = proc_macro2::Literal::u8_unsuffixed(*b);
            quote! { #lit }
        })
        .collect();

    // Build field list with the visibility + type annotations
    let field_defs: Vec<TokenStream2> = ev
        .fields
        .iter()
        .map(|f| {
            let name = format_ident!("{}", f.name);
            let ty = f.ty.rust_ty();
            quote! { pub #name: #ty, }
        })
        .collect();

    // Topics array body. Slots:
    //   topics[0] = Self::TOPIC_0
    //   topics[1..N+1] = indexed fields (each is a 32-byte fixed type)
    let indexed_fields: Vec<&Field> = ev.fields.iter().filter(|f| f.indexed).collect();
    let non_indexed_fields: Vec<&Field> = ev.fields.iter().filter(|f| !f.indexed).collect();

    let topic_inits: Vec<TokenStream2> = indexed_fields
        .iter()
        .map(|f| {
            let name = format_ident!("{}", f.name);
            topic_from_field(&f.ty, &name)
        })
        .collect();

    let topics_array_len = 1 + indexed_fields.len();
    let topics_array_len_lit = proc_macro2::Literal::usize_unsuffixed(topics_array_len);

    // Data encoding: borsh-style. For a single non-indexed field of
    // fixed-width type we can lean on the value's own to_le_bytes /
    // bytes-cast (same wire shape borsh produces, fewer alloc / decode
    // hops). For multi-field events we build a Vec<u8> + extend.
    let data_emit = build_data_emit(&non_indexed_fields);

    quote! {
        /// Type-safe emitter for an event declared in otigen.toml.
        ///
        /// Topic-0 = `Blake3(signature)`, precomputed at macro expansion.
        pub struct #struct_ident {
            #(#field_defs)*
        }

        impl #struct_ident {
            /// `Blake3(signature)`. Same value the chain stores as
            /// `topic[0]` for every emission, so off-chain
            /// subscribers can filter by this hash.
            pub const TOPIC_0: ::pyde_host::Hash32 = [#(#topic_0_bytes),*];

            /// Canonical Solidity-style signature derived from
            /// the typed fields, e.g. `Foo(uint64,address)`.
            /// This is the exact string Blake3-hashed into
            /// `TOPIC_0`; SDKs that reconstruct topic-0 from
            /// the typed ABI converge on the same string.
            ///
            /// The author's literal spelling from `otigen.toml`
            /// is preserved in [`Self::SIGNATURE_LIT`] for docs.
            pub const SIGNATURE: &'static str = #signature_lit;

            /// The author's literal `signature = "..."` from
            /// `otigen.toml`, byte-for-byte. Documentary only —
            /// not used for hashing. See [`Self::SIGNATURE`]
            /// for the canonical form.
            pub const SIGNATURE_LIT: &'static str = #signature_author_lit;


            /// Emit this event. Builds the topics array (topic[0] =
            /// the signature hash, topic[1..] = the indexed field
            /// values in declaration order) and the data payload
            /// (borsh-shape concatenation of non-indexed fields) and
            /// calls `pyde::emit_event`.
            pub fn emit(&self) {
                let topics: [::pyde_host::Hash32; #topics_array_len_lit] = [
                    Self::TOPIC_0,
                    #(#topic_inits)*
                ];
                #data_emit
            }
        }
    }
}

/// Build a 32-byte topic from a (validated-as-indexable) field.
///
/// Fixed 32-byte types pass through directly. Primitives (uN, iN,
/// bool) are little-endian, right-padded with zeros to 32 bytes
/// — Pyde's LE-everywhere convention, mirrored on the indexing
/// side so clients can filter by the same shape they decode
/// returns + storage in.
fn topic_from_field(ty: &FieldTy, name: &Ident) -> TokenStream2 {
    match ty {
        FieldTy::Address | FieldTy::Hash32 => quote! { self.#name, },
        FieldTy::U8 | FieldTy::I8 => quote! {
            {
                let mut __t = [0u8; 32];
                __t[0] = self.#name as u8;
                __t
            },
        },
        FieldTy::Bool => quote! {
            {
                let mut __t = [0u8; 32];
                __t[0] = if self.#name { 1u8 } else { 0u8 };
                __t
            },
        },
        FieldTy::U16
        | FieldTy::U32
        | FieldTy::U64
        | FieldTy::U128
        | FieldTy::I16
        | FieldTy::I32
        | FieldTy::I64
        | FieldTy::I128 => {
            let width = match ty {
                FieldTy::U16 | FieldTy::I16 => 2,
                FieldTy::U32 | FieldTy::I32 => 4,
                FieldTy::U64 | FieldTy::I64 => 8,
                FieldTy::U128 | FieldTy::I128 => 16,
                _ => unreachable!(),
            };
            let width_lit = proc_macro2::Literal::usize_unsuffixed(width);
            quote! {
                {
                    let mut __t = [0u8; 32];
                    let __bytes = self.#name.to_le_bytes();
                    __t[..#width_lit].copy_from_slice(&__bytes);
                    __t
                },
            }
        }
        FieldTy::Bytes | FieldTy::String => {
            // Rejected at parse time, but keep the arm defensive.
            unreachable!("variable-width indexed type slipped past lower_event")
        }
    }
}

/// Emit the data buffer + the `pyde::emit_event(&topics, &data)` call.
///
/// The wire shape mirrors `borsh::to_vec(&tuple_of_non_indexed_fields)`
/// — fixed-width primitives concatenate as LE bytes, addresses /
/// hash32 / bytes32 concatenate as raw 32-byte chunks, bytes / string
/// gain a u32_le length prefix.
fn build_data_emit(non_indexed: &[&Field]) -> TokenStream2 {
    // Fast path: single fixed-width field → cast directly. The
    // ergonomic difference vs. the buffer path is small here, but
    // bundle size + gas tracking are happier with one fewer
    // allocation hop.
    if non_indexed.len() == 1 {
        let f = non_indexed[0];
        let name = format_ident!("{}", f.name);
        let bytes_expr = single_field_as_bytes(&f.ty, &name);
        return quote! {
            let __data_bytes = #bytes_expr;
            ::pyde_host::emit_event(&topics, __data_bytes.as_ref());
        };
    }
    // Multi-field path: build a Vec<u8> + extend per field.
    if non_indexed.is_empty() {
        return quote! {
            ::pyde_host::emit_event(&topics, &[]);
        };
    }
    let append_stmts: Vec<TokenStream2> = non_indexed
        .iter()
        .map(|f| {
            let name = format_ident!("{}", f.name);
            append_field_bytes(&f.ty, &name)
        })
        .collect();
    quote! {
        let mut __data: ::alloc::vec::Vec<u8> = ::alloc::vec::Vec::new();
        #(#append_stmts)*
        ::pyde_host::emit_event(&topics, &__data);
    }
}

/// Express a single field's value as an `AsRef<[u8]>`-shaped binding
/// (no allocation when the field is a fixed-width primitive).
fn single_field_as_bytes(ty: &FieldTy, name: &Ident) -> TokenStream2 {
    match ty {
        FieldTy::U8 | FieldTy::I8 => quote! { [self.#name as u8] },
        FieldTy::Bool => quote! { [if self.#name { 1u8 } else { 0u8 }] },
        FieldTy::U16
        | FieldTy::U32
        | FieldTy::U64
        | FieldTy::U128
        | FieldTy::I16
        | FieldTy::I32
        | FieldTy::I64
        | FieldTy::I128 => {
            quote! { self.#name.to_le_bytes() }
        }
        FieldTy::Address | FieldTy::Hash32 => quote! { self.#name },
        FieldTy::Bytes => {
            quote! {
                {
                    let mut __v: ::alloc::vec::Vec<u8> = ::alloc::vec::Vec::with_capacity(4 + self.#name.len());
                    __v.extend_from_slice(&(self.#name.len() as u32).to_le_bytes());
                    __v.extend_from_slice(&self.#name);
                    __v
                }
            }
        }
        FieldTy::String => {
            quote! {
                {
                    let __b = self.#name.as_bytes();
                    let mut __v: ::alloc::vec::Vec<u8> = ::alloc::vec::Vec::with_capacity(4 + __b.len());
                    __v.extend_from_slice(&(__b.len() as u32).to_le_bytes());
                    __v.extend_from_slice(__b);
                    __v
                }
            }
        }
    }
}

/// Append a field's encoded bytes to `__data: Vec<u8>` (multi-field path).
fn append_field_bytes(ty: &FieldTy, name: &Ident) -> TokenStream2 {
    match ty {
        FieldTy::U8 | FieldTy::I8 => quote! { __data.push(self.#name as u8); },
        FieldTy::Bool => quote! { __data.push(if self.#name { 1u8 } else { 0u8 }); },
        FieldTy::U16
        | FieldTy::U32
        | FieldTy::U64
        | FieldTy::U128
        | FieldTy::I16
        | FieldTy::I32
        | FieldTy::I64
        | FieldTy::I128 => quote! {
            __data.extend_from_slice(&self.#name.to_le_bytes());
        },
        FieldTy::Address | FieldTy::Hash32 => quote! {
            __data.extend_from_slice(&self.#name);
        },
        FieldTy::Bytes => quote! {
            __data.extend_from_slice(&(self.#name.len() as u32).to_le_bytes());
            __data.extend_from_slice(&self.#name);
        },
        FieldTy::String => quote! {
            {
                let __b = self.#name.as_bytes();
                __data.extend_from_slice(&(__b.len() as u32).to_le_bytes());
                __data.extend_from_slice(__b);
            }
        },
    }
}

impl core::fmt::Display for FieldTy {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            FieldTy::U8 => "u8",
            FieldTy::U16 => "u16",
            FieldTy::U32 => "u32",
            FieldTy::U64 => "u64",
            FieldTy::U128 => "u128",
            FieldTy::I8 => "i8",
            FieldTy::I16 => "i16",
            FieldTy::I32 => "i32",
            FieldTy::I64 => "i64",
            FieldTy::I128 => "i128",
            FieldTy::Bool => "bool",
            FieldTy::Address => "address",
            FieldTy::Hash32 => "hash32",
            FieldTy::Bytes => "bytes",
            FieldTy::String => "string",
        };
        f.write_str(s)
    }
}

// ─────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_field(name: &str, ty: &str, indexed: bool) -> RawField {
        RawField {
            name: name.into(),
            ty: ty.into(),
            indexed,
        }
    }

    #[test]
    fn parse_basic_primitives() {
        assert!(matches!(FieldTy::parse("u128").unwrap(), FieldTy::U128));
        assert!(matches!(
            FieldTy::parse("address").unwrap(),
            FieldTy::Address
        ));
        assert!(matches!(
            FieldTy::parse("bytes32").unwrap(),
            FieldTy::Hash32
        ));
        assert!(matches!(FieldTy::parse("hash32").unwrap(), FieldTy::Hash32));
        assert!(matches!(FieldTy::parse("string").unwrap(), FieldTy::String));
    }

    #[test]
    fn parse_rejects_unknown() {
        let err = FieldTy::parse("uint256").unwrap_err();
        assert!(err.contains("uint256"));
    }

    #[test]
    fn lower_event_token_transfer_shape() {
        let raw = RawEvent {
            signature: "Transfer(address,address,uint128)".into(),
            fields: vec![
                raw_field("from", "address", true),
                raw_field("to", "address", true),
                raw_field("amount", "uint128", false),
            ],
        };
        let ev = lower_event("Transfer", &raw).expect("lower");
        assert_eq!(ev.fields.len(), 3);
        assert!(ev.fields[0].indexed);
        assert!(ev.fields[1].indexed);
        assert!(!ev.fields[2].indexed);
        // Topic-0 should be Blake3 of the signature string.
        let expected = blake3::hash(b"Transfer(address,address,uint128)")
            .as_bytes()
            .to_owned();
        assert_eq!(ev.topic_0, expected);
    }

    #[test]
    fn lower_event_rejects_indexed_variable_type() {
        let raw = RawEvent {
            signature: "Bad(string,uint128)".into(),
            fields: vec![
                raw_field("name", "string", true),
                raw_field("amount", "uint128", false),
            ],
        };
        let err = lower_event("Bad", &raw).unwrap_err();
        assert!(err.contains("cannot be indexed"), "got: {err}");
    }

    #[test]
    fn lower_event_accepts_indexed_primitives() {
        // ERC721's Transfer shape uses an indexed `uint64` token_id;
        // the macro pads it to 32 bytes LE on the emit side.
        let raw = RawEvent {
            signature: "Transfer(address,address,uint64)".into(),
            fields: vec![
                raw_field("from", "address", true),
                raw_field("to", "address", true),
                raw_field("token_id", "uint64", true),
            ],
        };
        let ev = lower_event("Transfer", &raw).expect("indexed u64 ok");
        assert!(ev.fields.iter().all(|f| f.indexed));
    }

    #[test]
    fn lower_event_rejects_too_many_indexed() {
        let raw = RawEvent {
            signature: "Many(address,address,address,address,uint128)".into(),
            fields: vec![
                raw_field("a", "address", true),
                raw_field("b", "address", true),
                raw_field("c", "address", true),
                raw_field("d", "address", true), // 4th indexed → reject (max 3 + topic-0)
                raw_field("amount", "uint128", false),
            ],
        };
        let err = lower_event("Many", &raw).unwrap_err();
        assert!(err.contains("too many indexed"), "got: {err}");
    }

    #[test]
    fn topic_0_is_blake3_of_signature() {
        // Pin the exact hash for ERC20 Transfer — same constant the
        // hand-rolled erc20-token used before migrating to the macro.
        let raw = RawEvent {
            signature: "Transfer(address,address,uint128)".into(),
            fields: vec![
                raw_field("from", "address", true),
                raw_field("to", "address", true),
                raw_field("amount", "uint128", false),
            ],
        };
        let ev = lower_event("Transfer", &raw).unwrap();
        // First 4 bytes of the precomputed constant the legacy
        // contract carried at TOPIC_TRANSFER[..4] = [0x71, 0xfb, 0xa7, 0x2c]
        assert_eq!(&ev.topic_0[..4], &[0x71, 0xfb, 0xa7, 0x2c]);
    }

    /// The canonical signature is derived from typed fields, NOT
    /// from `raw.signature`. Two authors writing
    /// `Incremented(u64,u64,u64)` and
    /// `Incremented(uint64,uint64,uint64)` against the same
    /// `fields = [...]` produce byte-identical topic-0s.
    #[test]
    fn topic_0_is_independent_of_author_signature_spelling() {
        let fields = vec![
            raw_field("by", "uint64", false),
            raw_field("prev", "uint64", false),
            raw_field("next", "uint64", false),
        ];
        let solidity_style = RawEvent {
            signature: "Incremented(uint64,uint64,uint64)".into(),
            fields: fields.clone(),
        };
        let rust_style = RawEvent {
            signature: "Incremented(u64,u64,u64)".into(),
            fields: fields.clone(),
        };
        let labeled_style = RawEvent {
            signature: "Incremented(by:uint64,prev:uint64,next:uint64)".into(),
            fields,
        };
        let a = lower_event("Incremented", &solidity_style).unwrap();
        let b = lower_event("Incremented", &rust_style).unwrap();
        let c = lower_event("Incremented", &labeled_style).unwrap();
        assert_eq!(a.topic_0, b.topic_0, "u64 vs uint64 must converge");
        assert_eq!(a.topic_0, c.topic_0, "labeled vs unlabeled must converge");
        assert_eq!(a.signature, "Incremented(uint64,uint64,uint64)");
        assert_eq!(a.signature, b.signature);
        assert_eq!(a.signature, c.signature);
        // The author's literal is preserved verbatim in
        // `signature_lit` for docs even though the hash ignores it.
        assert_eq!(b.signature_lit, "Incremented(u64,u64,u64)");
        assert_eq!(
            c.signature_lit,
            "Incremented(by:uint64,prev:uint64,next:uint64)"
        );
    }

    /// Mixed field types use canonical Solidity-style spelling
    /// throughout: `hash32` → `bytes32`, `i64` → `int64`, etc.
    #[test]
    fn canonical_signature_uses_solidity_style_for_every_type() {
        let raw = RawEvent {
            signature: "Mixed(hash32,i64,bool)".into(),
            fields: vec![
                raw_field("digest", "hash32", false),
                raw_field("delta", "i64", false),
                raw_field("flag", "bool", false),
            ],
        };
        let ev = lower_event("Mixed", &raw).unwrap();
        assert_eq!(ev.signature, "Mixed(bytes32,int64,bool)");
        let expected = blake3::hash(b"Mixed(bytes32,int64,bool)")
            .as_bytes()
            .to_owned();
        assert_eq!(ev.topic_0, expected);
    }
}
