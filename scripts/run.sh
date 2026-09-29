#!/bin/sh
# Boot the ISO on the serial console. Type `help` at the `rust_os>` prompt.
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$root"
./scripts/mkiso.sh
exec qemu-system-x86_64 \
    -machine pc \
    -accel tcg \
    -m 128M \
    -display none \
    -serial stdio \
    -cdrom build/rust_os.iso \
    -no-reboot \
    -no-shutdown
