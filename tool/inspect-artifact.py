#!/usr/bin/env python3
"""Inspect what is inside a build artifact, not just how big it is.

No status check looks inside an artifact. A build can succeed, upload a
package and ship something wrong without turning anything red. On
2026-11-10 the released Linux RPM was an 11 KB debuginfo package and the
build was green throughout (PR #34). The size gates added in PR #35
catch an artifact that is obviously not the application; they cannot
tell whether the right libraries are in it or whether its declared
dependencies match what its binaries actually need.

This script does that part, and keeps it in the repository so it does
not have to be reinvented each time. It uses only the standard library,
so it needs no rpm, dpkg or binutils and behaves the same on Windows,
macOS and Linux.

    python3 tool/inspect-artifact.py <artifact> [options]

      --require NAME   assert a file named NAME is present; repeatable.
                       Exits 1 when any is missing, so CI can use it.
      --needed         summarise DT_NEEDED across a directory tree
      --quiet          print assertion results only

Handles .apk, .ipa, .zip, .deb, .rpm, .tar.gz and directories. A .dmg
has to be unpacked with 7-Zip first; point this script at the result.
"""

from __future__ import annotations

import argparse
import io
import struct
import sys
import tarfile
import zipfile
from pathlib import Path

LIB_SUFFIXES = (".so", ".dll", ".dylib")

# Shared libraries every ELF has; listing them adds noise, not information.
BORING_SONAMES = ("libc.", "libm.", "libdl.", "libpthread.", "librt.",
                  "libstdc++", "libgcc_s", "ld-linux")


# ---------------------------------------------------------------- ELF ----

def elf_needed(data: bytes) -> list[str] | None:
    """DT_NEEDED entries of an ELF64 image, or None when not ELF64."""
    if len(data) < 64 or data[:4] != b"\x7fELF" or data[4] != 2:
        return None
    end = "<" if data[5] == 1 else ">"
    phoff, = struct.unpack_from(end + "Q", data, 0x20)
    phentsize, phnum = struct.unpack_from(end + "HH", data, 0x36)

    dynamic: tuple[int, int] | None = None
    loads: list[tuple[int, int, int]] = []
    for i in range(phnum):
        off = phoff + i * phentsize
        if off + 56 > len(data):
            return []
        p_type, = struct.unpack_from(end + "I", data, off)
        p_offset, p_vaddr = struct.unpack_from(end + "QQ", data, off + 0x08)
        p_filesz, = struct.unpack_from(end + "Q", data, off + 0x20)
        if p_type == 2:                                     # PT_DYNAMIC
            dynamic = (p_offset, p_filesz)
        elif p_type == 1:                                   # PT_LOAD
            loads.append((p_vaddr, p_offset, p_filesz))
    if dynamic is None:
        return []

    def to_offset(vaddr: int) -> int | None:
        for vstart, ostart, size in loads:
            if vstart <= vaddr < vstart + size:
                return ostart + (vaddr - vstart)
        return None

    offsets: list[int] = []
    strtab: int | None = None
    pos, stop = dynamic[0], dynamic[0] + dynamic[1]
    while pos + 16 <= stop:
        tag, value = struct.unpack_from(end + "QQ", data, pos)
        pos += 16
        if tag == 0:                                        # DT_NULL
            break
        if tag == 1:                                        # DT_NEEDED
            offsets.append(value)
        elif tag == 5:                                      # DT_STRTAB
            strtab = value
    if strtab is None:
        return []
    base = to_offset(strtab)
    if base is None:
        return []

    names = []
    for value in offsets:
        start = base + value
        names.append(data[start:data.index(b"\x00", start)].decode("utf-8", "replace"))
    return names


# ---------------------------------------------------------------- RPM ----

def _rpm_header(data: bytes, offset: int) -> tuple[dict[int, tuple[int, int, int]], int]:
    if data[offset:offset + 3] != b"\x8e\xad\xe8":
        raise ValueError(f"RPM header magic missing at {offset}")
    nindex, hsize = struct.unpack_from(">II", data, offset + 8)
    index = offset + 16
    store = index + nindex * 16
    entries = {}
    for i in range(nindex):
        tag, typ, doff, count = struct.unpack_from(">IIII", data, index + i * 16)
        entries[tag] = (typ, store + doff, count)
    return entries, store + hsize


