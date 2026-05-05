#!/usr/bin/env python3
import sys
import subprocess
import platform
import shutil
from pathlib import Path

PROJECT_ROOT = Path(__file__).parent
FRONTEND_DIR = PROJECT_ROOT / "frontend"
BACKEND_DIR = PROJECT_ROOT / "backend"
OUTPUT_DIR = PROJECT_ROOT / "output"


def run(cmd, cwd=None):
    print(f">>> {cmd}")
    subprocess.run(cmd, shell=True, cwd=cwd, check=True)


def main():
    system = platform.system()

    if system not in ("Linux", "Windows"):
        print(f"Unsupported platform: {system}")
        sys.exit(1)

    print("=== Step 1/3: Building frontend ===")
    run("npm run build", cwd=FRONTEND_DIR)

    print("=== Step 2/3: Building backend ===")
    if system == "Linux":
        target = "x86_64-unknown-linux-musl"
        run(f"cargo build --target {target} --release", cwd=BACKEND_DIR)
        binary_name = "backend"
        binary_path = BACKEND_DIR / "target" / target / "release" / binary_name
    else:
        run("cargo build --release", cwd=BACKEND_DIR)
        binary_name = "backend.exe"
        binary_path = BACKEND_DIR / "target" / "release" / binary_name

    if not binary_path.exists():
        print(f"Error: Binary not found at {binary_path}")
        sys.exit(1)

    print("=== Step 3/3: Copying output ===")
    OUTPUT_DIR.mkdir(exist_ok=True)
    output_path = OUTPUT_DIR / binary_name
    shutil.copy2(binary_path, output_path)

    size_mb = output_path.stat().st_size / (1024 * 1024)
    print(f"=== Done: {output_path} ({size_mb:.1f} MB) ===")


if __name__ == "__main__":
    main()
