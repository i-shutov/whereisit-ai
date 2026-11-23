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


def prepare_stable_assets() -> None:
    """Prepare the deliberately smaller Windows-only stable payload."""
    if len(sys.argv) != 2 or sys.argv[1] != "--stable":
        raise SystemExit("Invalid stable preparer invocation")
    sha = os.environ.get("SOURCE_SHA", "")
    run_id = os.environ.get("CI_RUN_ID", "")
    repository = os.environ.get("GITHUB_REPOSITORY", "")
    if not re.fullmatch(r"[0-9a-f]{40}", sha) or not run_id.isdigit():
        raise SystemExit("Invalid source/run identity")
    if not repository:
        raise SystemExit("Missing repository identity")

    version = (root / "VERSION").read_text(encoding="utf-8-sig").strip()
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise SystemExit("Stable publisher requires a numeric stable VERSION")
    acceptance_path = root / "release/ACCEPTANCE.json"
    acceptance = json.loads(acceptance_path.read_text(encoding="utf-8-sig"))
    if (acceptance.get("version") != version or acceptance.get("channel") != "stable"
            or acceptance.get("stable_ready") is not True
            or acceptance.get("personal_catalogs_in_release") is not False
            or acceptance.get("known_open_gates") != []
            or acceptance.get("publication_requires_successful_jobs") != ["test", "windows"]):
        raise SystemExit("Stable acceptance manifest is not ready")
    acceptance_scope = acceptance.get("release_scope")
    if (not isinstance(acceptance_scope, dict)
            or acceptance_scope.get("target_version") != version
            or acceptance_scope.get("ship_platforms") != ["windows-x64"]
            or acceptance_scope.get("unsigned_windows_stable_approved") is not True
            or acceptance_scope.get("usb_binding_enabled") is not False
            or not isinstance(acceptance_scope.get("deferred_features"), list)
            or not isinstance(acceptance_scope.get("deferred_product_platforms", acceptance_scope.get("deferred_platforms")), list)):
        raise SystemExit("Stable acceptance release_scope mismatch")

    run = json.loads(Path("verified-run.json").read_text(encoding="utf-8-sig"))
    run_status, run_conclusion = run.get("status"), run.get("conclusion")
    eligible_run_state = ((run_status in ("queued", "in_progress", "waiting", "pending", "requested")
                           and run_conclusion is None)
                          or (run_status == "completed"
                              and run_conclusion in ("success", "failure", "cancelled")))
    if (run.get("head_sha") != sha or run.get("head_branch") != "main"
            or str(run.get("id")) != run_id
            or not eligible_run_state
            or run.get("event") not in ("push", "workflow_dispatch")
            or run.get("path") != ".github/workflows/build.yml"
            or not isinstance(run.get("head_repository"), dict)
            or run["head_repository"].get("full_name") != repository):
        raise SystemExit("Build is not an eligible trusted exact-source main run")
    jobs = json.loads(Path("verified-jobs.json").read_text(encoding="utf-8-sig")).get("jobs", [])
    for required in ("test", "windows"):
        matches = [job for job in jobs if job.get("name") == required]
        if (len(matches) != 1 or matches[0].get("conclusion") != "success"
                or matches[0].get("status") != "completed"):
            raise SystemExit("Required Windows stable CI job did not pass: " + required)

    installer_name = f"WhereIsItAI-egui-{version}-windows-x64-setup.exe"
    matches = list(Path("incoming").rglob(installer_name))
    if len(matches) != 1 or not matches[0].is_file() or matches[0].is_symlink():
        raise SystemExit("Missing or ambiguous installer: " + installer_name)
    source = matches[0]
    manifest_path = source.parent / "provenance.json"
    checksum_path = source.parent / "SHA256SUMS.txt"
    if any(not path.is_file() or path.is_symlink() for path in (manifest_path, checksum_path)):
        raise SystemExit("Missing or unsafe Windows product provenance/checksum")
    product = json.loads(manifest_path.read_text(encoding="utf-8-sig"))
    if (product.get("schema") != "whereisit-egui-product-candidate-v1"
            or product.get("channel") != "stable"
            or product.get("target_ui") != "Rust/egui"
            or product.get("platform") != "windows"
            or product.get("architecture") != "x64"
            or product.get("version") != version
            or product.get("commit") != sha
            or str(product.get("run_id")) != run_id
            or product.get("stable_ready") is not True
            or product.get("personal_catalogs_included") is not False
            or product.get("signing") != "unsigned"
            or product.get("release_scope") != acceptance_scope):
        raise SystemExit("Stable product manifest identity, scope, or unsigned policy mismatch")

    checksum_lines = checksum_path.read_text(encoding="utf-8-sig").splitlines()
    expected_hash = None
    for line in checksum_lines:
        pieces = line.split(None, 1)
        if len(pieces) == 2 and pieces[1].lstrip("* ") == source.name:
            expected_hash = pieces[0]
            break
    if expected_hash is None or not re.fullmatch(r"[0-9a-fA-F]{64}", expected_hash):
        raise SystemExit("Product checksum manifest omits: " + installer_name)
    manifest_hash = product.get("installer_sha256")
    if not isinstance(manifest_hash, str) or manifest_hash.lower() != expected_hash.lower():
        raise SystemExit("Product manifest installer checksum mismatch: " + installer_name)
    digest = hashlib.sha256()
    with source.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    if digest.hexdigest().lower() != expected_hash.lower():
        raise SystemExit("Product checksum mismatch: " + installer_name)

    out = Path("release-assets")
    out.mkdir(exist_ok=False)
    shutil.copyfile(source, out / installer_name)
    provenance = {
        "version": version, "source_commit": sha, "ci_run_id": int(run_id),
        "repository": repository,
        "ci_url": f"https://github.com/{repository}/actions/runs/{run_id}",
        "required_jobs": {name: "success" for name in ("test", "windows")},
        "schema_version": 1, "target_ui": "Rust/egui", "channel": "stable",
        "platform": "windows", "architecture": "x64", "signing": "unsigned",
        "unsigned_windows_stable_approved": True, "stable_ready": True,
        "release_scope": acceptance_scope, "personal_catalogs_included": False,
    }
    (out / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")
    shutil.copyfile(acceptance_path, out / "ACCEPTANCE.json")
    lines = []
    for path in sorted(out.iterdir()):
        digest = hashlib.sha256()
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
        lines.append(f"{digest.hexdigest()}  {path.name}\n")
    (out / "SHA256SUMS.txt").write_text("".join(lines), encoding="ascii")
    print(json.dumps(provenance, indent=2))


root = Path(__file__).resolve().parents[1]
subprocess.run([sys.executable, str(root / 'tools/qa/check_public_evidence.py')],
               cwd=root, check=True)
if len(sys.argv) > 1 and sys.argv[1] == "--stable":
    prepare_stable_assets()
    raise SystemExit(0)
# The main native CI and offline kit are not substitutes for a release-time gate.
# The checker emits only filenames on failure, never private source identifiers.
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
    # Windows PowerShell 5.1 emits UTF-8 with BOM; macOS emits BOM-less UTF-8.
    product = json.loads(manifest_path.read_text(encoding="utf-8-sig"))
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
