package pyde

// Import-purity + signature-parity gate: builds a probe contract that
// exercises the whole wrapper surface with TinyGo, then parses the resulting
// wasm and asserts every import is (1) from module `pyde`, (2) a known host-fn
// wire name, and (3) declared with the EXACT WASM signature the engine
// registers. The third check is the important one: it proves TinyGo lowered
// each raw //go:wasmimport stub to the same (params) -> (results) the engine's
// linker expects, so a result-arity or width drift (e.g. the balance/tx_hash/
// tx_value/consume_gas i32->void fixes) can never silently regress — a mismatch
// here is the same rejection the engine would raise at on-chain instantiation.
//
// Native test (no build tag): shells out to TinyGo, parses bytes. Skips when
// TinyGo is absent so `go test ./...` stays green without it.

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

// hostFnSig mirrors otigen-abi's host_fn_signature table (HOST_FN_ABI_SPEC §7):
// wire name -> "params;results", each a comma-joined valtype list ("" = none).
// i = i32, I = i64. Kept in lockstep with host_fns.go / the engine.
var hostFnSig = map[string]string{
	// §7.1 storage — raw
	"sload": "i,i,i;i", "sstore": "i,i,i;", "sdelete": "i;",
	// §7.1 typed storage
	"sstore_scalar": "i,i,i,i;i", "sload_scalar": "i,i,i,i;i", "sdelete_scalar": "i,i;i",
	"sstore_map1": "i,i,i,i,i,i;i", "sload_map1": "i,i,i,i,i,i;i", "sdelete_map1": "i,i,i,i;i",
	"sstore_map2": "i,i,i,i,i,i,i,i;i", "sload_map2": "i,i,i,i,i,i,i,i;i", "sdelete_map2": "i,i,i,i,i,i;i",
	"sstore_map3": "i,i,i,i,i,i,i,i,i,i;i", "sload_map3": "i,i,i,i,i,i,i,i,i,i;i", "sdelete_map3": "i,i,i,i,i,i,i,i;i",
	// §7.2 account
	"balance": "i,i;", "transfer": "i,i;i",
	// §7.3 context
	"caller": "i;i", "origin": "i;i", "self_address": "i;i",
	"wave_id": ";I", "wave_timestamp": ";I", "chain_id": ";I",
	// §7.4 tx context
	"tx_hash": "i;", "tx_value": "i;", "tx_gas_remaining": ";I",
	"calldata_size": ";i", "calldata_copy": "i,i;i",
	// §7.5 events
	"emit_event": "i,i,i,i;i",
	// §7.6 hashing
	"hash_blake3": "i,i,i;", "hash_poseidon2": "i,i,i;", "hash_keccak256": "i,i,i;",
	// §7.7 PQ
	"falcon_verify": "i,i,i,i,i;i",
	// §7.8 cross-contract
	"cross_call":        "i,i,i,i,i,i,I,i,i;i",
	"cross_call_static": "i,i,i,i,i,I,i,i;i",
	"delegate_call":     "i,i,i,i,i,I,i,i;i",
	// §7.9 halt
	"return": "i,i;", "revert": "i,i;",
	// §7.10 gas
	"consume_gas": "I;",
	// §7.11 beacon
	"beacon_get": "i;i",
	// §7.12 factory
	"instantiate": "i,i,i,i,i,I,i,i,i;i",
}

func TestImportPurityAndSignatures(t *testing.T) {
	tinygo, err := exec.LookPath("tinygo")
	if err != nil {
		t.Skip("tinygo not installed; skipping import gate")
	}

	out := filepath.Join(t.TempDir(), "probe.wasm")
	cmd := exec.Command(tinygo, "build",
		"-target=wasm-unknown", "-opt=z", "-no-debug",
		"-o", out, "./testdata/importprobe")
	if b, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("tinygo build failed: %v\n%s", err, b)
	}
	wasm, err := os.ReadFile(out)
	if err != nil {
		t.Fatalf("read probe wasm: %v", err)
	}
	imports, err := parseWasmImports(wasm)
	if err != nil {
		t.Fatalf("parse imports: %v", err)
	}
	if len(imports) == 0 {
		t.Fatal("probe imported nothing — build likely stripped everything")
	}

	t.Logf("probe imports %d host fns", len(imports))
	for _, im := range imports {
		if im.module != "pyde" {
			t.Errorf("non-pyde import %s.%s — would fail on-chain instantiation", im.module, im.name)
			continue
		}
		want, ok := hostFnSig[im.name]
		if !ok {
			t.Errorf("unknown pyde import %q — not in HOST_FN_ABI_SPEC allowlist (wire-name typo?)", im.name)
			continue
		}
		got := im.params + ";" + im.results
		if got != want {
			t.Errorf("signature drift for %q: engine expects (%s), wasm declares (%s)", im.name, want, got)
		}
	}
}

