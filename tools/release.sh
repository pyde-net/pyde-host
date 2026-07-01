#!/usr/bin/env bash
# release.sh — local equivalent of .github/workflows/{ci,publish}.yml.
#
# Use when GitHub Actions can't run (org billing hold, offline, etc.)
# or when you want to verify a change before pushing. Mirrors the CI
# matrix so a local green here is what a green Actions run would be.
#
# Subcommands:
#   check                       run the full matrix: rust fmt/clippy/
#                               test/wasm, asc, tinygo, clang, clang++,
#                               parity. Exit 0 = clean.
#   publish [--dry-run]         run check first; then publish rust
#                               (crates.io), assemblyscript (npm), and
#                               tag the go submodule. --dry-run prints
#                               the commands without firing them.
#
# Publish auth:
#   crates.io: `cargo login <token>` once (persists to
#              ~/.cargo/credentials.toml). Get a token at
#              https://crates.io/settings/tokens.
#   npm:       `npm login` once (persists to ~/.npmrc). Prereq: the
#              @pyde-net scope exists on npm.
#   git tag:   uses your existing gh auth.
#
# Environment overrides:
#   CLANG_WASM_CC   / CLANG_WASM_CXX
#       C / C++ compiler with wasm32-unknown-unknown target. Auto-
#       detected: brew LLVM on macOS, system clang elsewhere.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

VERSION=$(sed -nE 's/^version = "([^"]+)".*/\1/p' rust/Cargo.toml | head -1)
[[ -z "$VERSION" ]] && { echo "release.sh: could not read version from rust/Cargo.toml"; exit 2; }
TAG="v$VERSION"
GO_TAG="go/v$VERSION"

# ── palette ────────────────────────────────────────────────────────
if [[ -t 1 ]]; then
    C_GREEN=$'\033[32m'; C_RED=$'\033[31m'; C_DIM=$'\033[2m'
    C_BOLD=$'\033[1m';  C_RESET=$'\033[0m'
else
    C_GREEN=""; C_RED=""; C_DIM=""; C_BOLD=""; C_RESET=""
fi

step() { echo "${C_BOLD}▸ $*${C_RESET}"; }
ok()   { echo "  ${C_GREEN}✓${C_RESET} $*"; }
fail() { echo "  ${C_RED}✗${C_RESET} $*"; }

# ── clang detection ────────────────────────────────────────────────
if [[ -z "${CLANG_WASM_CC:-}" ]]; then
    if [[ "$(uname)" == "Darwin" && -x /opt/homebrew/opt/llvm/bin/clang ]]; then
        CLANG_WASM_CC=/opt/homebrew/opt/llvm/bin/clang
        CLANG_WASM_CXX=/opt/homebrew/opt/llvm/bin/clang++
    else
        CLANG_WASM_CC=clang
        CLANG_WASM_CXX=clang++
    fi
fi
if ! "$CLANG_WASM_CC" -print-targets 2>&1 | grep -q wasm32; then
    fail "$CLANG_WASM_CC has no wasm32 target"
    echo "  Set CLANG_WASM_CC / CLANG_WASM_CXX to a wasm-capable clang."
    echo "  macOS: brew install llvm  → /opt/homebrew/opt/llvm/bin/clang{,++}"
    exit 2
fi

# ── check ──────────────────────────────────────────────────────────
run_rust() {
    step "rust: fmt / clippy / test / wasm build"
    (cd rust && cargo fmt --all -- --check) && ok "fmt clean" || { fail "fmt"; return 1; }
    (cd rust && cargo clippy --workspace --all-targets -- -D warnings) >/dev/null 2>&1 && ok "clippy clean" || { fail "clippy"; (cd rust && cargo clippy --workspace --all-targets -- -D warnings) || true; return 1; }
    (cd rust && cargo test --workspace) >/dev/null 2>&1 && ok "tests pass" || { fail "tests"; (cd rust && cargo test --workspace) || true; return 1; }
    (cd rust && cargo build --package pyde-host --target wasm32-unknown-unknown --release) >/dev/null 2>&1 \
        && ok "wasm build clean" || { fail "wasm build"; return 1; }
}

run_assemblyscript() {
    step "assemblyscript: asc compile"
    (cd assemblyscript && [[ -d node_modules ]] || npm ci >/dev/null 2>&1)
    (cd assemblyscript && npx --no-install asc src/index.ts --outFile /tmp/host-check-as.wasm --target release) \
        && ok "asc compile clean" || { fail "asc"; return 1; }
}

run_go() {
    step "go: tinygo build (smoke program importing pyde-host)"
    local smoke_dir
    smoke_dir=$(mktemp -d -t pyde-host-go-smoke.XXXXXX)
    cat > "$smoke_dir/go.mod" <<EOF
module smoke
go 1.21
require github.com/pyde-net/pyde-host/go v0.0.0
replace github.com/pyde-net/pyde-host/go => $REPO_ROOT/go
EOF
    cat > "$smoke_dir/main.go" <<'EOF'
package main

import _ "github.com/pyde-net/pyde-host/go"

func main() {}
EOF
    (cd "$smoke_dir" && tinygo build -target=wasi -o /tmp/host-check-go.wasm .) \
        && ok "tinygo build clean" || { fail "tinygo"; rm -rf "$smoke_dir"; return 1; }
    rm -rf "$smoke_dir"
}

