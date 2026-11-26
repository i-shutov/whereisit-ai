use crate::{
    selection::{select, Selection, COLUMNS},
    text,
};
use atlas_core::{Catalog, Error, Result};
use parquet::{
    basic::Compression,
    data_type::{ByteArray, ByteArrayType, Int64Type},
    file::{properties::WriterProperties, writer::SerializedFileWriter},
    schema::parser::parse_message_type,
};
use serde_json::{json, Value};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

const HEADERS: [&str; 15] = [
    "id",
    "volume_id",
    "parent_id",
    "name",
    "relative_path",
    "kind",
    "size",
    "modified_unix",
    "description",
    "tags",
    "volume_label",
    "categories",
    "duplicate_count",
    "duplicate_group",
    "modified_local",
];
const SCHEMA: &str = r#"message atlas_catalog {
 REQUIRED INT64 id;
 REQUIRED INT64 volume_id;
 OPTIONAL INT64 parent_id;
 REQUIRED BINARY name (UTF8);
 REQUIRED BINARY relative_path (UTF8);
 REQUIRED BINARY kind (UTF8);
 REQUIRED INT64 size;
 OPTIONAL INT64 modified_unix;
 REQUIRED BINARY description (UTF8);
 REQUIRED BINARY tags (UTF8);
 REQUIRED BINARY volume_label (UTF8);
 REQUIRED BINARY categories (UTF8);
 REQUIRED INT64 duplicate_count;
 REQUIRED BINARY duplicate_group (UTF8);
 OPTIONAL BINARY modified_local (UTF8);
}"#;

fn export_error(e: impl std::fmt::Display) -> Error {
    Error::new("export", e)
}

/// Never allow an export to replace the live database or one of its WAL files.
pub(crate) fn output_file(v: &Value) -> Result<(tempfile::NamedTempFile, PathBuf, bool)> {
    let target = PathBuf::from(text(v, "path")?);
    let db = PathBuf::from(text(v, "db")?);
    let overwrite = v.get("overwrite").and_then(Value::as_bool).unwrap_or(false);
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if !parent.is_dir() {
        return Err(Error::new(
            "invalid_path",
            "Output directory does not exist",
        ));
    }
    let file_name = target
        .file_name()
        .ok_or_else(|| Error::new("invalid_path", "Output needs a filename"))?;
    let resolved = parent.canonicalize()?.join(file_name);
    let db_abs = if db.exists() {
        db.canonicalize()?
    } else {
        std::env::current_dir()?.join(db)
    };
    let db_name = db_abs.as_os_str().to_string_lossy();
    if resolved == db_abs
        || resolved == format!("{db_name}-wal")
        || resolved == format!("{db_name}-shm")
    {
        return Err(Error::new(
            "unsafe_output",
            "Export cannot overwrite the working database",
        ));
    }
    if target.is_symlink() {
        return Err(Error::new(
            "unsafe_output",
            "Output may not be a symbolic link",
        ));
    }
    #[cfg(unix)]
    if target.exists() && db_abs.exists() {
        use std::os::unix::fs::MetadataExt;
        let a = target.metadata()?;
        let b = db_abs.metadata()?;
        if a.dev() == b.dev() && a.ino() == b.ino() {
            return Err(Error::new(
                "unsafe_output",
                "Output is a hard link to the database",
            ));
        }
    }
    if target.exists() && !overwrite {
        return Err(Error::new(
            "file_exists",
            "Output already exists; choose another name or set overwrite=true",
        ));
    }
    let tmp = tempfile::Builder::new()
        .prefix(".atlas-export-")
        .tempfile_in(parent)?;
    Ok((tmp, target, overwrite))
}
pub(crate) fn finish(tmp: tempfile::NamedTempFile, target: &Path, overwrite: bool) -> Result<()> {
    tmp.as_file().sync_all()?;
    publish(tmp, target, overwrite)
}
pub(crate) fn publish(tmp: tempfile::NamedTempFile, target: &Path, overwrite: bool) -> Result<()> {
    if overwrite {
        tmp.persist(target).map_err(export_error)?;
    } else {
        tmp.persist_noclobber(target).map_err(|error| {
            if error.error.kind() == std::io::ErrorKind::AlreadyExists {
                Error::new("file_exists", "Output already exists; choose another name")
            } else {
                export_error(error)
            }
        })?;
    }
    Ok(())
}

