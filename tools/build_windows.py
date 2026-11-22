"""Build the Windows x64 client, verify its GUI subsystem and collect third-party notices."""

import argparse
import collections
import hashlib
import json
import os
import pathlib
import shutil
import struct
import subprocess
import tempfile
from html.parser import HTMLParser

PROJECT = pathlib.Path(__file__).resolve().parents[1]
DEFAULT_TARGET = "x86_64-pc-windows-msvc" if os.name == "nt" else "x86_64-pc-windows-gnu"


def command(*args):
    return subprocess.check_output(args, cwd=PROJECT, text=True).strip()


def inspect_exe(path):
    with path.open("rb") as file:
        if file.read(2) != b"MZ":
            raise RuntimeError("Missing DOS signature")
        file.seek(60)
        pe = struct.unpack("<I", file.read(4))[0]
        file.seek(pe)
        if file.read(6) != b"PE\0\0\x64\x86":
            raise RuntimeError("Expected x86-64 Windows PE executable")
        file.seek(pe + 24)
        if file.read(2) != b"\x0b\x02":
            raise RuntimeError("Expected PE32+ optional header")
        file.seek(pe + 24 + 68)
        if struct.unpack("<H", file.read(2))[0] != 2:
            raise RuntimeError("Incorrect Windows subsystem for release kind")


def notices(metadata, path, target):
    groups = collections.defaultdict(list)
    summary = ["Frontline Surge — third-party components", ""]
    for item in sorted(metadata["packages"], key=lambda v: (v["name"], v["version"])):
        if not item["source"]:
            continue
        name = f'{item["name"]} {item["version"]}'
        summary.append(f'{name} — {item.get("license") or item.get("license_file")} — {", ".join(item["authors"])}')
        directory = pathlib.Path(item["manifest_path"]).parent
        paths = list(directory.iterdir())
        for folder in ("licenses", "LICENSES"):
            tree = directory / folder
            if tree.is_dir():
                paths.extend(tree.rglob("*"))
        for file in sorted(paths):
            base = file.name.lower()
            if not file.is_file() or not (file.parent != directory or base.startswith(("license", "licence", "copying", "notice", "copyright", "unlicense"))):
                continue
            content = file.read_text(encoding="utf8", errors="replace").strip()
            if content:
                groups[content].append(f"{name} — {file.name}")
    sections = ["\n".join(summary)]
    sections.extend("\n".join(names) + "\n\n" + text for text, names in groups.items())
    font = (PROJECT / "assets/FONT-LICENSE.txt").read_text(encoding="utf8")
    sections.append("User interface font\n\n" + font)
    bundled = PROJECT / "tools/notices"
    if target.endswith("-windows-gnu"):
        for name in ("mingw-w64.txt", "gcc-license.txt", "gcc-runtime-exception.txt"):
            sections.append(name + "\n\n" + (bundled / name).read_text(encoding="utf8"))
    # The standard library ships its compiled-in dependencies and exact notices
    # in this file with the normal official Rust distribution.
    standard = pathlib.Path(command("rustc", "--print", "sysroot")) / "share/doc/rust/COPYRIGHT-library.html"
    if standard.is_file():
        class LicenseText(HTMLParser):
            def handle_data(self, data):
                if data.strip():
                    self.parts.append(data.strip())
        parser = LicenseText()
        parser.parts = []
        parser.feed(standard.read_text(encoding="utf8"))
        sections.append("Rust standard library\n\n" + "\n".join(parser.parts))
    path.write_text("\n\n--------------------\n\n".join(sections) + "\n", encoding="utf8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=["x86_64-pc-windows-msvc", "x86_64-pc-windows-gnu"], default=DEFAULT_TARGET)
    parser.add_argument("--out", type=pathlib.Path, default=PROJECT / "dist/windows-client")
    parser.add_argument("--offline", action="store_true", help="use dependencies already cached by Cargo")
    args = parser.parse_args()
    flags = ["--locked", *(["--offline"] if args.offline else [])]
    subprocess.run(["cargo", "build", "--release", "--bin", "frontline-surge", "--target", args.target, *flags],
                   cwd=PROJECT, check=True)
    meta = json.loads(command("cargo", "metadata", "--format-version", "1", "--filter-platform", args.target, *flags))
    binary = pathlib.Path(meta["target_directory"]) / args.target / "release/frontline-surge.exe"
    inspect_exe(binary)
    output = args.out.resolve()
    output.mkdir(exist_ok=True, parents=True)
    with tempfile.TemporaryDirectory(dir=output) as stage:
        stage = pathlib.Path(stage)
        shutil.copy2(binary, stage / "FrontlineSurge.exe")
        notices(meta, stage / "THIRD-PARTY.txt", args.target)
        for item in stage.iterdir():
            item.replace(output / item.name)
    print(f"{output / 'FrontlineSurge.exe'}: SHA-256 {hashlib.sha256(binary.read_bytes()).hexdigest()}")


if __name__ == "__main__":
    main()
