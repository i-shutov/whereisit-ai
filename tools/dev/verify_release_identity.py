#!/usr/bin/env python3
"""Fail-closed, read-only release identity acceptance. No user files accessed."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import plistlib
import re
import runpy
import tomllib

ROOT = Path(__file__).resolve().parents[2]
version = (ROOT / "VERSION").read_text(encoding="utf-8").strip()
match = re.fullmatch(r"(\d+\.\d+\.\d+)(?:-rc\.(\d+))?", version)
if match is None:
    raise SystemExit("Invalid release version")
numeric, rc = match.groups()
expected_build = str(10000 + int(rc)) if rc is not None else "10099"

manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
assert manifest["workspace"]["package"]["version"] == version, "Workspace version mismatch"
internal = {"atlas-core", "atlas-service", "atlas-cli", "atlas-ffi", "atlas-windows", "ctf-format"}
for path in [*sorted((ROOT / "crates").glob("*/Cargo.toml")), ROOT / "apps/windows/Cargo.toml"]:
    meta = tomllib.loads(path.read_text(encoding="utf-8"))
    package = meta["package"]
    assert package["name"] in internal, ("Unexpected crate package", path.name)
    spec = package["version"]
    if isinstance(spec, dict):
        assert spec.get("workspace") is True, ("Crate is not workspace-versioned", path)
    else:
        assert spec == version, ("Crate version mismatch", package["name"])
raw_lock = (ROOT / "Cargo.lock").read_bytes()
lock_packages = tomllib.loads(raw_lock.decode("utf-8"))["package"]
actual = {p["name"]: p["version"] for p in lock_packages if p["name"] in internal}
assert len(actual) == len(internal), ("Missing internal lock package", actual)
assert all(v == version for v in actual.values()), ("Locked local crate versions mismatch", actual)
digest = hashlib.sha256(raw_lock).hexdigest()
notices = (ROOT / "THIRD_PARTY_NOTICES.md").read_text(encoding="utf-8")
assert ("SHA-256 Cargo.lock: `" + digest + "`") in notices, "Locked notices digest mismatch"

with (ROOT / "apps/macos/Info.plist").open("rb") as handle:
    plist = plistlib.load(handle)
assert plist["CFBundleShortVersionString"] == numeric, "macOS numeric version mismatch"
assert plist["CFBundleVersion"] == expected_build, "macOS build number not monotone with RC"
assert plist["WhereIsItReleaseVersion"] == version, "macOS release identity mismatch"
assert plist["CFBundleGetInfoString"] == "WhereIsIt AI " + version, "macOS bundle info mismatch"
iss = (ROOT / "installers/windows/WhereIsItAI.iss").read_text(encoding="utf-8")
assert f'#define AppVersion "{version}"' in iss, "Inno Setup fallback version mismatch"
assert f"VersionInfoVersion={numeric}.0" in iss, "Inno numeric version mismatch"

notes = ROOT / "docs/releases" / f"v{version}.md"
assert notes.is_file(), "Missing exact versioned release notes"
acceptance = json.loads((ROOT / "release/ACCEPTANCE.json").read_text(encoding="utf-8"))
assert acceptance["version"] == version, "Acceptance manifest version mismatch"
assert acceptance["personal_catalogs_in_release"] is False, "Private catalog inclusion forbidden"
if rc is not None:
    assert acceptance["channel"] == "prerelease", "RC must be prerelease"
    assert acceptance["stable_ready"] is False, "RC cannot masquerade as stable"
expected_jobs = {"test", "windows", "macos (macos-15)", "macos (macos-15-intel)"}
assert set(acceptance["publication_requires_successful_jobs"]) == expected_jobs, "Incomplete native matrix"
# Mandatory native CI gate: historic owner-data digests must not enter a release.
# This is separate from public executable/lockfile provenance digests.
runpy.run_path(str(ROOT / "tools/qa/check_public_evidence.py"), run_name="__main__")
print(json.dumps({
    "version": version,
    "macos_bundle_build": expected_build,
    "internal_crates": len(actual),
    "cargo_lock_sha256": digest,
    "release_notes_present": True,
    "private_content_included": False,
    "stable_ready": acceptance["stable_ready"],
}, sort_keys=True))