#[derive(Clone)]
pub(crate) struct ExportRow {
    integers: [Option<i64>; 5],
    text: [String; 9],
    modified_local: Option<String>,
}
impl ExportRow {
    fn read(r: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            integers: [
                Some(r.get(0)?),
                Some(r.get(1)?),
                r.get(2)?,
                Some(r.get(6)?),
                r.get(7)?,
            ],
            text: [
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(8)?,
                r.get(9)?,
                r.get(10)?,
                r.get(11)?,
                r.get::<_, i64>(12)?.to_string(),
                r.get(13)?,
            ],
            modified_local: r.get(14)?,
        })
    }
    fn csv(&self) -> Vec<String> {
        vec![
            self.integers[0].unwrap().to_string(),
            self.integers[1].unwrap().to_string(),
            self.integers[2].map(|n| n.to_string()).unwrap_or_default(),
            self.text[0].clone(),
            self.text[1].clone(),
            self.text[2].clone(),
            self.integers[3].unwrap().to_string(),
            self.integers[4].map(|n| n.to_string()).unwrap_or_default(),
            self.text[3].clone(),
            self.text[4].clone(),
            self.text[5].clone(),
            self.text[6].clone(),
            self.text[7].clone(),
            self.text[8].clone(),
            self.modified_local.clone().unwrap_or_default(),
        ]
    }

    pub(crate) fn from_parts(
        integers: [Option<i64>; 5],
        text: [String; 9],
        modified_local: Option<String>,
    ) -> Self {
        Self {
            integers,
            text,
            modified_local,
        }
    }

    fn json(&self) -> Value {
        json!({
            "id":self.integers[0].unwrap(),
            "volume_id":self.integers[1].unwrap(),
            "parent_id":self.integers[2],
            "name":self.text[0].as_str(),
            "relative_path":self.text[1].as_str(),
            "kind":self.text[2].as_str(),
            "size":self.integers[3].unwrap(),
            "modified_unix":self.integers[4],
            "description":self.text[3].as_str(),
            "tags":self.text[4].as_str(),
            "volume_label":self.text[5].as_str(),
            "categories":self.text[6].as_str(),
            "duplicate_count":self.text[7].parse::<i64>().unwrap_or(0),
            "duplicate_group":self.text[8].as_str(),
            "modified_local":self.modified_local.as_deref(),
        })
    }
}

/// Shared streaming writer for SQLite-backed and direct-preview exports.
/// Rows are bounded by the caller; Parquet buffers only one row group.
pub(crate) fn write_rows(
    file: &mut File,
    format: &str,
    bom: bool,
    rows: impl Iterator<Item = Result<ExportRow>>,
) -> Result<u64> {
    let mut count = 0u64;
    if format == "export_parquet" {
        let props = Arc::new(
            WriterProperties::builder()
                .set_compression(Compression::SNAPPY)
                .set_created_by("WhereIsIt AI 0.1".into())
                .build(),
        );
        let schema = Arc::new(parse_message_type(SCHEMA).map_err(export_error)?);
        let mut writer = SerializedFileWriter::new(file, schema, props).map_err(export_error)?;
        let mut batch = Vec::with_capacity(8192);
        for row in rows {
            batch.push(row?);
            count += 1;
            if batch.len() == 8192 {
                write_group(&mut writer, &batch)?;
                batch.clear();
            }
        }
        if !batch.is_empty() {
            write_group(&mut writer, &batch)?;
        }
        writer.close().map_err(export_error)?;
    } else if format == "export_json" {
        let mut writer = std::io::BufWriter::new(file);
        writer.write_all(b"{\"schema\":\"atlas-export-v1\",\"entries\":[")?;
        for row in rows {
            if count > 0 {
                writer.write_all(b",")?;
            }
            serde_json::to_writer(&mut writer, &row?.json()).map_err(export_error)?;
            count += 1;
        }
        writer.write_all(b"]}")?;
        writer.flush()?;
    } else if format == "export_csv" {
        if bom {
            file.write_all(b"\xEF\xBB\xBF")?;
        }
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(file);
        writer.write_record(HEADERS).map_err(export_error)?;
        for row in rows {
            writer.write_record(row?.csv()).map_err(export_error)?;
            count += 1;
        }
        writer.flush()?;
    } else {
        return Err(Error::new("export", "Unsupported export format"));
    }
    Ok(count)
}

