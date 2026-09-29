# rust_os

A freestanding x86_64 kernel. It boots with the Limine BIOS bootloader, draws a text console on the framebuffer, mirrors that console to COM1, and runs a small keyboard shell.

This is one slice of an operating system: a bootable kernel with interrupts, a heap, and a shell. It does not have processes, a filesystem, or userspace.

## Shell

At the `rust_os>` prompt:

- `help` lists commands
- `echo <text>` prints the rest of the line
- `mem` prints heap size, bytes in use, and usable RAM from the memory map
- `ticks` prints how many timer interrupts have fired
- `clear` clears the screen
- `halt` stops the CPU

Input comes from the PS/2 keyboard and from the serial port.

## Build and boot

Requirements: Rust with the `x86_64-unknown-none` target, `make`, a C compiler, `xorriso`, and `qemu-system-x86_64`.

```sh
rustup target add x86_64-unknown-none
./scripts/run.sh
```

`scripts/run.sh` builds `build/rust_os.iso` and starts QEMU with the serial console on stdio. The framebuffer is also written; use a QEMU display instead of `-display none` if you want to see it.

Host tests cover the scancode decoder, the shell parser, and the heap. They do not boot the kernel:

```sh
cargo test --lib -p rust_os
```

The boot check sends `help` over the serial port and waits for the command list:

```sh
./scripts/mkiso.sh
python3 scripts/qemu-help-test.py
```

## Layout

The kernel crate is `kernel/`. Limine's BIOS stages, the UEFI boot image used to build the hybrid ISO, and the host install tool live in `third_party/limine` (BSD-2-Clause). The 8x8 glyphs in `kernel/src/font.rs` are public domain, from Daniel Hepper's font8x8, based on the public-domain IBM VGA font.
