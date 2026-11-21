#!/usr/bin/env python3
"""Prepare only known installer files from one successful native CI run."""
from pathlib import Path
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
# The main native CI and offline kit are not substitutes for a release-time gate.
# The checker emits only filenames on failure, never private source identifiers.
subprocess.run([sys.executable, str(root / 'tools/qa/check_public_evidence.py')],
               cwd=root, check=True)
version = (root / "VERSION").read_text().strip()
if not re.fullmatch(r"\d+\.\d+\.\d+-rc\.\d+", version):
    raise SystemExit("This publisher is restricted to release candidates")
sha = os.environ["SOURCE_SHA"]
run_id = os.environ["CI_RUN_ID"]
if not re.fullmatch(r"[0-9a-f]{40}", sha) or not run_id.isdigit():
    raise SystemExit("Invalid source/run identity")
acceptance = json.loads((root / "release/ACCEPTANCE.json").read_text())
if acceptance["version"] != version or acceptance["channel"] != "prerelease":
    raise SystemExit("Acceptance manifest version/channel mismatch")
if acceptance.get("stable_ready") is not False:
    raise SystemExit("Prerelease publication requires stable_ready=false")
run = json.loads(Path("verified-run.json").read_text())
if run["head_sha"] != sha or run["head_branch"] != "main" or run["conclusion"] != "success":
    raise SystemExit("Build is not a successful exact-source main run")
if run["head_repository"]["full_name"] != os.environ["GITHUB_REPOSITORY"]:
    raise SystemExit("Build came from a different repository")
jobs = json.loads(Path("verified-jobs.json").read_text())["jobs"]
for required in acceptance["publication_requires_successful_jobs"]:
    matches = [job for job in jobs if job["name"] == required]
    if len(matches) != 1 or matches[0]["conclusion"] != "success":
        raise SystemExit("Required platform job did not pass: " + required)

expected = {f"WhereIsItAI-egui-{version}-windows-x64-setup.exe": "x64"}
for arch in ("arm64", "x86_64"):
    for suffix in ("pkg", "dmg"):
        expected[f"WhereIsItAI-egui-{version}-macOS-{arch}.{suffix}"] = arch
candidate_signing = {}
selected = []
for name, architecture in expected.items():
    matches = list(Path("incoming").rglob(name))
    if len(matches) != 1 or not matches[0].is_file() or matches[0].is_symlink():
        raise SystemExit("Missing or ambiguous installer: " + name)
    source = matches[0]
    manifest_path = source.parent / "provenance.json"
    if not manifest_path.is_file():
        # macOS product artifacts keep the platform manifest beside the files
        # in the uploaded artifact root, with the architecture in its filename.
        candidates = list(source.parent.glob("provenance-egui-*.json"))
        candidates = [p for p in candidates if architecture in p.name]
        if len(candidates) == 1:
            manifest_path = candidates[0]
    if not manifest_path.is_file() or manifest_path.is_symlink():
        raise SystemExit("Missing product provenance manifest for: " + name)
    product = json.loads(manifest_path.read_text(encoding="utf-8"))
    if (product.get("schema") != "whereisit-egui-product-candidate-v1"
            or product.get("channel") != "prerelease"
            or product.get("target_ui") != "Rust/egui"
            or product.get("platform") != ("windows" if architecture == "x64" else "macos")
            or product.get("version") != version
            or product.get("commit") != sha
            or str(product.get("run_id")) != run_id
            or product.get("architecture") != architecture
            or product.get("stable_ready") is not False
            or product.get("personal_catalogs_included") is not False):
        raise SystemExit("Egui product manifest identity or prerelease contract mismatch: " + name)
    signing = product.get("signing")
    # This publisher's title and channel explicitly describe an unsigned RC.
    # A future signed/notarized publication must update this reviewed contract.
    allowed_signing = {"unsigned"} if architecture == "x64" else {
        "ad-hoc app; unsigned pkg/dmg; not notarized",
    }
    if signing not in allowed_signing:
        raise SystemExit("Unrecognized product signing state: " + name)
    candidate_signing[architecture] = signing
    checksum_name = "SHA256SUMS.txt" if architecture == "x64" else f"SHA256-egui-{architecture}.txt"
    checksum_path = source.parent / checksum_name
    if not checksum_path.is_file() or checksum_path.is_symlink():
        raise SystemExit("Missing product checksum manifest for: " + name)
    checksum_lines = checksum_path.read_text(encoding="ascii").splitlines()
    expected_hash = None
    for line in checksum_lines:
        pieces = line.split(None, 1)
        if len(pieces) == 2 and pieces[1].lstrip("* ") == source.name:
            expected_hash = pieces[0]
            break
    if expected_hash is None:
        raise SystemExit("Product checksum manifest omits: " + name)
    digest_builder = hashlib.sha256()
    with source.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest_builder.update(chunk)
    if digest_builder.hexdigest().lower() != expected_hash.lower():
        raise SystemExit("Product checksum mismatch: " + name)
    selected.append((source, name))
out = Path("release-assets")
out.mkdir(exist_ok=False)
for source, name in selected:
    shutil.copyfile(source, out / name)
provenance = {
    "version": version, "source_commit": sha, "ci_run_id": int(run_id),
    "repository": os.environ["GITHUB_REPOSITORY"],
    "ci_url": f'https://github.com/{os.environ["GITHUB_REPOSITORY"]}/actions/runs/{run_id}',
    "required_jobs": {job["name"]: job["conclusion"] for job in jobs},
    "schema_version": 3,
    "target_ui": "Rust/egui",
    "channel": "prerelease",
    "signing": {
        "windows_x64": candidate_signing["x64"],
        "macos_arm64": candidate_signing["arm64"],
        "macos_x86_64": candidate_signing["x86_64"],
    },
    "stable_ready": False,
    "known_open_gates": acceptance["known_open_gates"],
    "personal_catalogs_included": False,
}
(out / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")
shutil.copyfile(root / "release/ACCEPTANCE.json", out / "ACCEPTANCE.json")
lines = []
for path in sorted(out.iterdir()):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    lines.append(f"{digest.hexdigest()}  {path.name}\n")
(out / "SHA256SUMS.txt").write_text("".join(lines), encoding="ascii")
print(json.dumps(provenance, indent=2))