pub(crate) fn export(c: &Catalog, v: &Value, format: &str) -> Result<Value> {
    let selection = select(v, false)?;
    export_impl(c, v, format, selection, true, true)
}

/// Linked CTF duplicates use a read-only query, never a mutable duplicate cache.
pub(crate) fn export_pinned_selected(
    c: &Catalog,
    v: &Value,
    format: &str,
    selection: Selection,
    private_stage: bool,
) -> Result<Value> {
    export_impl(c, v, format, selection, false, !private_stage)
}

fn export_impl(
    c: &Catalog,
    v: &Value,
    format: &str,
    s: Selection,
    use_editable_duplicate_cache: bool,
    enforce_candidate_no_clobber: bool,
) -> Result<Value> {
    // Keep candidate identity validation and the streamed result on one
    // SQLite read snapshot, so the selected key cannot change mid-export.
    let candidate_snapshot = if s.candidate {
        Some(c.conn().unchecked_transaction()?)
    } else {
        None
    };
    let source_conn = candidate_snapshot.as_deref().unwrap_or_else(|| c.conn());
    if s.candidate {
        let entry_id = crate::optional_id(v, "entry_id")?
            .ok_or_else(|| Error::new("invalid_request", "Select a positive file entry_id"))?;
        let table = c.query_table("")?;
        let exists: bool = source_conn.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id=?1 AND kind='file')"),
            [entry_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(Error::new(
                "not_found",
                "Selected file is unavailable in this catalog snapshot",
            ));
        }
    }
    let snapshot = if s.duplicate && use_editable_duplicate_cache {
        Some(crate::duplicates::export_snapshot(c, v)?)
    } else {
        None
    };
    let conn = snapshot.as_deref().unwrap_or(source_conn);
    conn.create_scalar_function(
        "ctf_local",
        1,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8
            | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let raw = ctx.get::<Option<i64>>(0)?;
            Ok(raw
                .and_then(|n| u32::try_from(n).ok())
                .and_then(crate::dates::dos_local))
        },
    )?;
    let has_ctf: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='ctf_entries')",
        [],
        |r| r.get(0),
    )?;
    let local_sql = if has_ctf {
        "ctf_local((SELECT modified_dos FROM ctf_entries WHERE entry_id=matches.id))"
    } else {
        "NULL"
    };
    let (mut tmp, target, overwrite) = if s.candidate && enforce_candidate_no_clobber {
        // Candidate exports are review artifacts and always publish without
        // replacing an existing user file, even if a caller asks otherwise.
        let mut safe = v.clone();
        safe["overwrite"] = json!(false);
        output_file(&safe)?
    } else {
        output_file(v)?
    };
    let sql = format!(
        "{} SELECT {COLUMNS},(SELECT label FROM volumes WHERE id=matches.volume_id),COALESCE((SELECT group_concat(c.name,'; ') FROM entry_categories ec JOIN categories c ON c.id=ec.category_id WHERE ec.entry_id=matches.id),''),duplicate_count,duplicate_group,{local_sql} FROM matches ORDER BY volume_id,id",
        s.cte
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(s.args.iter()))?;
    let count = write_rows(
        tmp.as_file_mut(),
        format,
        v.get("bom").and_then(Value::as_bool).unwrap_or(false),
        std::iter::from_fn(|| match rows.next() {
            Ok(Some(row)) => Some(ExportRow::read(row).map_err(Error::from)),
            Ok(None) => None,
            Err(error) => Some(Err(Error::from(error))),
        }),
    )?;
    drop(rows);
    drop(stmt);
    drop(snapshot);
    if let Some(transaction) = candidate_snapshot {
        transaction.commit()?;
    }
    let bytes = tmp.as_file().metadata()?.len();
    finish(tmp, &target, overwrite)?;
    let mut result = json!({
        "path":target,
        "rows":count,
        "bytes":bytes,
        "format":format.trim_start_matches("export_"),
        "scope":v.get("mode").and_then(Value::as_str).unwrap_or("search"),
        "duplicate_candidates":s.duplicate
    });
    if s.candidate {
        result["duplicate_candidates"] = json!(true);
        result["verified"] = json!(false);
        result["basis"] = json!("exact_normalized_name_and_size");
    }
    Ok(result)
}

