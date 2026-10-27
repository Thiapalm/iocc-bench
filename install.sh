#!/usr/bin/env bash
# Installer for the iocc-bench plugin.
#
# Usage: ./install.sh [--prefix DIR] [--uninstall] [--help]
#   --prefix DIR   Install directory (default: folder of `iocc` if found in PATH,
#                  otherwise ~/.local/bin). Must be the same folder as iocc or in PATH.
#   --uninstall    Remove iocc-bench from the install directory
set -euo pipefail

BIN_DIR=""
UNINSTALL=0

usage() { sed -n '2,7p' "$0" | sed 's/^# \{0,1\}//'; }

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix)
            [ $# -ge 2 ] || { echo "error: --prefix requires a directory" >&2; exit 1; }
            BIN_DIR="$2"; shift 2 ;;
        --uninstall) UNINSTALL=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "error: unknown option: $1" >&2; usage >&2; exit 1 ;;
    esac
done

if [ -z "$BIN_DIR" ]; then
    if IOCC_PATH="$(command -v iocc 2>/dev/null)"; then
        BIN_DIR="$(dirname "$IOCC_PATH")"
    else
        BIN_DIR="${HOME}/.local/bin"
    fi
fi

if [ "$UNINSTALL" -eq 1 ]; then
    rm -fv "${BIN_DIR}/iocc-bench"
    echo "The bench section of config.yaml was left untouched."
    exit 0
fi

cd "$(dirname "$0")"

if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo not found. Install Rust from https://www.rust-lang.org/tools/install" >&2
    exit 1
fi

echo "==> Building iocc-bench (release)"
cargo build --release

echo "==> Installing to ${BIN_DIR}"
mkdir -p "$BIN_DIR"
install -m 755 target/release/iocc-bench "${BIN_DIR}/iocc-bench"

if ! command -v iocc >/dev/null 2>&1 && [ ! -x "${BIN_DIR}/iocc" ]; then
    echo "warning: iocc not found. Install it first; the plugin only runs through 'iocc bench'." >&2
fi

case ":${PATH}:" in
    *":${BIN_DIR}:"*) ;;
    *) echo "note: ${BIN_DIR} is not in your PATH. Add this to your shell profile:"
       echo "      export PATH=\"${BIN_DIR}:\$PATH\"" ;;
esac

echo "Done. Try: iocc bench --help"
echo "Settings go in the 'bench:' section of the iocc config.yaml (see ./config.yaml for an example)."
