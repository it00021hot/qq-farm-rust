#!/usr/bin/env bash
set -euo pipefail

VER="${1:?usage: sync-app-version.sh <version>}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

python3 - "$VER" "$ROOT/Cargo.toml" "$ROOT/crates/qq-farm-desktop/tauri.conf.json" "$ROOT/desktop-ui/package.json" <<'PY'
import json
import pathlib
import re
import sys

ver, cargo_toml, tauri_conf, package_json = sys.argv[1:]

cargo_path = pathlib.Path(cargo_toml)
text = cargo_path.read_text(encoding="utf-8")
match = re.search(r'^(version = ")([^"]+)(")', text, flags=re.MULTILINE)
if match is None:
    raise SystemExit(f"failed to find workspace version in {cargo_path}")
if match.group(2) != ver:
    updated = text[: match.start(2)] + ver + text[match.end(2) :]
    cargo_path.write_text(updated, encoding="utf-8")

for path in (tauri_conf, package_json):
    p = pathlib.Path(path)
    data = json.loads(p.read_text(encoding="utf-8"))
    data["version"] = ver
    p.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

print(f"synced app version to {ver}")
PY