fn write_group(writer: &mut SerializedFileWriter<&mut File>, rows: &[ExportRow]) -> Result<()> {
    let mut group = writer.next_row_group().map_err(export_error)?;
    for index in 0..15 {
        let mut column = group
            .next_column()
            .map_err(export_error)?
            .ok_or_else(|| Error::new("export", "Missing Parquet column"))?;
        if let Some(slot) = match index {
            0 => Some(0),
            1 => Some(1),
            2 => Some(2),
            6 => Some(3),
            7 => Some(4),
            _ => None,
        } {
            let values: Vec<i64> = rows.iter().filter_map(|r| r.integers[slot]).collect();
            let defs: Vec<i16> = rows
                .iter()
                .map(|r| i16::from(r.integers[slot].is_some()))
                .collect();
            column
                .typed::<Int64Type>()
                .write_batch(
                    &values,
                    if index == 2 || index == 7 {
                        Some(&defs)
                    } else {
                        None
                    },
                    None,
                )
                .map_err(export_error)?;
        } else if index == 14 {
            let values: Vec<ByteArray> = rows
                .iter()
                .filter_map(|r| r.modified_local.as_deref().map(ByteArray::from))
                .collect();
            let definitions: Vec<i16> = rows
                .iter()
                .map(|r| i16::from(r.modified_local.is_some()))
                .collect();
            column
                .typed::<ByteArrayType>()
                .write_batch(&values, Some(&definitions), None)
                .map_err(export_error)?;
        } else if index == 12 {
            let values: Vec<i64> = rows
                .iter()
                .map(|r| r.text[7].parse().unwrap_or(0))
                .collect();
            column
                .typed::<Int64Type>()
                .write_batch(&values, None, None)
                .map_err(export_error)?;
        } else {
            let slot = match index {
                3 => 0,
                4 => 1,
                5 => 2,
                8 => 3,
                9 => 4,
                10 => 5,
                11 => 6,
                13 => 8,
                _ => unreachable!(),
            };
            let values: Vec<ByteArray> = rows
                .iter()
                .map(|r| ByteArray::from(r.text[slot].as_str()))
                .collect();
            column
                .typed::<ByteArrayType>()
                .write_batch(&values, None, None)
                .map_err(export_error)?;
        }
        column.close().map_err(export_error)?;
    }
    group.close().map_err(export_error)?;
    Ok(())
}

#[cfg(test)]
mod preview_writer_tests {
    use super::*;

    #[test]
    fn cancelled_stream_stops_after_partial_rows_without_publishing() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("never-published.json");
        let mut stage = tempfile::Builder::new().tempfile_in(dir.path()).unwrap();
        let row = ExportRow::from_parts(
            [Some(1), Some(1), None, Some(17), None],
            [
                "one.txt".into(),
                "one.txt".into(),
                "file".into(),
                String::new(),
                String::new(),
                "disk".into(),
                String::new(),
                "0".into(),
                String::new(),
            ],
            None,
        );
        let rows = std::iter::once(Ok(row)).chain(std::iter::once(Err(Error::new(
            "cancelled",
            "cancelled during row stream",
        ))));
        assert_eq!(
            write_rows(stage.as_file_mut(), "export_json", false, rows)
                .unwrap_err()
                .code,
            "cancelled"
        );
        drop(stage);
        assert!(!target.exists());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}

#[cfg(test)]
mod candidate_tests {
    use super::*;
    use atlas_core::{Catalog, NewEntry};
    use std::fs;

    fn entry(name: &str, size: u64, relative_path: &str) -> NewEntry {
        NewEntry {
            parent_id: None,
            name: name.into(),
            relative_path: relative_path.into(),
            kind: "file".into(),
            size,
            modified_unix: None,
            description: "候補".into(),
            tags: "映画".into(),
        }
    }

    fn seed() -> (tempfile::TempDir, Catalog, i64, i64) {
        let tmp = tempfile::tempdir().unwrap();
        let mut catalog = Catalog::open(tmp.path().join("catalog.sqlite")).unwrap();
        let mut selected = 0;
        let mut second_volume = 0;
        catalog
            .bulk_import("Shelf A", "disk-A", |writer| {
                selected = writer.insert(&entry("映画 🎬.mkv", 4096, "a/selected.mkv"))?;
                for n in 0..149 {
                    writer.insert(&entry("映画 🎬.mkv", 4096, &format!("a/{n:03}.mkv")))?;
                }
                writer.insert(&entry("別名.mkv", 4097, "a/other-size.mkv"))?;
                Ok(())
            })
            .unwrap();
        catalog
            .bulk_import("Shelf B", "disk-B", |writer| {
                second_volume = writer.insert(&entry("映画 🎬.mkv", 4096, "b/000.mkv"))?;
                for n in 1..150 {
                    writer.insert(&entry("映画 🎬.mkv", 4096, &format!("b/{n:03}.mkv")))?;
                }
                writer.insert(&entry("映画-other.mkv", 4096, "b/other-name.mkv"))?;
                Ok(())
            })
            .unwrap();
        (tmp, catalog, selected, second_volume)
    }

