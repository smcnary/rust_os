#!/usr/bin/env python3
"""Boot the ISO and check that a scripted `help` prints the command list."""

import os
import pty
import select
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ISO = os.path.join(ROOT, "build", "rust_os.iso")


def main() -> int:
    if not os.path.exists(ISO):
        print(f"missing {ISO}; run scripts/mkiso.sh first", file=sys.stderr)
        return 1

    master, slave = pty.openpty()
    tty = os.ttyname(slave)
    qemu = subprocess.Popen(
        [
            "qemu-system-x86_64",
            "-machine",
            "pc",
            "-accel",
            "tcg",
            "-m",
            "128M",
            "-display",
            "none",
            "-serial",
            tty,
            "-cdrom",
            ISO,
            "-no-reboot",
            "-no-shutdown",
        ],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )
    # Keep this slave fd open. Closing it before QEMU opens the pty
    # makes reads on the master return EIO and the check exits early.
    buf = b""
    sent = False
    deadline = time.time() + 30
    try:
        while time.time() < deadline:
            if qemu.poll() is not None:
                break
            ready, _, _ = select.select([master], [], [], 0.2)
            if master in ready:
                try:
                    chunk = os.read(master, 4096)
                except OSError:
                    break
                if not chunk:
                    break
                buf += chunk
                sys.stdout.buffer.write(chunk)
                sys.stdout.buffer.flush()
            if not sent and b"rust_os ready" in buf:
                os.write(master, b"help\n")
                sent = True
            if sent and b"commands:" in buf and b"halt" in buf and b"echo" in buf:
                print("\nserial help check passed", file=sys.stderr)
                return 0
        print("\nserial help check failed", file=sys.stderr)
        print(buf.decode("utf-8", "replace"), file=sys.stderr)
        return 1
    finally:
        qemu.kill()
        qemu.wait(timeout=5)
        os.close(slave)
        os.close(master)
        err = qemu.stderr.read().decode("utf-8", "replace")
        if err.strip():
            print(err, file=sys.stderr)


if __name__ == "__main__":
    sys.exit(main())