def _rpm_strings(data: bytes, offset: int, count: int) -> list[str]:
    out, pos = [], offset
    for _ in range(count):
        stop = data.index(b"\x00", pos)
        out.append(data[pos:stop].decode("utf-8", "replace"))
        pos = stop + 1
    return out


def read_rpm(data: bytes) -> tuple[str, list[str], list[str]]:
    """Package name, file paths and Requires. RPM headers are uncompressed."""
    _, after_signature = _rpm_header(data, 96)
    header, _ = _rpm_header(data, (after_signature + 7) & ~7)

    name = ""
    if 1000 in header:                                      # NAME
        _, off, count = header[1000]
        name = _rpm_strings(data, off, count)[0]

    files: list[str] = []
    if {1116, 1117, 1118} <= header.keys():
        _, off, count = header[1117]                        # BASENAMES
        basenames = _rpm_strings(data, off, count)
        _, off, count = header[1118]                        # DIRNAMES
        dirnames = _rpm_strings(data, off, count)
        _, off, count = header[1116]                        # DIRINDEXES
        indexes = [struct.unpack_from(">I", data, off + i * 4)[0] for i in range(count)]
        files = [dirnames[indexes[i]] + basenames[i] for i in range(len(basenames))]

    requires: list[str] = []
    if 1049 in header:                                      # REQUIRENAME
        _, off, count = header[1049]
        requires = _rpm_strings(data, off, count)
    return name, files, requires


# ---------------------------------------------------------------- deb ----

def _ar_members(data: bytes):
    if data[:8] != b"!<arch>\n":
        raise ValueError("not an ar archive")
    pos = 8
    while pos + 60 <= len(data):
        header = data[pos:pos + 60]
        name = header[0:16].decode("utf-8", "replace").strip().rstrip("/")
        size = int(header[48:58].decode().strip())
        yield name, data[pos + 60:pos + 60 + size]
        pos += 60 + size + (size & 1)


def _decompress(name: str, body: bytes) -> bytes:
    if name.endswith(".gz"):
        import gzip
        return gzip.decompress(body)
    if name.endswith((".xz", ".lzma")):
        import lzma
        return lzma.decompress(body)
    if name.endswith(".zst"):
        from compression import zstd
        return zstd.decompress(body)
    return body


def read_deb(data: bytes) -> tuple[str, list[str]]:
    """The control file text and the paths in data.tar."""
    control, files = "", []
    for name, body in _ar_members(data):
        if name.startswith("control.tar"):
            tar = tarfile.open(fileobj=io.BytesIO(_decompress(name, body)))
            for member in tar.getmembers():
                if member.name.lstrip("./") == "control":
                    control = tar.extractfile(member).read().decode("utf-8")
        elif name.startswith("data.tar"):
            tar = tarfile.open(fileobj=io.BytesIO(_decompress(name, body)))
            files = [m.name.lstrip(".") for m in tar.getmembers() if m.isfile()]
    return control, files


def field(control: str, key: str) -> str:
    """One control field, continuation lines folded into a single line."""
    parts, collecting = [], False
    for line in control.splitlines():
        if line.startswith(key + ":"):
            collecting = True
            parts.append(line[len(key) + 1:])
        elif collecting and line[:1] in (" ", "\t"):
            parts.append(line)
        elif collecting:
            break
    return " ".join(p.strip() for p in parts).strip()


# ------------------------------------------------------------- dispatch ----

