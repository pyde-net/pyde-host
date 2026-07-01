//! `#[pyde::entry]` — wraps a Rust function in the calldata-decode +
//! return-wrap shim required by Pyde's `() -> ()` entry ABI.
//!
//! ## What the macro does
//!
//! Pyde entry points are `() -> ()` at the WASM-function-boundary level
//! (HOST_FN_ABI §3.5.2). Args flow through the `pyde::calldata_*` host
//! fns; return data flows through `pyde::return`. Authors who write a
//! natural Rust signature like `fn deposit(amount: u128, to: Address)
//! -> Receipt` would have to hand-write that decode + return shim per
//! function. This macro generates it.
//!
//! ## Expansion shape
//!
//! Given:
//!
//! ```ignore
//! #[pyde::entry]
//! fn transfer(to: Address, amount: u128) -> bool {
//!     // ... business logic
//!     true
//! }
//! ```
//!
//! the macro emits:
//!
//! ```ignore
//! #[no_mangle]
//! pub extern "C" fn transfer() {
//!     fn __pyde_entry_transfer_inner(to: Address, amount: u128) -> bool {
//!         // ... business logic
//!         true
//!     }
//!     let __pyde_size = unsafe { ::pyde_host::raw::calldata_size() } as usize;
//!     let mut __pyde_buf = ::alloc::vec![0u8; __pyde_size];
//!     if __pyde_size > 0 {
//!         unsafe {
//!             ::pyde_host::raw::calldata_copy(0, __pyde_size as i32, __pyde_buf.as_mut_ptr());
//!         }
//!     }
//!     let (to, amount): (Address, u128) =
//!         ::borsh::BorshDeserialize::try_from_slice(&__pyde_buf)
//!             .expect("calldata decode");
//!     let __pyde_result = __pyde_entry_transfer_inner(to, amount);
//!     let __pyde_bytes =
//!         ::borsh::to_vec(&__pyde_result).expect("encode return");
//!     unsafe {
//!         ::pyde_host::raw::pyde_return(__pyde_bytes.as_ptr(), __pyde_bytes.len() as i32)
//!     };
//! }
//! ```
//!
//! ## Attributes belong in `otigen.toml`, NOT on the macro
//!
//! `view`, `payable`, `reentrant`, `constructor`, `fallback`,
//! `receive`, `sponsored` — all of these are semantic attributes the
//! chain enforces by reading the deployed `pyde.abi` custom section.
//! That section is built from the per-function blocks in
//! `otigen.toml`:
//!
//! ```toml
//! [functions.get]
//! attributes = ["entry", "view"]
//! ```
//!
//! The macro itself does **not** accept attribute arguments. Passing
//! any is a compile error pointing the author at `otigen.toml`. The
//! single source of truth dodges the divergence footgun where the
//! Rust source claims one thing and the deployed ABI says another.
//!
//! ## Author requirements
//!
//! The contract crate must depend on `pyde-host` (which re-exports
//! this macro) and `borsh`. `pyde-host`'s default features pull in a
//! global allocator (`dlmalloc`) so `Vec` works inside `#![no_std]`.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{parse_macro_input, FnArg, ItemFn, Pat, PatType, ReturnType, Type};

