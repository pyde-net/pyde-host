package pyde

// Import-purity gate: builds a probe contract that exercises the wrapper
// surface with TinyGo, then parses the resulting wasm's import section and
// asserts every import is a known `pyde` host fn — no `env`, no WASI, no
// stray module. A contract that imports anything outside `pyde` is rejected
// by the engine at instantiation, so catching it here turns a deploy-time
// failure into a test failure.
//
// Native test (no build tag): it shells out to TinyGo and parses bytes;
// it does not itself import the tinygo-only wrappers. Skips when TinyGo is
// not installed so `go test ./...` stays green on machines without it.

import (
	"os"
	"os/exec"
	"path/filepath"
	"testing"
)

// knownHostFns is the allowlisted set of wire names a contract may import.
// Kept in sync with host_fns.go / HOST_FN_ABI_SPEC §7 (+ §8 parachain,
// §9 test-only). A wire-name typo in a //go:wasmimport directive surfaces
// here as an unknown import.
var knownHostFns = map[string]bool{
	"sload": true, "sstore": true, "sdelete": true,
	"sstore_scalar": true, "sload_scalar": true, "sdelete_scalar": true,
	"sstore_map1": true, "sload_map1": true, "sdelete_map1": true,
	"sstore_map2": true, "sload_map2": true, "sdelete_map2": true,
	"sstore_map3": true, "sload_map3": true, "sdelete_map3": true,
	"balance": true, "transfer": true,
	"caller": true, "origin": true, "self_address": true,
	"wave_id": true, "wave_timestamp": true, "chain_id": true,
	"tx_hash": true, "tx_value": true, "tx_gas_remaining": true,
	"calldata_size": true, "calldata_copy": true,
	"emit_event":  true,
	"hash_blake3": true, "hash_poseidon2": true, "hash_keccak256": true,
	"falcon_verify": true,
	"cross_call":    true, "cross_call_static": true, "delegate_call": true,
	"return": true, "revert": true,
	"consume_gas": true, "beacon_get": true, "instantiate": true,
}

func TestImportPurity(t *testing.T) {
	tinygo, err := exec.LookPath("tinygo")
	if err != nil {
		t.Skip("tinygo not installed; skipping import-purity gate")
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
		t.Fatal("probe imported nothing — build likely stripped everything; check the probe")
	}

	t.Logf("probe imports %d host fns", len(imports))
	for _, im := range imports {
		if im.module != "pyde" {
			t.Errorf("non-pyde import: %s.%s (contract would fail on-chain instantiation)", im.module, im.name)
			continue
		}
		if !knownHostFns[im.name] {
			t.Errorf("unknown pyde import %q — not in HOST_FN_ABI_SPEC allowlist (wire-name typo?)", im.name)
		}
	}
}

type wasmImport struct{ module, name string }

// parseWasmImports reads the import section (id 2) of a wasm module and
// returns its (module, name) entries. Pure Go, no wasm runtime.
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

	var imports []wasmImport
	for pos < len(d) {
		if pos >= len(d) {
			break
		}
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
		if sid != 2 { // only care about the import section
			pos = end
			continue
		}
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
			// Skip the import descriptor by kind.
			switch kind {
			case 0x00: // func: typeidx
				if _, err := uleb(); err != nil {
					return nil, err
				}
			case 0x01: // table: reftype(1) + limits
				pos++ // reftype
				if err := skipLimits(uleb); err != nil {
					return nil, err
				}
			case 0x02: // mem: limits
				if err := skipLimits(uleb); err != nil {
					return nil, err
				}
			case 0x03: // global: valtype(1) + mut(1)
				pos += 2
			default:
				return nil, errBadWasm
			}
			imports = append(imports, wasmImport{mod, nm})
		}
		pos = end
	}
	return imports, nil
}

// skipLimits consumes a wasm limits: flags(byte) + min(uleb) [+ max(uleb)].
func skipLimits(uleb func() (uint64, error)) error {
	// flags is the next byte, read via uleb (single-byte varint).
	flags, err := uleb()
	if err != nil {
		return err
	}
	if _, err := uleb(); err != nil { // min
		return err
	}
	if flags&0x01 != 0 { // has max
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
