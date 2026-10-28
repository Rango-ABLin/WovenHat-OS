"""Prepare pinned WovenWeb sources without resetting existing files."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--build", action="store_true")
    args = parser.parse_args()
    destination = args.destination.resolve()
    if destination.exists():
        parser.error("destination must not exist; existing files are never reset")
    upstream = json.loads((HERE / "upstream.json").read_text())
    subprocess.run(["git", "init", str(destination)], check=True)
    git = ["git", "-C", str(destination)]
    subprocess.run([*git, "remote", "add", "origin", upstream["repository"]], check=True)
    subprocess.run([*git, "fetch", "--depth", "1", "origin", upstream["revision"]], check=True)
    subprocess.run([*git, "checkout", "--detach", "FETCH_HEAD"], check=True)
    revision = subprocess.check_output([*git, "rev-parse", "HEAD"], text=True).strip()
    if revision != upstream["revision"]:
        raise RuntimeError("upstream revision mismatch")
    subprocess.run([*git, "apply", "--check", str(HERE / "ladybird.patch")], check=True)
    subprocess.run([*git, "apply", str(HERE / "ladybird.patch")], check=True)
    if args.build:
        subprocess.run([sys.executable, "Meta/ladybird.py", "build"], cwd=destination, check=True)
    print(f"Prepared {destination}. Native WovenHat is not a supported build target.")


if __name__ == "__main__":
    main()