    #[test]
    fn copy_candidates_export_all_metadata_matches_with_unicode_and_scopes() {
        let (tmp, catalog, selected, second_entry) = seed();
        let full_path = tmp.path().join("all.csv");
        let full = export(
            &catalog,
            &json!({"mode":"copy_candidates","entry_id":selected,"path":full_path,"db":tmp.path().join("catalog.sqlite"),"other_disks_only":false}),
            "export_csv",
        )
        .unwrap();
        assert_eq!(full["rows"], 300);
        assert_eq!(full["basis"], "exact_normalized_name_and_size");
        assert_eq!(full["verified"], false);
        assert_eq!(full["duplicate_candidates"], true);
        let mut csv = csv::Reader::from_path(&full_path).unwrap();
        let records = csv
            .records()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(records.len(), 300);
        assert!(records.iter().any(|row| row.get(3) == Some("映画 🎬.mkv")));
        assert!(records.iter().all(|row| row.get(6) == Some("4096")));
        assert!(records
            .iter()
            .all(|row| row.get(10) == Some("Shelf A") || row.get(10) == Some("Shelf B")));

        let other_path = tmp.path().join("other.json");
        let other = export(
            &catalog,
            &json!({"mode":"copy_candidates","entry_id":selected,"path":other_path,"db":tmp.path().join("catalog.sqlite"),"other_disks_only":true,"candidate_volume_id":catalog.conn().query_row("SELECT volume_id FROM entries WHERE id=?1",[second_entry],|r|r.get::<_,i64>(0)).unwrap()}),
            "export_json",
        )
        .unwrap();
        assert_eq!(other["rows"], 150);
        let value: Value = serde_json::from_slice(&fs::read(other_path).unwrap()).unwrap();
        let items = value["entries"].as_array().unwrap();
        assert_eq!(items.len(), 150);
        assert!(items
            .iter()
            .all(|item| item["volume_id"] == items[0]["volume_id"]));
        assert_eq!(items[0]["volume_label"], "Shelf B");
        assert!(items.iter().any(|item| item["name"] == "映画 🎬.mkv"));

        let parquet_path = tmp.path().join("candidates.parquet");
        let parquet = export(
            &catalog,
            &json!({"mode":"copy_candidates","entry_id":selected,"path":parquet_path,"db":tmp.path().join("catalog.sqlite"),"other_disks_only":true}),
            "export_parquet",
        )
        .unwrap();
        assert_eq!(parquet["rows"], 150);
        assert!(parquet["bytes"].as_u64().unwrap() > 0);
    }

    #[test]
    fn candidate_export_rejects_bad_selection_and_never_clobbers() {
        let (tmp, catalog, selected, _) = seed();
        let db = tmp.path().join("catalog.sqlite");
        let target = tmp.path().join("preserve.csv");
        fs::write(&target, b"keep existing bytes").unwrap();
        let err = export(
            &catalog,
            &json!({"mode":"copy_candidates","entry_id":selected,"path":target,"db":db,"overwrite":true}),
            "export_csv",
        )
        .unwrap_err();
        assert_eq!(err.code, "file_exists");
        assert_eq!(fs::read(&target).unwrap(), b"keep existing bytes");

        for entry_id in [0, -1, 999_999] {
            let err = export(
                &catalog,
                &json!({"mode":"copy_candidates","entry_id":entry_id,"path":tmp.path().join(format!("bad-{entry_id}.csv")),"db":db}),
                "export_csv",
            )
            .unwrap_err();
            assert!(matches!(err.code, "invalid_request" | "not_found"));
        }
        assert!(export(
            &catalog,
            &json!({"mode":"copy_candidates","entry_id":selected,"query":"name:movie","path":tmp.path().join("query.csv"),"db":db}),
            "export_csv",
        )
        .is_err());
    }
}