type wasmImport struct {
	module, name    string
	params, results string // comma-joined valtypes ("i"=i32, "I"=i64), "" = none
}

// parseWasmImports reads the type section (id 1) and import section (id 2) of a
// wasm module and returns each function import with its resolved signature.
// Pure Go, no wasm runtime. Valid wasm orders section 1 before 2, so a single
// forward pass resolves func-import typeidx against the type table.
func parseWasmImports(d []byte) ([]wasmImport, error) {
	if len(d) < 8 || string(d[:4]) != "\x00asm" {
		return nil, errBadWasm
	}
	pos := 8
	uleb := func() (uint64, error) {
		var r uint64
		var s uint
		for {
			if pos >= len(d) {
				return 0, errTruncated
			}
			b := d[pos]
			pos++
			r |= uint64(b&0x7f) << s
			if b&0x80 == 0 {
				return r, nil
			}
			s += 7
		}
	}
	readName := func() (string, error) {
		n, err := uleb()
		if err != nil {
			return "", err
		}
		if pos+int(n) > len(d) {
			return "", errTruncated
		}
		s := string(d[pos : pos+int(n)])
		pos += int(n)
		return s, nil
	}
	valtype := func(b byte) string {
		switch b {
		case 0x7f:
			return "i"
		case 0x7e:
			return "I"
		case 0x7d:
			return "f32"
		case 0x7c:
			return "f64"
		default:
			return "?"
		}
	}

	var types []string // typeidx -> "params;results"
	var imports []wasmImport
	for pos < len(d) {
		sid := d[pos]
		pos++
		size, err := uleb()
		if err != nil {
			return nil, err
		}
		end := pos + int(size)
		if end > len(d) {
			return nil, errTruncated
		}
		switch sid {
		case 1: // type section
			count, err := uleb()
			if err != nil {
				return nil, err
			}
			for i := uint64(0); i < count; i++ {
				if pos >= len(d) || d[pos] != 0x60 {
					return nil, errBadWasm // not a functype
				}
				pos++
				pc, err := uleb()
				if err != nil {
					return nil, err
				}
				ps := make([]string, pc)
				for j := range ps {
					ps[j] = valtype(d[pos])
					pos++
				}
				rc, err := uleb()
				if err != nil {
					return nil, err
				}
				rs := make([]string, rc)
				for j := range rs {
					rs[j] = valtype(d[pos])
					pos++
				}
				types = append(types, strings.Join(ps, ",")+";"+strings.Join(rs, ","))
			}
		case 2: // import section
			count, err := uleb()
			if err != nil {
				return nil, err
			}
			for i := uint64(0); i < count; i++ {
				mod, err := readName()
				if err != nil {
					return nil, err
				}
				nm, err := readName()
				if err != nil {
					return nil, err
				}
				if pos >= len(d) {
					return nil, errTruncated
				}
				kind := d[pos]
				pos++
				switch kind {
				case 0x00: // func: typeidx
					ti, err := uleb()
					if err != nil {
						return nil, err
					}
					if int(ti) >= len(types) {
						return nil, errBadWasm
					}
					pr := strings.SplitN(types[ti], ";", 2)
					imports = append(imports, wasmImport{module: mod, name: nm, params: pr[0], results: pr[1]})
				case 0x01: // table
					pos++
					if err := skipLimits(uleb); err != nil {
						return nil, err
					}
				case 0x02: // mem
					if err := skipLimits(uleb); err != nil {
						return nil, err
					}
				case 0x03: // global
					pos += 2
				default:
					return nil, errBadWasm
				}
			}
		}
		pos = end
	}
	return imports, nil
}

// skipLimits consumes a wasm limits: flags(byte) + min(uleb) [+ max(uleb)].
func skipLimits(uleb func() (uint64, error)) error {
	flags, err := uleb()
	if err != nil {
		return err
	}
	if _, err := uleb(); err != nil {
		return err
	}
	if flags&0x01 != 0 {
		if _, err := uleb(); err != nil {
			return err
		}
	}
	return nil
}

var (
	errBadWasm   = wasmErr("not a wasm module")
	errTruncated = wasmErr("truncated wasm")
)

type wasmErr string

func (e wasmErr) Error() string { return string(e) }
