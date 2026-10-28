#!/usr/bin/env python3
"""Apply checked release metadata to source; no dependency upgrades or private data."""
from pathlib import Path
import hashlib
import plistlib
import re
import tomllib

root = Path(__file__).resolve().parents[2]
version = (root / "VERSION").read_text().strip()
if not re.fullmatch(r"\d+\.\d+\.\d+(?:-rc\.\d+)?", version):
    raise ValueError("Invalid release version")
numeric = version.split("-")[0]
workspace_names = {"atlas-core", "atlas-service", "atlas-cli", "atlas-ffi", "atlas-windows", "ctf-format"}
for path in [root / "Cargo.toml", *root.glob("crates/*/Cargo.toml"), root / "apps/windows/Cargo.toml"]:
    text = path.read_text(encoding="utf-8")
    parsed = tomllib.loads(text)
    if path == root / "Cargo.toml" or isinstance(parsed.get("package", {}).get("version"), str):
        text, count = re.subn(r'^version\s*=\s*"[^"]+"', f'version = "{version}"', text, count=1, flags=re.M)
        if count != 1:
            raise ValueError(f"Missing package version: {path.name}")
        path.write_text(text, encoding="utf-8")
for path in [root / "Cargo.lock", root / "crates/atlas-core/Cargo.lock"]:
    if not path.exists():
        continue
    text = path.read_text(encoding="utf-8")
    for name in workspace_names:
        pattern = rf'(\[\[package\]\]\nname = "{re.escape(name)}"\nversion = ")[^"]+("\n)'
        text = re.sub(pattern, lambda m: m[1] + version + m[2], text)
    path.write_text(text, encoding="utf-8")
path = root / "apps/macos/Info.plist"
with path.open("rb") as handle:
    info = plistlib.load(handle)
info["CFBundleShortVersionString"] = numeric
info["CFBundleVersion"] = "10001" if "-rc." in version else "10099"
info["WhereIsItReleaseVersion"] = version
info["CFBundleGetInfoString"] = "WhereIsIt AI " + version
with path.open("wb") as handle:
    plistlib.dump(info, handle, sort_keys=False)
path = root / "THIRD_PARTY_NOTICES.md"
text = path.read_text(encoding="utf-8")
text = re.sub(r"SHA-256 Cargo.lock: `[0-9a-f]{64}`", "SHA-256 Cargo.lock: `" + hashlib.sha256((root / "Cargo.lock").read_bytes()).hexdigest() + "`", text)
path.write_text(text, encoding="utf-8")
path = root / "installers/windows/WhereIsItAI.iss"
text = path.read_text(encoding="utf-8")
text = re.sub(r'#define AppVersion "[^"]+"', f'#define AppVersion "{version}"', text)
if "VersionInfoVersion=" not in text:
    text = text.replace("AppVersion={#AppVersion}", "AppVersion={#AppVersion}\nVersionInfoVersion=" + numeric + ".0")
path.write_text(text, encoding="utf-8")
print("Release identity:", version)
