#!/usr/bin/env python3
"""Prepare only known installer files from one successful native CI run."""
from pathlib import Path
import hashlib
import json
import os
import re
import shutil

root = Path(__file__).resolve().parents[1]
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

wanted = [f"WhereIsItAI-{version}-windows-x64-setup.exe"]
for arch in ("arm64", "x86_64"):
    for suffix in ("pkg", "dmg"):
        wanted.append(f"WhereIsItAI-{version}-macOS-{arch}.{suffix}")
out = Path("release-assets")
out.mkdir(exist_ok=False)
for name in wanted:
    matches = list(Path("incoming").rglob(name))
    if len(matches) != 1 or not matches[0].is_file() or matches[0].is_symlink():
        raise SystemExit("Missing or ambiguous installer: " + name)
    shutil.copyfile(matches[0], out / name)
provenance = {
    "version": version, "source_commit": sha, "ci_run_id": int(run_id),
    "repository": os.environ["GITHUB_REPOSITORY"],
    "ci_url": f'https://github.com/{os.environ["GITHUB_REPOSITORY"]}/actions/runs/{run_id}',
    "required_jobs": {job["name"]: job["conclusion"] for job in jobs},
    "schema_version": 3,
    "signing": acceptance["signing"],
    "stable_ready": acceptance["stable_ready"],
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
