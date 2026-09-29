#!/bin/sh
# Build the kernel and a Limine BIOS ISO at build/rust_os.iso.
# The image is a hybrid (BIOS + UEFI El Torito) so `limine bios-install`
# can place its stage2 in the post-MBR gap. QEMU boots the BIOS path.
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$root"

make -C third_party/limine
cargo build --target x86_64-unknown-none --bin kernel

limine=third_party/limine
rm -rf build/iso
mkdir -p build/iso/boot build/iso/EFI/BOOT
cp target/x86_64-unknown-none/debug/kernel build/iso/boot/kernel
cp limine.conf build/iso/boot/limine.conf
cp "$limine/limine-bios.sys" build/iso/boot/limine-bios.sys
cp "$limine/limine-bios-cd.bin" build/iso/boot/limine-bios-cd.bin
cp "$limine/limine-uefi-cd.bin" build/iso/boot/limine-uefi-cd.bin
cp "$limine/BOOTX64.EFI" build/iso/EFI/BOOT/BOOTX64.EFI

xorriso -as mkisofs -R -r -J \
    -b boot/limine-bios-cd.bin \
    -no-emul-boot -boot-load-size 4 -boot-info-table \
    -hfsplus -apm-block-size 2048 \
    --efi-boot boot/limine-uefi-cd.bin \
    -efi-boot-part --efi-boot-image --protective-msdos-label \
    build/iso -o build/rust_os.iso
./third_party/limine/limine bios-install build/rust_os.iso
echo "wrote build/rust_os.iso"
