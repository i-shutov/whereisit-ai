use crate::{
    selection::{select, COLUMNS},
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
        || resolved == PathBuf::from(format!("{db_name}-wal"))
        || resolved == PathBuf::from(format!("{db_name}-shm"))
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
    if overwrite {
        tmp.persist(target).map_err(export_error)?;
    } else {
        tmp.persist_noclobber(target).map_err(export_error)?;
    }
    Ok(())
}

#[derive(Clone)]
struct ExportRow {
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
}

pub(crate) fn export(c: &Catalog, v: &Value, format: &str) -> Result<Value> {
    let s = select(v, false)?;
    let snapshot = if s.duplicate {
        Some(crate::duplicates::export_snapshot(c, v)?)
    } else {
        None
    };
    let conn = snapshot.as_deref().unwrap_or(c.conn());
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
    let (mut tmp, target, overwrite) = output_file(v)?;
    let sql=format!("{} SELECT {COLUMNS},(SELECT label FROM volumes WHERE id=matches.volume_id),COALESCE((SELECT group_concat(c.name,'; ') FROM entry_categories ec JOIN categories c ON c.id=ec.category_id WHERE ec.entry_id=matches.id),''),duplicate_count,duplicate_group,{local_sql} FROM matches ORDER BY volume_id,id",s.cte);
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(rusqlite::params_from_iter(s.args.iter()))?;
    let mut count = 0u64;
    if format == "export_parquet" {
        let props = Arc::new(
            WriterProperties::builder()
                .set_compression(Compression::SNAPPY)
                .set_created_by("WhereIsIt AI 0.1".into())
                .build(),
        );
        let schema = Arc::new(parse_message_type(SCHEMA).map_err(export_error)?);
        let mut writer =
            SerializedFileWriter::new(tmp.as_file_mut(), schema, props).map_err(export_error)?;
        let mut batch = Vec::with_capacity(8192);
        while let Some(r) = rows.next()? {
            batch.push(ExportRow::read(r)?);
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
        let mut writer = std::io::BufWriter::new(tmp.as_file_mut());
        writer.write_all(b"{\"schema\":\"atlas-export-v1\",\"entries\":[")?;
        while let Some(r) = rows.next()? {
            if count > 0 {
                writer.write_all(b",")?;
            }
            let mut item = serde_json::to_value(atlas_core::entry_row(r)?).map_err(export_error)?;
            item["volume_label"] = json!(r.get::<_, String>(10)?);
            item["categories"] = json!(r.get::<_, String>(11)?);
            item["duplicate_count"] = json!(r.get::<_, i64>(12)?);
            item["duplicate_group"] = json!(r.get::<_, String>(13)?);
            item["modified_local"] = json!(r.get::<_, Option<String>>(14)?);
            serde_json::to_writer(&mut writer, &item).map_err(export_error)?;
            count += 1;
        }
        writer.write_all(b"]}")?;
        writer.flush()?;
    } else {
        if v.get("bom").and_then(Value::as_bool).unwrap_or(false) {
            tmp.write_all(b"\xEF\xBB\xBF")?;
        }
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(tmp.as_file_mut());
        writer.write_record(HEADERS).map_err(export_error)?;
        while let Some(r) = rows.next()? {
            writer
                .write_record(ExportRow::read(r)?.csv())
                .map_err(export_error)?;
            count += 1;
        }
        writer.flush()?;
    }
    drop(rows);
    drop(stmt);
    drop(snapshot);
    let bytes = tmp.as_file().metadata()?.len();
    finish(tmp, &target, overwrite)?;
    Ok(
        json!({"path":target,"rows":count,"bytes":bytes,"format":format.trim_start_matches("export_"),"scope":v.get("mode").and_then(Value::as_str).unwrap_or("search"),"duplicate_candidates":s.duplicate}),
    )
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
