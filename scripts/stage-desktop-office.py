#!/usr/bin/env python3
"""Stage the existing anydoc backend into Windows' isolated embedded Python.

No pip, global installation or host Python packages are used at runtime.
The pinned upstream abi3 wheel works with the bundled CPython 3.12 x64.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent.parent
WHEEL = "firecrawl_anydoc-0.1.2-cp310-abi3-win_amd64.whl"
URL = "https://files.pythonhosted.org/packages/df/55/7b634a1c7963b1cbb45214e2bcf70cc3b7885710826f54b8d4bd149461ed/" + WHEEL
SHA256 = "dcb20ff01a7874acd3397903271da64e0cc966d2a516afff54527df8dc58ba30"


def stage(python_dir: Path, parsers_dir: Path, wheel: Path) -> dict:
    python_dir = python_dir.resolve()
    if not (python_dir / "python.exe").is_file():
        raise ValueError("Windows embedded Python must be staged first")
    if not wheel.is_file():
        wheel.parent.mkdir(parents=True, exist_ok=True)
        with urllib.request.urlopen(URL, timeout=60) as response:
            data = response.read()
        if hashlib.sha256(data).hexdigest() != SHA256:
            raise ValueError("Downloaded anydoc wheel SHA256 mismatch")
        wheel.write_bytes(data)
    if hashlib.sha256(wheel.read_bytes()).hexdigest() != SHA256:
        raise ValueError("Cached anydoc wheel SHA256 mismatch")
    # Extract beside python.exe: embedded Python's pinned ._pth already includes
    # this directory. Leave its isolation and disabled site imports intact.
    with zipfile.ZipFile(wheel) as archive:
        for name in archive.namelist():
            if not (python_dir / name).resolve().is_relative_to(python_dir):
                raise ValueError("Wheel member escapes embedded Python directory")
        archive.extractall(python_dir)
    wrapper = python_dir / "anydoc_extract"
    wrapper.mkdir(exist_ok=True)
    source = ROOT / "avrag-rs/scripts/anydoc-extract/src/anydoc_extract"
    for name in ("__init__.py", "main.py"):
        shutil.copyfile(source / name, wrapper / name)
    parsers_dir.mkdir(parents=True, exist_ok=True)
    command = ROOT / "desktop/runtime/parsers/anydoc-extract.cmd"
    target = parsers_dir / command.name
    if command.resolve() != target.resolve():
        shutil.copyfile(command, target)
    manifest = {
        "package": "firecrawl-anydoc", "version": "0.1.2",
        "wheel": WHEEL, "sha256": SHA256, "source": URL,
        "wrapper_sha256": hashlib.sha256((source / "main.py").read_bytes()).hexdigest(),
        "command_sha256": hashlib.sha256(command.read_bytes()).hexdigest(),
    }
    (parsers_dir / "anydoc-package.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return manifest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--python-dir", type=Path, required=True)
    parser.add_argument("--parsers-dir", type=Path, required=True)
    parser.add_argument("--wheel", type=Path, default=ROOT / "desktop/runtime/vendor" / WHEEL)
    args = parser.parse_args()
    print(json.dumps(stage(args.python_dir, args.parsers_dir, args.wheel), indent=2))
