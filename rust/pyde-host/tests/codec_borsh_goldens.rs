//! Golden generator for the `@pyde-net/sdk` codec-parity vectors.
//!
//! Emits `vectors/codec_borsh.json`: for every Pyde `ScalarType`, ≥3
//! values (including the load-bearing edge cases — `u128::MAX`, negative
//! `i128`, empty + multi-element `vec`/`bytes`/`string`), each paired
//! with the byte-exact output of the reference `borsh::to_vec`.
//!
//! The AssemblyScript side (`assemblyscript-sdk/tests/codec_parity.spec.mjs`)
//! reads this file, re-encodes each `input` with `BorshEncoder`, and
//! asserts the bytes match `borsh` — proving the AS SDK is byte-identical
//! to Rust with NO mock host in the loop.
//!
//! Schema (per vector):
//!   name  : stable id, unique
//!   type  : ScalarType tag the AS driver switches on
//!   input : portable typed input — decimal string (ints), bool,
//!           hex string (address/hash32/bytes), raw string, or an
//!           array of decimal strings (vec_u64)
//!   borsh : hex of `borsh::to_vec(value)`
//!
//! Regenerate after an intentional change:
//!   cargo test -p pyde-host --test codec_borsh_goldens regenerate -- --ignored

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Vector {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    input: Value,
    borsh: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct VectorFile {
    #[serde(rename = "_meta")]
    meta: Value,
    vectors: Vec<Vector>,
}

fn vectors_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vectors/codec_borsh.json")
}

// ── vector constructors ────────────────────────────────────────────────
//
// One helper per input shape. `borsh` is always the reference encoding of
// the exact same Rust value the `input` field describes.

/// Integer types whose portable input is a decimal string.
fn v_int<T>(name: &str, ty: &str, value: T) -> Vector
where
    T: borsh::BorshSerialize + std::fmt::Display,
{
    Vector {
        name: name.into(),
        ty: ty.into(),
        input: Value::String(value.to_string()),
        borsh: hex::encode(borsh::to_vec(&value).unwrap()),
    }
}

fn v_bool(name: &str, value: bool) -> Vector {
    Vector {
        name: name.into(),
        ty: "bool".into(),
        input: Value::Bool(value),
        borsh: hex::encode(borsh::to_vec(&value).unwrap()),
    }
}

/// A fixed 32-byte value (address / hash32). Input hex is the raw 32
/// bytes; borsh of a `[u8; 32]` is those same 32 bytes (no length prefix).
fn v_fixed32(name: &str, ty: &str, value: [u8; 32]) -> Vector {
    Vector {
        name: name.into(),
        ty: ty.into(),
        input: Value::String(hex::encode(value)),
        borsh: hex::encode(borsh::to_vec(&value).unwrap()),
    }
}

/// Length-prefixed bytes (`Vec<u8>`). Input hex is the raw content;
/// borsh prepends a u32 LE length.
fn v_bytes(name: &str, value: Vec<u8>) -> Vector {
    Vector {
        name: name.into(),
        ty: "bytes".into(),
        input: Value::String(hex::encode(&value)),
        borsh: hex::encode(borsh::to_vec(&value).unwrap()),
    }
}

/// UTF-8 string. Input is the raw string; borsh is u32 LE byte-length +
/// UTF-8 bytes.
fn v_string(name: &str, value: &str) -> Vector {
    Vector {
        name: name.into(),
        ty: "string".into(),
        input: Value::String(value.to_string()),
        borsh: hex::encode(borsh::to_vec(&value.to_string()).unwrap()),
    }
}

/// `vec<u64>`. Input is an array of decimal strings; borsh is u32 LE
/// count + each element as 8-byte LE.
fn v_vec_u64(name: &str, value: Vec<u64>) -> Vector {
    Vector {
        name: name.into(),
        ty: "vec_u64".into(),
        input: Value::Array(value.iter().map(|e| Value::String(e.to_string())).collect()),
        borsh: hex::encode(borsh::to_vec(&value).unwrap()),
    }
}

