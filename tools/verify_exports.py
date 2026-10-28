#!/usr/bin/env python3
"""Compare Atlas CSV/Parquet exports independently (Python 3.10+, optional DuckDB)."""
import argparse
import csv
import datetime
import itertools
import json
from pathlib import Path
import re
import sys

COLUMNS = ["id", "volume_id", "parent_id", "name", "relative_path", "kind", "size", "modified_unix", "description", "tags", "volume_label", "categories", "duplicate_count", "duplicate_group", "modified_local"]
INTEGER_COLUMNS = ["id", "volume_id", "parent_id", "size", "modified_unix", "duplicate_count"]


class ValidationError(Exception):
    pass


def require(condition, message):
    if not condition:
        raise ValidationError(message)


def verify(csv_path, parquet_path):
    try:
        import duckdb
    except ImportError as error:
        raise ValidationError("Parquet comparison requires the optional developer dependency: python -m pip install duckdb==1.4.1") from error
    connection = duckdb.connect(config={"threads": "1", "memory_limit": "128MB"})
    csv.field_size_limit(16 * 1024 * 1024)
    try:
        cursor = connection.execute("SELECT * FROM read_parquet(?)", [str(parquet_path)])
        columns = [column[0] for column in cursor.description]
        types = {column[0]: str(column[1]) for column in cursor.description}
        require(columns == COLUMNS, "Parquet column names/order differ from the 15-column Atlas schema")
        for name in INTEGER_COLUMNS:
            require(types[name] == "BIGINT", "Expected Parquet BIGINT for " + name)
        def parquet_rows():
            while True:
                batch = cursor.fetchmany(2048)
                if not batch:
                    return
                yield from batch
        report = {
            "status": "passed", "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "readers": {"csv": "Python standard library csv", "parquet": "DuckDB " + duckdb.__version__},
            "columns": 15, "rows": 0, "all_values_equal": True, "size_type": types["size"],
            "size_sum_bytes": 0, "size_min_bytes": None, "size_max_bytes": None,
            "rows_over_4_gib": 0, "rows_with_cyrillic_name_or_path": 0,
            "null_modified_dates": 0, "null_parent_ids": 0, "local_date_strings": 0,
            "normalization": "Parquet NULL equals an empty CSV field; all other values compare as exact text. Integer values never pass through floating point.",
        }
        sentinel = object()
        with csv_path.open(encoding="utf-8-sig", newline="") as stream:
            reader = csv.reader(stream)
            require(next(reader, None) == COLUMNS, "CSV column names/order differ from the 15-column Atlas schema")
            for csv_row, parquet_row in itertools.zip_longest(reader, parquet_rows(), fillvalue=sentinel):
                require(csv_row is not sentinel and parquet_row is not sentinel, "CSV and Parquet row counts differ")
                number = report["rows"] + 1
                require(len(csv_row) == len(parquet_row) == 15, "Column count differs at row " + str(number))
                normalized = ["" if value is None else str(value) for value in parquet_row]
                require(csv_row == normalized, "Field values differ at row " + str(number) + " (values intentionally omitted)")
                for name in INTEGER_COLUMNS:
                    value = parquet_row[COLUMNS.index(name)]
                    require(value is None or (type(value) is int and -(2**63) <= value < 2**63), "Invalid signed 64-bit integer at row " + str(number))
                size = parquet_row[6]
                require(type(size) is int and size >= 0, "Invalid file size at row " + str(number))
                require(int(csv_row[6]) == size, "Exact decimal size mismatch at row " + str(number))
                report["rows"] += 1
                report["size_sum_bytes"] += size
                report["size_min_bytes"] = size if report["size_min_bytes"] is None else min(report["size_min_bytes"], size)
                report["size_max_bytes"] = size if report["size_max_bytes"] is None else max(report["size_max_bytes"], size)
                report["rows_over_4_gib"] += int(size > 4 * 1024**3)
                report["rows_with_cyrillic_name_or_path"] += int(any(re.search("[\u0400-\u04ff]", str(parquet_row[i])) for i in (3, 4)))
                report["null_modified_dates"] += int(parquet_row[7] is None)
                report["null_parent_ids"] += int(parquet_row[2] is None)
                local = parquet_row[14]
                if local:
                    require(isinstance(local, str) and re.fullmatch(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}", local), "Invalid local date format at row " + str(number))
                    datetime.datetime.strptime(local, "%Y-%m-%d %H:%M:%S")
                    report["local_date_strings"] += 1
        report["csv_bytes"] = csv_path.stat().st_size
        report["parquet_bytes"] = parquet_path.stat().st_size
        return report
    finally:
        connection.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("csv", type=Path, help="Completed Atlas CSV export")
    parser.add_argument("parquet", type=Path, help="Completed matching Atlas Parquet export")
    parser.add_argument("--output", type=Path, help="Optional new JSON report; otherwise print JSON only")
    args = parser.parse_args()
    if not args.csv.is_file() or not args.parquet.is_file():
        parser.error("Both export files must exist")
    if args.output and (args.output.exists() or args.output.resolve() in {args.csv.resolve(), args.parquet.resolve()}):
        parser.error("--output must be a new file separate from the inputs")
    report = verify(args.csv, args.parquet)
    if args.output:
        with args.output.open("x", encoding="utf-8") as stream:
            json.dump(report, stream, ensure_ascii=False, indent=2)
            stream.write("\n")
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        # Do not print rows or field contents in failure reports.
        message = str(error) if isinstance(error, ValidationError) else "Could not read or validate the exports; no input files were changed"
        print(json.dumps({"status": "failed", "error": type(error).__name__, "message": message}), file=sys.stderr)
        sys.exit(1)
