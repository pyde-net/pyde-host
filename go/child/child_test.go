// Conformance against the shared golden vectors
// (vectors/child_address.json, repo root). Go cannot run the Poseidon2
// step — identity-salt vectors carry the pre-hashed salt, so the hash
// itself is out of scope here; the preimage assembly and the salt
// source encodings are what these tests pin.

package child

import (
	"bytes"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

type vector struct {
	Name               string   `json:"name"`
	Parent             string   `json:"parent"`
	Template           string   `json:"template"`
	Salt               string   `json:"salt"`
	Preimage           string   `json:"preimage"`
	ChildAddress       string   `json:"child_address"`
	SaltSourceBorsh    string   `json:"salt_source_borsh"`
	SaltSourcePairArgs []string `json:"salt_source_pair_args"`
}

func loadVectors(t *testing.T) []vector {
	t.Helper()
	_, thisFile, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("runtime.Caller failed; cannot locate vectors file")
	}
	path := filepath.Join(filepath.Dir(thisFile), "..", "..", "vectors", "child_address.json")
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read golden vectors: %v", err)
	}
	var file struct {
		Vectors []vector `json:"vectors"`
	}
	if err := json.Unmarshal(raw, &file); err != nil {
		t.Fatalf("parse golden vectors: %v", err)
	}
	if len(file.Vectors) == 0 {
		t.Fatal("golden vectors file has no vectors")
	}
	return file.Vectors
}

func mustHex32(t *testing.T, field, s string) [32]byte {
	t.Helper()
	raw, err := hex.DecodeString(s)
	if err != nil {
		t.Fatalf("%s: bad hex: %v", field, err)
	}
	if len(raw) != 32 {
		t.Fatalf("%s: want 32 bytes, got %d", field, len(raw))
	}
	var out [32]byte
	copy(out[:], raw)
	return out
}

// Every vector's preimage must be reproduced byte-for-byte from
// (parent, template, salt).
func TestPreimageMatchesGoldenVectors(t *testing.T) {
	for _, v := range loadVectors(t) {
		v := v
		t.Run(v.Name, func(t *testing.T) {
			got := Preimage(
				mustHex32(t, "parent", v.Parent),
				mustHex32(t, "template", v.Template),
				mustHex32(t, "salt", v.Salt),
			)
			if gotHex := hex.EncodeToString(got[:]); gotHex != v.Preimage {
				t.Errorf("preimage mismatch\n got: %s\nwant: %s", gotHex, v.Preimage)
			}
		})
	}
}

// The layout constants, asserted explicitly: 107 bytes total, and the
// 11-byte ASCII domain tag "pyde-child:" (70 79 64 65 2d 63 68 69 6c
// 64 3a) at the front.
func TestPreimageLayout(t *testing.T) {
	var parent, template, salt [32]byte
	for i := range parent {
		parent[i] = 0xaa
		template[i] = 0xbb
		salt[i] = 0xcc
	}
	p := Preimage(parent, template, salt)

	if len(p) != 107 || PreimageLen != 107 {
		t.Fatalf("preimage length: want 107, got %d (PreimageLen=%d)", len(p), PreimageLen)
	}
	wantTag := []byte{0x70, 0x79, 0x64, 0x65, 0x2d, 0x63, 0x68, 0x69, 0x6c, 0x64, 0x3a}
	if !bytes.Equal(p[:11], wantTag) {
		t.Errorf("domain tag: want % x, got % x", wantTag, p[:11])
	}
	if !bytes.Equal(p[11:43], parent[:]) {
		t.Error("bytes 11..43 must be parent")
	}
	if !bytes.Equal(p[43:75], template[:]) {
		t.Error("bytes 43..75 must be template")
	}
	if !bytes.Equal(p[75:107], salt[:]) {
		t.Error("bytes 75..107 must be salt")
	}
}

// The unordered-pair vector records its args deliberately UNSORTED:
// the canonical encoding must sort them, both arg orders must agree,
// and the naive as-passed concatenation must NOT match — proving the
// sort is load-bearing.
func TestUnorderedPairEncoding(t *testing.T) {
	var found bool
	for _, v := range loadVectors(t) {
		if len(v.SaltSourcePairArgs) == 0 {
			continue
		}
		found = true
		if len(v.SaltSourcePairArgs) != 2 {
			t.Fatalf("%s: want 2 pair args, got %d", v.Name, len(v.SaltSourcePairArgs))
		}
		argA := mustHex32(t, "pair_args[0]", v.SaltSourcePairArgs[0])
		argB := mustHex32(t, "pair_args[1]", v.SaltSourcePairArgs[1])
		want, err := hex.DecodeString(v.SaltSourceBorsh)
		if err != nil || len(want) != 64 {
			t.Fatalf("%s: salt_source_borsh: want 64 hex bytes, got %d (err %v)", v.Name, len(want), err)
		}

		ab := UnorderedPairEncoding(argA, argB)
		ba := UnorderedPairEncoding(argB, argA)
		if !bytes.Equal(ab[:], want) {
			t.Errorf("%s: encoding(a, b) != salt_source_borsh\n got: %x\nwant: %x", v.Name, ab, want)
		}
		if ab != ba {
			t.Errorf("%s: encoding must be order-independent\n(a,b): %x\n(b,a): %x", v.Name, ab, ba)
		}
		naive := append(append([]byte{}, argA[:]...), argB[:]...)
		if bytes.Equal(naive, want) {
			t.Errorf("%s: naive as-passed concatenation equals salt_source_borsh — recorded args are sorted, vector no longer exercises the sort", v.Name)
		}
	}
	if !found {
		t.Fatal("no vector with salt_source_pair_args")
	}
}

// The counter vectors pin the borsh u64 encoding: 8-byte little-endian.
func TestCounterEncoding(t *testing.T) {
	want := map[string]uint64{
		"salt-of-counter-0": 0,
		"salt-of-counter-1": 1,
	}
	seen := 0
	for _, v := range loadVectors(t) {
		counter, ok := want[v.Name]
		if !ok {
			continue
		}
		seen++
		enc := CounterEncoding(counter)
		if got := hex.EncodeToString(enc[:]); got != v.SaltSourceBorsh {
			t.Errorf("%s: CounterEncoding(%d) = %s, want %s", v.Name, counter, got, v.SaltSourceBorsh)
		}
		// Redundant with the hex check, but pins the byte order against
		// an aliased LE reading rather than string comparison alone.
		if binary.LittleEndian.Uint64(enc[:]) != counter {
			t.Errorf("%s: encoding does not round-trip as LE u64", v.Name)
		}
	}
	if seen != len(want) {
		t.Fatalf("found %d of %d counter vectors", seen, len(want))
	}
}