def gather(path: Path) -> tuple[str, list[tuple[str, int]], list[str]]:
    """Return (kind, [(name, size)], notes). A size of -1 means unknown."""
    notes: list[str] = []

    if path.is_dir():
        entries = [
            (str(p.relative_to(path)).replace("\\", "/"), p.stat().st_size)
            for p in path.rglob("*") if p.is_file()
        ]
        return "directory", entries, notes

    data = path.read_bytes()
    lower = path.name.lower()

    if lower.endswith(".rpm"):
        name, files, requires = read_rpm(data)
        shared = [r for r in requires if ".so" in r]
        notes.append(f"package: {name}")
        notes.append(f"requires: {len(requires)} entries, {len(shared)} shared libraries")
        for r in sorted(set(shared)):
            if not r.startswith(BORING_SONAMES):
                notes.append(f"    {r}")
        return "rpm", [(f, -1) for f in files], notes

    if lower.endswith(".deb"):
        control, files = read_deb(data)
        notes.append(f"package: {field(control, 'Package')}")
        notes.append(f"Depends: {field(control, 'Depends') or '(none)'}")
        return "deb", [(f, -1) for f in files], notes

    if zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as zf:
            entries = [(i.filename, i.file_size) for i in zf.infolist() if not i.is_dir()]
        kind = ("apk" if lower.endswith(".apk")
                else "ipa" if lower.endswith(".ipa") else "zip")
        return kind, entries, notes

    try:
        with tarfile.open(path) as tf:
            entries = [(m.name.lstrip("./"), m.size) for m in tf.getmembers() if m.isfile()]
        return "tar", entries, notes
    except tarfile.TarError:
        pass

    if lower.endswith(".dmg"):
        notes.append("Unpack with 7-Zip first, then run this script on the directory.")
        notes.append("Symlinks fail to extract on Windows; the real binary of a")
        notes.append("framework sits under Versions/A and is unaffected.")
        return "dmg", [], notes

    if elf_needed(data) is not None:
        notes.append("Plain ELF. An AppImage is ELF plus squashfs and is not unpacked here.")
        return "elf", [(path.name, len(data))], notes

    return "unknown", [], notes


def report_needed(root: Path) -> None:
    """DT_NEEDED across a tree, minus the libraries the tree ships itself."""
    bundled = {p.name for p in root.rglob("*.so")}
    external: dict[str, set[str]] = {}
    for p in root.rglob("*"):
        if not p.is_file():
            continue
        try:
            needed = elf_needed(p.read_bytes())
        except OSError:
            continue
        if not needed:
            continue
        rel = str(p.relative_to(root)).replace("\\", "/")
        for lib in needed:
            external.setdefault(lib, set()).add(rel)

    print()
    print("External shared libraries (excluding ones bundled here):")
    for lib in sorted(external):
        if lib in bundled:
            continue
        print(f"  {lib:<34} <- {', '.join(sorted(external[lib]))}")


def main() -> int:
    parser = argparse.ArgumentParser(description="Inspect a build artifact's contents")
    parser.add_argument("path", type=Path)
    parser.add_argument("--require", action="append", default=[], metavar="NAME",
                        help="assert a file named NAME is present; repeatable")
    parser.add_argument("--needed", action="store_true",
                        help="summarise DT_NEEDED across a directory tree")
    parser.add_argument("--quiet", action="store_true")
    args = parser.parse_args()

    if not args.path.exists():
        print(f"no such path: {args.path}")
        return 2

    kind, entries, notes = gather(args.path)
    unpacked = sum(size for _, size in entries if size >= 0)

    if not args.quiet:
        print(f"path: {args.path}")
        print(f"kind: {kind}")
        if args.path.is_file():
            print(f"size: {args.path.stat().st_size} bytes")
        print(f"files: {len(entries)}")
        if unpacked:
            print(f"unpacked: {unpacked} bytes")
        for note in notes:
            print(note)

        libraries = [(n, s) for n, s in entries
                     if n.endswith(LIB_SUFFIXES) or ".framework/" in n]
        if libraries:
            print()
            print(f"native libraries ({len(libraries)}):")
            for name, size in sorted(libraries, key=lambda item: -item[1])[:40]:
                print(f"  {size if size >= 0 else '-':>12}  {name}")

    if args.needed:
        if args.path.is_dir():
            report_needed(args.path)
        else:
            print()
            print("--needed only works on a directory; unpack the artifact first.")

    missing = [
        want for want in args.require
        if not any(Path(name).name == want or name == want for name, _ in entries)
    ]
    if args.require:
        print()
        for want in args.require:
            print(f"{'MISSING' if want in missing else 'ok     '}  {want}")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