run_c() {
    step "c: clang smoke build"
    cat > /tmp/pyde-host-check.c <<'EOF'
#include <pyde/host.h>
int main(void) { return 0; }
EOF
    "$CLANG_WASM_CC" \
        --target=wasm32-unknown-unknown \
        -nostdlib \
        -Wl,--no-entry \
        -Wl,--export-all \
        -I c/include \
        -o /tmp/host-check-c.wasm \
        /tmp/pyde-host-check.c \
        && ok "c smoke build clean" || { fail "c"; return 1; }
}

run_cpp() {
    step "c++: clang++ smoke build (guards extern \"C\")"
    cat > /tmp/pyde-host-check.cpp <<'EOF'
#include <pyde/host.h>
int main() { return 0; }
EOF
    "$CLANG_WASM_CXX" \
        --target=wasm32-unknown-unknown \
        -nostdlib \
        -Wl,--no-entry \
        -Wl,--export-all \
        -I c/include \
        -o /tmp/host-check-cpp.wasm \
        /tmp/pyde-host-check.cpp \
        && ok "c++ smoke build clean" || { fail "c++"; return 1; }
}

run_parity() {
    step "parity: signature check across all four bindings"
    python3 scripts/check_parity.py >/dev/null && ok "parity clean" || { fail "parity"; python3 scripts/check_parity.py || true; return 1; }
}

cmd_check() {
    echo "${C_DIM}version: $VERSION${C_RESET}"
    echo "${C_DIM}clang wasm: $CLANG_WASM_CC${C_RESET}"
    echo
    local failed=0
    run_rust          || failed=$((failed+1))
    run_assemblyscript || failed=$((failed+1))
    run_go            || failed=$((failed+1))
    run_c             || failed=$((failed+1))
    run_cpp           || failed=$((failed+1))
    run_parity        || failed=$((failed+1))
    echo
    if (( failed == 0 )); then
        echo "${C_GREEN}${C_BOLD}✓ all 6 jobs clean${C_RESET}  (mirrors GitHub Actions ci matrix)"
        return 0
    else
        echo "${C_RED}${C_BOLD}✗ $failed job(s) failed${C_RESET}"
        return 1
    fi
}

# ── publish ────────────────────────────────────────────────────────
publish_rust() {
    step "publish rust to crates.io (dep order)"
    local dry=$1
    for pkg in pyde-entry-macros pyde-storage-macros pyde-events-macros pyde-host; do
        if [[ $dry == 1 ]]; then
            echo "  ${C_DIM}(dry-run)${C_RESET} cargo publish --package $pkg --no-verify"
        else
            (cd rust && cargo publish --package "$pkg" --no-verify) && ok "$pkg published" \
                || { fail "$pkg"; return 1; }
        fi
    done
}

publish_npm() {
    step "publish assemblyscript to npm"
    local dry=$1
    if [[ $dry == 1 ]]; then
        echo "  ${C_DIM}(dry-run)${C_RESET} npm publish --access public   (in assemblyscript/)"
    else
        (cd assemblyscript && npm publish --access public) && ok "@pyde-net/host published" \
            || { fail "npm"; return 1; }
    fi
}

publish_go() {
    step "tag go submodule $GO_TAG"
    local dry=$1
    if [[ $dry == 1 ]]; then
        echo "  ${C_DIM}(dry-run)${C_RESET} git tag $GO_TAG && git push origin $GO_TAG"
    else
        if git rev-parse "$GO_TAG" >/dev/null 2>&1; then
            ok "$GO_TAG already exists"
        else
            git tag "$GO_TAG" && git push origin "$GO_TAG" && ok "$GO_TAG pushed"
        fi
    fi
}

cmd_publish() {
    local dry=0
    [[ "${1:-}" == "--dry-run" ]] && dry=1
    if [[ $dry == 0 ]]; then
        echo "${C_BOLD}About to publish v$VERSION to crates.io, npm, and tag $GO_TAG.${C_RESET}"
        echo "${C_DIM}Ctrl-C in the next 3s to abort.${C_RESET}"
        sleep 3
    fi
    cmd_check || { echo "${C_RED}check failed — refusing to publish${C_RESET}"; return 1; }
    echo
    publish_rust $dry
    publish_npm  $dry
    publish_go   $dry
    echo
    if [[ $dry == 1 ]]; then
        echo "${C_GREEN}${C_BOLD}dry-run complete${C_RESET}"
    else
        echo "${C_GREEN}${C_BOLD}✓ published v$VERSION${C_RESET}"
        echo "  crates.io:  https://crates.io/crates/pyde-host"
        echo "  npm:        https://www.npmjs.com/package/@pyde-net/host"
        echo "  pkg.go.dev: https://pkg.go.dev/github.com/pyde-net/pyde-host/go@$GO_TAG"
    fi
}

# ── dispatch ───────────────────────────────────────────────────────
case "${1:-}" in
    check)           cmd_check ;;
    publish)         shift; cmd_publish "$@" ;;
    *)               echo "usage: release.sh { check | publish [--dry-run] }"; exit 2 ;;
esac
