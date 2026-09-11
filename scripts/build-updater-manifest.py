#!/usr/bin/env python3
"""Assemble latest.json once after all Release matrix jobs finish.

tauri-action's uploadUpdaterJson races when macOS Apple Silicon, macOS Intel,
and Windows upload the same asset in parallel — the last writer wins and
darwin-* keys disappear. Mac clients then fail check-update.

This script reads the already-uploaded .sig + bundle assets and writes one
manifest. Requires: gh, GH_TOKEN, GITHUB_REPOSITORY.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path


REQUIRED = ("darwin-aarch64", "darwin-x86_64", "windows-x86_64")


def run_gh(*args: str, raw: bool = False) -> str | bytes:
    cmd = ["gh", *args]
    if raw:
        return subprocess.check_output(cmd)
    return subprocess.check_output(cmd, text=True)


def norm(name: str) -> str:
    return name.replace(" ", ".").lower()


def asset_api_url(repo: str, asset_id: int) -> str:
    return f"https://api.github.com/repos/{repo}/releases/assets/{asset_id}"


def classify_sig(name: str) -> list[str]:
    n = norm(name)
    if not n.endswith(".sig"):
        return []
    if n.endswith(".exe.sig"):
        return ["windows-x86_64", "windows-x86_64-nsis"]
    if n.endswith(".app.tar.gz.sig"):
        if "aarch64" in n:
            return ["darwin-aarch64", "darwin-aarch64-app"]
        if "x86_64" in n or "_x64." in n or n.endswith("_x64.app.tar.gz.sig"):
            return ["darwin-x86_64", "darwin-x86_64-app"]
    return []


def companion_name(sig_name: str) -> str:
    if sig_name.endswith(".sig"):
        return sig_name[: -len(".sig")]
    return sig_name


def find_companion(assets: list[dict], sig_name: str) -> dict | None:
    want = norm(companion_name(sig_name))
    for asset in assets:
        candidates = [asset.get("name") or "", asset.get("label") or ""]
        if any(norm(c) == want for c in candidates if c):
            return asset
    return None


def download_sig(repo: str, asset_id: int) -> str:
    data = run_gh(
        "api",
        "-H",
        "Accept: application/octet-stream",
        f"repos/{repo}/releases/assets/{asset_id}",
        raw=True,
    )
    text = data.decode("utf-8")
    if not text.strip():
        raise SystemExit(f"empty signature asset {asset_id}")
    return text


def self_test() -> None:
    assert classify_sig("QQ.Farm_0.3.5_x64-setup.exe.sig") == [
        "windows-x86_64",
        "windows-x86_64-nsis",
    ]
    assert classify_sig("QQ Farm_aarch64.app.tar.gz.sig") == [
        "darwin-aarch64",
        "darwin-aarch64-app",
    ]
    assert classify_sig("QQ.Farm_x64.app.tar.gz.sig") == [
        "darwin-x86_64",
        "darwin-x86_64-app",
    ]
    assert classify_sig("QQ.Farm_0.3.5_aarch64.dmg") == []
    assets = [
        {"name": "QQ.Farm_aarch64.app.tar.gz", "label": "QQ Farm_aarch64.app.tar.gz"},
        {"name": "QQ.Farm_aarch64.app.tar.gz.sig", "label": "QQ Farm_aarch64.app.tar.gz.sig"},
    ]
    found = find_companion(assets, "QQ Farm_aarch64.app.tar.gz.sig")
    assert found is not None
    assert found["name"] == "QQ.Farm_aarch64.app.tar.gz"


def main() -> int:
    if len(sys.argv) == 2 and sys.argv[1] == "--self-test":
        self_test()
        print("ok")
        return 0
    if len(sys.argv) != 2:
        print("usage: build-updater-manifest.py <tag>", file=sys.stderr)
        return 2
    tag = sys.argv[1]
    repo = os.environ.get("GITHUB_REPOSITORY")
    if not repo:
        raise SystemExit("GITHUB_REPOSITORY is required")

    release = json.loads(run_gh("api", f"repos/{repo}/releases/tags/{tag}"))
    assets = release.get("assets") or []
    version = tag[1:] if tag.startswith("v") else tag
    notes = (release.get("body") or "See the assets to download and install this version.").strip()
    platforms: dict[str, dict[str, str]] = {}

    for asset in assets:
        name = asset.get("name") or ""
        keys = classify_sig(name)
        if not keys:
            continue
        companion = find_companion(assets, name)
        if companion is None:
            raise SystemExit(f"no updater bundle for signature {name}")
        sig_text = download_sig(repo, int(asset["id"]))
        entry = {
            "signature": sig_text,
            "url": asset_api_url(repo, int(companion["id"])),
        }
        for key in keys:
            platforms[key] = entry

    missing = [key for key in REQUIRED if key not in platforms]
    if missing:
        have = ", ".join(sorted(platforms)) or "(none)"
        raise SystemExit(f"latest.json missing {missing}; have {have}")

    manifest = {
        "version": version,
        "notes": notes,
        "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.000Z"),
        "platforms": platforms,
    }
    out = Path("latest.json")
    out.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(out.read_text(encoding="utf-8"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
