#!/usr/bin/env python3
"""
check_parity.py — verify the host-function extern surface is identical across
all four language bindings (Rust, AssemblyScript, Go, C).

CI (.github/workflows/ci.yml, parity job) invokes this script on every pull
request and every push to main. It exits non-zero if any host function
wire name appears in one language binding but not another, and prints a
matrix showing which language is missing which name.

The "wire name" is what the engine sees at WASM instantiation time — the
string in the module's import section. Source-language identifiers may
differ (e.g. Rust uses `pyde_return` for the wire name `return` because
`return` is a Rust keyword). This script always compares wire names.

Adding a new host function:
    1. Add `pub fn <wire>(...)` to rust/pyde-host/src/lib.rs (inside the
       `extern "C"` block). Use `#[link_name = "<wire>"]` above the fn
       if the source-language identifier must differ from the wire name.
    2. Add `@external("pyde", "<wire>")` + declaration to
       assemblyscript/src/host_fns.ts.
    3. Add `//go:wasmimport pyde <wire>` + func stub to go/host_fns.go.
    4. Add `PYDE_HOST_FN(<wire>);` to c/include/pyde/host.h, or
       (for C-keyword names) an inline `__attribute__((import_module("pyde"),
       import_name("<wire>"))) extern <type> <c_ident>(...)`.
    5. Re-run this script locally: `python3 scripts/check_parity.py`.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

RUST_SRC = ROOT / "rust" / "pyde-host" / "src" / "lib.rs"
AS_SRC = ROOT / "assemblyscript" / "src" / "host_fns.ts"
GO_SRC = ROOT / "go" / "host_fns.go"
C_SRC = ROOT / "c" / "include" / "pyde" / "host.h"


def _read(path: Path) -> str:
    if not path.exists():
        print(f"error: expected source file is missing: {path}", file=sys.stderr)
        sys.exit(2)
    return path.read_text(encoding="utf-8")


def _strip_block_and_line_comments(text: str) -> str:
    """Strip /* … */ and // … comments so parsers don't match inside them.
    Preserves line count so error messages still point at the right line
    if we later add position tracking."""
    # Strip /* … */ (non-greedy, multi-line)
    text = re.sub(r"/\*.*?\*/", lambda m: "\n" * m.group(0).count("\n"), text, flags=re.S)
    # Strip // … to end-of-line
    text = re.sub(r"//[^\n]*", "", text)
    return text


def parse_rust(text: str) -> set[str]:
    """Extract wire names from every `extern "C" { ... }` block in lib.rs.

    Rust source identifier may differ from the wire name when the fn has
    `#[link_name = "<wire>"]` attached (used for keyword collisions like
    `return`). We prefer link_name; else fall back to the fn identifier.
    """
    stripped = _strip_block_and_line_comments(text)
    names: set[str] = set()
    i = 0
    while True:
        start = stripped.find('extern "C"', i)
        if start == -1:
            break
        brace = stripped.find("{", start)
        if brace == -1:
            break
        depth = 1
        j = brace + 1
        while j < len(stripped) and depth > 0:
            c = stripped[j]
            if c == "{":
                depth += 1
            elif c == "}":
                depth -= 1
            j += 1
        block = stripped[brace + 1 : j - 1]

        # Walk fn-by-fn. For each `pub fn <ident>(`, look backward within
        # this block for a matching `#[link_name = "<wire>"]` attribute
        # that appears between the previous fn and this one.
        fn_iter = list(re.finditer(r"pub\s+fn\s+(\w+)\s*\(", block))
        prev_end = 0
        for m in fn_iter:
            preface = block[prev_end : m.start()]
            link = re.search(r'#\[\s*link_name\s*=\s*"([^"]+)"\s*\]', preface)
            wire = link.group(1) if link else m.group(1)
            names.add(wire)
            prev_end = m.end()

        i = j
    return names


def parse_assemblyscript(text: str) -> set[str]:
    """Extract wire names from AssemblyScript. The wire name is the second
    arg of the `@external("pyde", "<wire>")` decorator. The declaration
    identifier on the following line is ignored — it can differ from the
    wire name when a TS keyword collides (e.g. `return`)."""
    stripped = _strip_block_and_line_comments(text)
    names: set[str] = set()
    for m in re.finditer(
        r'@external\(\s*"pyde"\s*,\s*"([^"]+)"\s*\)', stripped
    ):
        names.add(m.group(1))
    return names


def parse_go(text: str) -> set[str]:
    """Extract wire names from Go. Each host fn is declared as:
        //go:wasmimport pyde <wire>
        func <goIdent>(...)
    The wire name is what the linker resolves against; the go identifier
    is a local rename."""
    # NB: don't strip // comments here — //go:wasmimport IS a magic
    # comment that TinyGo consumes.
    names: set[str] = set()
    for line in text.splitlines():
        m = re.match(r"\s*//go:wasmimport\s+pyde\s+(\w+)\s*$", line)
        if m:
            names.add(m.group(1))
    return names


def parse_c(text: str) -> set[str]:
    """Extract wire names from the C header.

    Two forms are supported:
        PYDE_HOST_FN(<wire>)          // preferred: macro expansion
        __attribute__((import_module("pyde"), import_name("<wire>"), ...))
                                      // fallback: inline attribute
                                      // (needed for C-keyword collisions
                                      //  like `return`)
    The macro DEFINITION line (`#define PYDE_HOST_FN(name)`) is skipped
    so we don't match the parameter `name` as a host fn.
    """
    stripped = _strip_block_and_line_comments(text)
    names: set[str] = set()

    # PYDE_HOST_FN(<wire>) — but not inside the #define line.
    for line in stripped.splitlines():
        if line.lstrip().startswith("#define"):
            continue
        for m in re.finditer(r"PYDE_HOST_FN\(\s*(\w+)\s*\)", line):
            names.add(m.group(1))

    # Inline __attribute__((..., import_name("<wire>"), ...))
    for m in re.finditer(
        r'__attribute__\s*\(\(\s*[^)]*?import_module\(\s*"pyde"\s*\)\s*,'
        r'\s*import_name\(\s*"([^"]+)"\s*\)',
        stripped,
    ):
        names.add(m.group(1))

    return names


def render_matrix(per_lang: dict[str, set[str]]) -> tuple[str, bool]:
    """Return (rendered_table, all_equal)."""
    union = sorted(set().union(*per_lang.values()))
    if not union:
        return "(no host functions discovered in any binding)", False

    langs = ["rust", "as", "go", "c"]
    name_col = max(len("name"), max(len(n) for n in union))
    header = (
        f"{'name'.ljust(name_col)} | "
        + " | ".join(l.ljust(4) for l in langs)
    )
    sep = "-" * len(header)
    rows = [header, sep]
    all_equal = True
    for name in union:
        cells = []
        for l in langs:
            present = name in per_lang[l]
            cells.append(("OK  " if present else "MISS").ljust(4))
            if not present:
                all_equal = False
        rows.append(f"{name.ljust(name_col)} | " + " | ".join(cells))
    return "\n".join(rows), all_equal


def main() -> int:
    per_lang = {
        "rust": parse_rust(_read(RUST_SRC)),
        "as": parse_assemblyscript(_read(AS_SRC)),
        "go": parse_go(_read(GO_SRC)),
        "c": parse_c(_read(C_SRC)),
    }

    print("host-fn parity check")
    print("=" * 60)
    for lang, names in per_lang.items():
        print(f"{lang}: {len(names)} names")
        for n in sorted(names):
            print(f"  - {n}")
        print()

    table, all_equal = render_matrix(per_lang)
    print("parity matrix")
    print("-" * 60)
    print(table)
    print()

    if all_equal and all(per_lang.values()):
        print("PASS: all four bindings expose the same host-fn surface.")
        return 0

    print("FAIL: host-fn surface is not consistent across bindings.")
    union = set().union(*per_lang.values())
    for lang, names in per_lang.items():
        missing = sorted(union - names)
        if missing:
            print(f"  {lang} missing: {', '.join(missing)}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
