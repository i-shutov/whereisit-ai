#!/usr/bin/env python3
"""Fail-closed approval gate for publishing a Windows-only RC candidate."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any


RC_VERSION = re.compile(r"\d+\.\d+\.\d+-rc\.\d+\Z")
PRODUCT_SCHEMA = "whereisit-egui-product-candidate-v1"


def policy_error(version: str, acceptance: Any, product: Any) -> str | None:
    """Return a refusal reason unless both reviewed manifests approve this RC."""
    if not isinstance(version, str) or not RC_VERSION.fullmatch(version):
        return "VERSION is not an exact release-candidate version"
    if not isinstance(acceptance, dict) or not isinstance(product, dict):
        return "Acceptance or product manifest is not an object"
    if acceptance.get("version") != version:
        return "VERSION does not match release/ACCEPTANCE.json"
    if acceptance.get("channel") != "prerelease":
        return "Acceptance channel must be prerelease"
    if acceptance.get("stable_ready") is not False:
        return "Prerelease publication requires stable_ready=false"
    if acceptance.get("candidate_publication_approved") is not True:
        return "Candidate publication requires explicit boolean approval"
    scope = acceptance.get("release_scope")
    if (not isinstance(scope, dict) or scope.get("target_version") != version
            or scope.get("ship_platforms") != ["windows-x64"]):
        return "Candidate release scope must ship Windows x64 only"

    if product.get("schema") != PRODUCT_SCHEMA:
        return "Product manifest schema mismatch"
    if (product.get("version") != version
            or product.get("channel") != "prerelease"
            or product.get("stable_ready") is not False
            or product.get("candidate_publication_approved") is not True
            or product.get("platform") != "windows"
            or product.get("architecture") != "x64"
            or product.get("release_scope") != scope):
        return "Windows product manifest does not exactly match approved RC scope"
    return None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--candidate-dir", type=Path, required=True)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()

    try:
        version = (args.repo / "VERSION").read_text(encoding="utf-8-sig").strip()
        acceptance = json.loads(
            (args.repo / "release/ACCEPTANCE.json").read_text(encoding="utf-8-sig")
        )
        manifests = []
        for path in args.candidate_dir.rglob("provenance.json"):
            if path.is_symlink() or not path.is_file():
                continue
            product = json.loads(path.read_text(encoding="utf-8-sig"))
            if isinstance(product, dict) and product.get("schema") == PRODUCT_SCHEMA:
                manifests.append(product)
        if len(manifests) != 1:
            raise ValueError("Expected exactly one Windows product candidate manifest")
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"Candidate publication denied: {error}", file=sys.stderr)
        return 1

    reason = policy_error(version, acceptance, manifests[0])
    if reason is not None:
        print(f"Candidate publication denied: {reason}", file=sys.stderr)
        return 1
    print(f"Explicit publication approval verified for Windows RC {version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
