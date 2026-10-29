#!/usr/bin/env python3
"""Regenerate license inventory from *locked* Cargo registry dependencies.

Used only by the isolated SQLite upgrade preparation workflow. Never touches
user catalogs or personal CTF files. This does not sign a release.
"""
from __future__ import annotations

import hashlib
from pathlib import Path
import re
import shutil
import tomllib

ROOT = Path(__file__).resolve().parents[2]
REGISTRY = Path.home() / ".cargo" / "registry" / "src"
LOCK = ROOT / "Cargo.lock"
NOTICES = ROOT / "THIRD_PARTY_NOTICES.md"
LICENSES = ROOT / "licenses"
SQLITE_VERSION = "3.51.3"
REQUIRED = {"rusqlite": "0.39.0", "libsqlite3-sys": "0.37.0"}
COPY_NAMES = ("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE", "UNLICENSE")


def registry_directory(name: str, version: str) -> Path:
    matches = sorted(REGISTRY.glob(f"*/{name}-{version}"))
    if len(matches) != 1:
        raise RuntimeError(f"Expected one downloaded source for {name} {version}; found {len(matches)}")
    return matches[0]


def check_sqlite_source() -> Path:
    source = registry_directory("libsqlite3-sys", REQUIRED["libsqlite3-sys"])
    amalgamation = source / "sqlite3" / "sqlite3.c"
    if not amalgamation.is_file():
        raise RuntimeError("Bundled SQLite amalgamation is missing")
    with amalgamation.open("rb") as stream:
        chunk = stream.read(200_000)
    marker = f'#define SQLITE_VERSION "{SQLITE_VERSION}"'.encode()
    if marker not in chunk:
        raise RuntimeError("Bundled sqlite3.c is not the reviewed SQLite " + SQLITE_VERSION)
    if b"The author disclaims copyright to this source code." not in chunk:
        raise RuntimeError("SQLite public-domain notice missing from bundled source")
    return amalgamation


def copy_licenses(source: Path, destination: Path) -> list[Path]:
    destination.mkdir(parents=True, exist_ok=True)
    files = []
    for file in sorted(source.iterdir()):
        if not file.is_file() or not file.name.upper().startswith(COPY_NAMES):
            continue
        if file.stat().st_size > 1024 * 1024:
            raise RuntimeError("Unexpectedly large license/notice file: " + file.name)
        target = destination / file.name
        if not target.exists() or target.read_bytes() != file.read_bytes():
            shutil.copyfile(file, target)
        files.append(target)
    return files


def main() -> None:
    raw = LOCK.read_bytes()
    packages = [
        p for p in tomllib.loads(raw.decode("utf-8"))["package"]
        if str(p.get("source", "")).startswith("registry+")
    ]
    for name, version in REQUIRED.items():
        matched = [p["version"] for p in packages if p["name"] == name]
        if matched != [version]:
            raise RuntimeError(f"Expected {name}={version}; locked {matched}")

    sqlite_c = check_sqlite_source()
    original = (LICENSES / "SQLite-3.46.0" / "COPYRIGHT.txt").read_text(
        encoding="utf-8"
    )
    if "SQLite 3.46.0" not in original:
        raise RuntimeError("Legacy SQLite public domain notice has changed")
    new_notice = original.replace("SQLite 3.46.0", "SQLite " + SQLITE_VERSION)
    dest = LICENSES / f"SQLite-{SQLITE_VERSION}" / "COPYRIGHT.txt"
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(new_notice, encoding="utf-8")

    backtick = chr(96)
    lines = [
        "# Уведомления о сторонних компонентах",
        "",
        "Список генерируется из зафиксированных registry-пакетов в Cargo.lock.",
        "Он охватывает зависимости сборки, тестов и всех поддержанных платформ;",
        "не каждая из перечисленных библиотек попадает в каждый бинарный файл.",
        "Лицензионные формулировки получены из опубликованных Cargo.toml;",
        "файлы лицензий/NOTICE скопированы из соответствующих Cargo-пакетов.",
        "При отсутствии отдельного файла в архиве остаётся выражение лицензии",
        "из его манифеста. Наличие файла не заменяет юридического анализа.",
        "",
        f"SHA-256 Cargo.lock: {backtick}{hashlib.sha256(raw).hexdigest()}{backtick}.",
        "",
        "## Встроенный SQLite",
        "",
        f"rusqlite {REQUIRED['rusqlite']} + libsqlite3-sys {REQUIRED['libsqlite3-sys']}",
        f"с включённым bundled используют SQLite {SQLITE_VERSION}.",
        "Проверено по sqlite3.c опубликованного crate, а не по системной SQLite.",
        "В исходнике SQLite авторы отказываются от авторского права.",
        f"Исходный заголовок сохранён: [COPYRIGHT.txt](licenses/SQLite-{SQLITE_VERSION}/COPYRIGHT.txt).",
        "Исправление WAL-reset включено в SQLite 3.51.3 и позднее.",
        "Версия подтверждается также при сборке и тестах.",
        "",
        "## Зафиксированные registry-пакеты",
        "",
        "| Пакет | Версия | Лицензия из Cargo.toml | Файлы |",
        "|---|---|---|---|",
    ]
    for p in sorted(packages, key=lambda x: (x["name"], x["version"])):
        name, version = p["name"], p["version"]
        source = registry_directory(name, version)
        metadata = tomllib.loads((source / "Cargo.toml").read_text(encoding="utf-8"))
        package = metadata.get("package", {})
        license_expression = package.get("license", "")
        if not license_expression:
            raise RuntimeError(f"Published Cargo package lacks SPDX license metadata: {name} {version}")
        files = copy_licenses(source, LICENSES / f"{name}-{version}")
        declared = package.get("license-file")
        if not files and declared:
            file = source / declared
            if not file.is_file() or file.stat().st_size > 1024 * 1024:
                raise RuntimeError(f"Missing declared license file: {name} {version}")
            target = LICENSES / f"{name}-{version}" / file.name
            shutil.copyfile(file, target)
            files = [target]
        links = ", ".join(
            f"[{file.name}]({file.relative_to(ROOT).as_posix()})" for file in files
        ) if files else "Отдельный файл в архиве отсутствует"
        escaped = str(license_expression).replace("|", r"\|")
        lines.append(
            f"| {name} | {version} | {backtick}{escaped}{backtick} | {links} |"
        )
    NOTICES.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(
        f"Verified {len(packages)} registry packages; bundled SQLite {SQLITE_VERSION}; "
        f"lock SHA-256 {hashlib.sha256(raw).hexdigest()}; amalgamation {sqlite_c.name}"
    )


if __name__ == "__main__":
    main()