fn render_file() -> VectorFile {
    let incrementing: [u8; 32] = std::array::from_fn(|i| i as u8);

    let vectors = vec![
        // ── unsigned integers ──────────────────────────────────────────
        v_int("u8_zero", "u8", 0u8),
        v_int("u8_one", "u8", 1u8),
        v_int("u8_max", "u8", u8::MAX),
        v_int("u16_zero", "u16", 0u16),
        v_int("u16_mid", "u16", 0x0102u16),
        v_int("u16_max", "u16", u16::MAX),
        v_int("u32_zero", "u32", 0u32),
        v_int("u32_one", "u32", 1u32),
        v_int("u32_max", "u32", u32::MAX),
        v_int("u64_zero", "u64", 0u64),
        v_int("u64_one", "u64", 1u64),
        v_int("u64_max", "u64", u64::MAX),
        v_int("u128_zero", "u128", 0u128),
        v_int("u128_one", "u128", 1u128),
        v_int("u128_max", "u128", u128::MAX),
        // ── signed integers (two's-complement LE) ──────────────────────
        v_int("i8_min", "i8", i8::MIN),
        v_int("i8_neg_one", "i8", -1i8),
        v_int("i8_max", "i8", i8::MAX),
        v_int("i16_min", "i16", i16::MIN),
        v_int("i16_neg_one", "i16", -1i16),
        v_int("i16_max", "i16", i16::MAX),
        v_int("i32_min", "i32", i32::MIN),
        v_int("i32_neg_one", "i32", -1i32),
        v_int("i32_max", "i32", i32::MAX),
        v_int("i64_min", "i64", i64::MIN),
        v_int("i64_neg_one", "i64", -1i64),
        v_int("i64_max", "i64", i64::MAX),
        v_int("i128_min", "i128", i128::MIN),
        v_int("i128_neg_one", "i128", -1i128),
        v_int("i128_max", "i128", i128::MAX),
        // ── bool ────────────────────────────────────────────────────────
        v_bool("bool_false", false),
        v_bool("bool_true", true),
        // ── address / hash32 (raw 32 bytes) ─────────────────────────────
        v_fixed32("address_zero", "address", [0u8; 32]),
        v_fixed32("address_incrementing", "address", incrementing),
        v_fixed32("address_max", "address", [0xffu8; 32]),
        v_fixed32("hash32_zero", "hash32", [0u8; 32]),
        v_fixed32("hash32_incrementing", "hash32", incrementing),
        v_fixed32("hash32_max", "hash32", [0xffu8; 32]),
        // ── bytes (length-prefixed) ─────────────────────────────────────
        v_bytes("bytes_empty", vec![]),
        v_bytes("bytes_single", vec![0xaa]),
        v_bytes("bytes_multi", vec![0x00, 0x01, 0x02, 0xfe, 0xff]),
        // ── string (length-prefixed UTF-8) ──────────────────────────────
        v_string("string_empty", ""),
        v_string("string_ascii", "hi"),
        v_string("string_unicode", "héllo🚀"),
        // ── vec<u64> ────────────────────────────────────────────────────
        v_vec_u64("vec_u64_empty", vec![]),
        v_vec_u64("vec_u64_single", vec![1]),
        v_vec_u64("vec_u64_multi", vec![1, 2, u64::MAX]),
    ];

    VectorFile {
        meta: json!({
            "description": "Golden borsh-encoding vectors for @pyde-net/sdk codec parity. \
                Every value is encoded by the reference `borsh::to_vec`; the AssemblyScript \
                SDK must reproduce every `borsh` field byte-for-byte.",
            "borsh_rules": "ints = fixed-width LE two's-complement; bool = 1 byte 0/1; \
                address/hash32 = 32 raw bytes (no prefix); bytes/string = u32_le(len) + payload; \
                vec<T> = u32_le(len) + fixed-stride elements.",
            "input_encoding": "decimal string (ints), JSON bool, hex string (address/hash32/bytes), \
                raw string (string), array of decimal strings (vec_u64).",
            "generator": "cargo test -p pyde-host --test codec_borsh_goldens regenerate -- --ignored",
        }),
        vectors,
    }
}

/// The committed vector file must equal what the typed cases produce —
/// guards the file against manual edits AND the code against drift.
#[test]
fn golden_vectors_hold() {
    let raw = std::fs::read_to_string(vectors_path()).expect(
        "vectors/codec_borsh.json missing — run \
         `cargo test -p pyde-host --test codec_borsh_goldens regenerate -- --ignored`",
    );
    let on_disk: VectorFile = serde_json::from_str(&raw).expect("vector file parses");
    let expected = render_file();
    assert_eq!(
        on_disk.vectors.len(),
        expected.vectors.len(),
        "vector count mismatch",
    );
    for (got, want) in on_disk.vectors.iter().zip(expected.vectors.iter()) {
        assert_eq!(got, want, "vector `{}` drifted from the generator", want.name);
    }
}

/// Every type family the acceptance criteria enumerate has ≥3 vectors.
#[test]
fn every_type_has_at_least_three_values() {
    use std::collections::BTreeMap;
    let file = render_file();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for v in &file.vectors {
        *counts.entry(v.ty.as_str()).or_default() += 1;
    }
    // bool has exactly two meaningful values; everything else needs ≥3.
    for (ty, n) in &counts {
        let min = if *ty == "bool" { 2 } else { 3 };
        assert!(n >= &min, "type `{ty}` has only {n} vectors (need {min})");
    }
    // All 15 ScalarType tags present.
    for ty in [
        "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128", "bool", "address",
        "hash32", "bytes", "string", "vec_u64",
    ] {
        assert!(counts.contains_key(ty), "missing type `{ty}`");
    }
}

/// Dev tool, not a test: rewrites the golden file from the typed cases.
#[test]
#[ignore]
fn regenerate() {
    let file = render_file();
    let json = format!(
        "{}\n",
        serde_json::to_string_pretty(&file).expect("serialize vectors"),
    );
    let path = vectors_path();
    std::fs::create_dir_all(path.parent().expect("vectors dir")).expect("mkdir vectors");
    std::fs::write(&path, json).expect("write vectors");
    println!("wrote {}", path.display());
}