/// Wrap a function with the Pyde entry-point shim.
///
/// See the crate-level docs for the expansion shape.
#[proc_macro_attribute]
pub fn entry(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        // Attributes belong in otigen.toml. Reject here with a
        // clear pointer instead of silently accepting + ignoring,
        // which would create a Rust-says-X-but-ABI-says-Y
        // divergence footgun.
        let attr2: TokenStream2 = attr.into();
        return syn::Error::new_spanned(
            attr2,
            "#[pyde::entry] takes no arguments. Function attributes \
             (`view`, `payable`, `reentrant`, `constructor`, `fallback`, \
             `receive`, `sponsored`) belong in `otigen.toml` under the \
             matching `[functions.<name>] attributes = [...]` block. The \
             chain reads them from the deployed pyde.abi section; \
             duplicating them on the macro would create two sources of \
             truth that could silently diverge.",
        )
        .to_compile_error()
        .into();
    }

    let input = parse_macro_input!(item as ItemFn);
    match expand_entry(input) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand_entry(input: ItemFn) -> Result<TokenStream2, syn::Error> {
    let user_fn_name = input.sig.ident.clone();
    let user_fn_vis = input.vis.clone();
    let user_fn_attrs = input.attrs.clone();

    // Pyde entry points are `() -> ()` at the WASM boundary. We reject
    // any `async`, generic, or `unsafe` signatures upfront — none of
    // them have honest semantics under the chain's dispatch convention.
    if let Some(asyncness) = input.sig.asyncness {
        return Err(syn::Error::new_spanned(
            asyncness,
            "#[pyde::entry] cannot be applied to async functions — Pyde entry points run \
             synchronously inside the WASM call frame",
        ));
    }
    if !input.sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.sig.generics,
            "#[pyde::entry] cannot be applied to generic functions — entry points must be \
             monomorphized at the WASM boundary",
        ));
    }
    if let Some(unsafety) = input.sig.unsafety {
        return Err(syn::Error::new_spanned(
            unsafety,
            "#[pyde::entry] cannot be applied to `unsafe fn` — the host invokes entry points \
             from outside the contract, where `unsafe` carries no useful meaning",
        ));
    }
    for arg in &input.sig.inputs {
        if let FnArg::Receiver(rcv) = arg {
            return Err(syn::Error::new_spanned(
                rcv,
                "#[pyde::entry] cannot be applied to methods (functions with `self`/`&self`/\
                 `&mut self`) — entry points are free functions",
            ));
        }
    }

    let inner_ident = format_ident!("__pyde_entry_{}_inner", user_fn_name);

    // Extract typed parameters as a (binding, type) pair list.
    let typed_args: Vec<(Pat, Type)> = input
        .sig
        .inputs
        .iter()
        .filter_map(|a| match a {
            FnArg::Typed(PatType { pat, ty, .. }) => Some(((**pat).clone(), (**ty).clone())),
            FnArg::Receiver(_) => None,
        })
        .collect();

    let arg_bindings: Vec<Pat> = typed_args.iter().map(|(p, _)| p.clone()).collect();
    let arg_types: Vec<Type> = typed_args.iter().map(|(_, t)| t.clone()).collect();
    let inner_call_args: Vec<Pat> = arg_bindings.clone();

    // Decode block: empty when the function takes no args.
    //
    // Host ABI: `calldata_copy(out_ptr, out_len_ptr) -> i32`. The
    // contract writes the buffer's max-bytes (LE u32) to
    // `out_len_ptr`; the host caps at that limit, copies bytes to
    // `out_ptr`, and writes the actual length back to `out_len_ptr`.
    // (HOST_FN_ABI_SPEC §7.4.)
    let calldata_load: TokenStream2 = quote! {
        let __pyde_size = unsafe { ::pyde_host::raw::calldata_size() } as usize;
        let mut __pyde_buf = ::alloc::vec![0u8; __pyde_size];
        if __pyde_size > 0 {
            let mut __pyde_limit: [u8; 4] = (__pyde_size as u32).to_le_bytes();
            unsafe {
                ::pyde_host::raw::calldata_copy(
                    __pyde_buf.as_mut_ptr(),
                    __pyde_limit.as_mut_ptr(),
                );
            }
        }
    };
    let decode_block: TokenStream2 = if typed_args.is_empty() {
        quote! {}
    } else if typed_args.len() == 1 {
        let pat = &arg_bindings[0];
        let ty = &arg_types[0];
        quote! {
            #calldata_load
            let #pat: #ty =
                <#ty as ::borsh::BorshDeserialize>::try_from_slice(&__pyde_buf)
                    .expect("calldata decode");
        }
    } else {
        quote! {
            #calldata_load
            let ( #( #arg_bindings ),* ): ( #( #arg_types ),* ) =
                <( #( #arg_types ),* ) as ::borsh::BorshDeserialize>::try_from_slice(
                    &__pyde_buf,
                )
                .expect("calldata decode");
        }
    };

    // Inner-call + return-wrap. Unit returns don't go through
    // `pyde::return`; non-unit returns get borsh-encoded.
    let returns_unit = matches!(input.sig.output, ReturnType::Default)
        || matches!(&input.sig.output, ReturnType::Type(_, ty) if is_unit(ty));

    let invocation: TokenStream2 = if returns_unit {
        quote! {
            #inner_ident( #( #inner_call_args ),* );
        }
    } else {
        quote! {
            let __pyde_result = #inner_ident( #( #inner_call_args ),* );
            let __pyde_bytes =
                ::borsh::to_vec(&__pyde_result).expect("encode return");
            unsafe {
                ::pyde_host::raw::pyde_return(
                    __pyde_bytes.as_ptr(),
                    __pyde_bytes.len() as i32,
                )
            };
        }
    };

    // Rename the user's function (signature + body intact) so the
    // shim can call it. Strip the original `#[no_mangle]` / `pub
    // extern "C"` if the author left them on — the shim owns those.
    let mut inner_fn = input.clone();
    inner_fn.sig.ident = inner_ident.clone();
    inner_fn.vis = syn::Visibility::Inherited;
    inner_fn.attrs.clear();

    Ok(quote! {
        #( #user_fn_attrs )*
        #[no_mangle]
        #user_fn_vis extern "C" fn #user_fn_name() {
            #inner_fn

            #decode_block
            #invocation
        }
    })
}

/// True iff `ty` is the unit type `()` (in any of its syntactic shapes).
fn is_unit(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(t) if t.elems.is_empty())
}
