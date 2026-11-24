"""Collect notices for the locked Windows runtime dependency graph."""
import os
import subprocess
import sys
import tomllib
from pathlib import Path


def main(destination):
    graph = subprocess.check_output([
        "cargo", "tree", "--locked", "-p", "indexray", "--target", "x86_64-pc-windows-msvc",
        "--edges", "normal", "--prefix", "none", "--format", "{p}",
    ], text=True, encoding="utf-8")
    registry = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")) / "registry/src"
    with destination.open("w", encoding="utf-8") as output:
        output.write("IndexRay Windows x64 — third-party software notices\n")
        output.write("Corresponding versions and registry sources are pinned in Cargo.lock.\n")
        for item in sorted(set(line.removesuffix(" (*)") for line in graph.splitlines())):
            name, version, *location = item.split(" ", 2)
            if name in {"indexray", "dowse"}:
                continue
            version = version.removeprefix("v")
            package, = sorted(registry.glob(f"*/{name}-{version}"))
            manifest = tomllib.loads((package / "Cargo.toml").read_text(encoding="utf-8"))["package"]
            output.write(f"\n{'=' * 72}\n{name} {version}\n")
            output.write(f"License: {manifest.get('license', 'see license text')}\n")
            output.write(f"Source: https://crates.io/crates/{name}/{version}\n")
            files = {file for file in package.iterdir() if file.is_file() and
                     file.name.upper().startswith(("LICENSE", "COPYING", "NOTICE", "COPYRIGHT", "UNLICENSE"))}
            if file := manifest.get("license-file"):
                files.add(package / file)
            for file in sorted(files):
                output.write(f"\n--- {file.name} ---\n")
                output.write(file.read_text(encoding="utf-8", errors="replace"))
                output.write("\n")
            if not files:
                for name in ("README.md", "README", "README.txt"):
                    if (file := package / name).is_file():
                        output.write(f"\n--- {name} ---\n" + file.read_text(encoding="utf-8", errors="replace") + "\n")
                        break


if __name__ == "__main__":
    main(Path(sys.argv[1]))
