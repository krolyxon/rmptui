#!/bin/sh

set -eu

BIN_DIR=/usr/local/bin
ACTION=install

usage() {
    cat <<'EOF'
Build and install rmptui from this checkout, or uninstall it.

Usage:
  ./install.sh [--bin-dir DIR]
  ./install.sh --uninstall [--bin-dir DIR]

Options:
  --bin-dir DIR  Install/remove rmptui in DIR (default: /usr/local/bin)
  --uninstall    Remove the installed rmptui executable
  -h, --help     Show this help
EOF
}

fail() {
    printf 'install.sh: %s\n' "$1" >&2
    exit 1
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --bin-dir)
            [ "$#" -ge 2 ] || fail "--bin-dir requires a directory"
            [ -n "$2" ] || fail "--bin-dir cannot be empty"
            BIN_DIR=$2
            shift 2
            ;;
        --uninstall)
            [ "$ACTION" = install ] || fail "--uninstall cannot be specified more than once"
            ACTION=uninstall
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            fail "unknown option: $1 (use --help for usage)"
            ;;
    esac
done

run_privileged() {
    if [ "$(id -u)" -eq 0 ]; then
        "$@"
    elif command -v sudo >/dev/null 2>&1; then
        sudo "$@"
    else
        fail "sudo is required to write to $BIN_DIR; install sudo or run this script as root"
    fi
}

SCRIPT_DIR=$(CDPATH= cd "$(dirname "$0")" && pwd)
DESTINATION=$BIN_DIR/rmptui

if [ "$ACTION" = uninstall ]; then
    if [ ! -e "$DESTINATION" ]; then
        printf 'rmptui is not installed at %s\n' "$DESTINATION"
        exit 0
    fi
    run_privileged rm -f "$DESTINATION"
    printf 'Removed %s\n' "$DESTINATION"
    exit 0
fi

if ! command -v cargo >/dev/null 2>&1; then
    fail "Cargo was not found. Install Rust and Cargo from https://rustup.rs/ and try again."
fi

printf 'Building rmptui from %s\n' "$SCRIPT_DIR"
cargo build --release --locked --manifest-path "$SCRIPT_DIR/Cargo.toml"

EXECUTABLE=$SCRIPT_DIR/target/release/rmptui
[ -x "$EXECUTABLE" ] || fail "build succeeded but executable was not found at $EXECUTABLE"

run_privileged install -d "$BIN_DIR"
run_privileged install -m 0755 "$EXECUTABLE" "$DESTINATION"
printf 'Installed rmptui to %s\n' "$DESTINATION"
