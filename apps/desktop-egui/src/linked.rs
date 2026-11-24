//! Pure request/response adapter for a read-only linked CTF generation.
//!
//! This module deliberately never exposes the derived SQLite mirror path as a
//! catalog path. Every request is pinned to the source path and generation.
use serde_json::{json, Map, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedCtf {
    pub path: String,
    pub cache_root: String,
    pub generation: String,
    pub source_sha256: String,
    pub preview: bool,
}

impl LinkedCtf {
    #[cfg(test)]
    pub fn matches_source_snapshot(&self, path: &str, source_sha256: &str) -> bool {
        self.path == path && self.source_sha256 == source_sha256
    }

    /// Parse the `data` member of a successful `ctf_sync` service response.
    #[cfg(test)]
    pub fn parse_sync(path: String, cache_root: String, data: &Value) -> Result<Self, String> {
        if path.trim().is_empty() || cache_root.trim().is_empty() {
            return Err("CTF source and cache paths are required".to_owned());
        }
        if data.get("state").and_then(Value::as_str) != Some("ready") {
            return Err("CTF sync did not return a ready generation".to_owned());
        }
        let generation = digest_field(data, "generation")?;
        let source_sha256 = digest_field(data, "source_sha256")?;
        if data.get("original_ctf_read_only").and_then(Value::as_bool) != Some(true)
            || data
                .get("native_annotation_writes_allowed")
                .and_then(Value::as_bool)
                != Some(false)
        {
            return Err("CTF sync did not confirm a read-only source".to_owned());
        }
        Ok(Self {
            path,
            cache_root,
            generation,
            source_sha256,
            preview: false,
        })
    }

    /// Validated read-only source view used directly for browsing and search.
    pub fn parse_preview(path: String, cache_root: String, data: &Value) -> Result<Self, String> {
        if path.trim().is_empty() {
            return Err("CTF source path is required".to_owned());
        }
        if data.get("state").and_then(Value::as_str) != Some("browsable")
            || data.get("original_ctf_read_only").and_then(Value::as_bool) != Some(true)
            || data
                .get("native_annotation_writes_allowed")
                .and_then(Value::as_bool)
                != Some(false)
        {
            return Err("CTF preview did not confirm a browsable read-only source".to_owned());
        }
        Ok(Self {
            path,
            cache_root,
            generation: digest_field(data, "generation")?,
            source_sha256: digest_field(data, "source_sha256")?,
            preview: true,
        })
    }

    /// Translate a SQLite-shaped egui request into an allowlisted pinned CTF
    /// request. The incoming `db` field is intentionally ignored.
    pub fn rewrite(&self, request: Value) -> Result<Value, String> {
        let op = request
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| "Request operation is missing".to_owned())?;
        if self.preview {
            let mut out = Map::new();
            out.insert("path".to_owned(), json!(self.path));
            out.insert("generation".to_owned(), json!(self.generation));
            let (mapped_op, fields): (&str, &[&str]) =
                match op {
                    "volumes" => ("ctf_preview_volumes", &[]),
                    "search_fast" => (
                        "ctf_preview_search",
                        &[
                            "query",
                            "volume_id",
                            "offset",
                            "limit",
                            "full_count",
                            "job_id",
                        ],
                    ),
                    "children" => (
                        "ctf_preview_children",
                        &["volume_id", "parent_id", "offset", "limit"],
                    ),
                    "children_focused" => (
                        "ctf_preview_children_focused",
                        &["volume_id", "parent_id", "entry_id", "limit"],
                    ),
                    _ => return Err(
                        "This operation requires importing the CTF into a native SQLite catalog"
                            .to_owned(),
                    ),
                };
            out.insert("op".to_owned(), json!(mapped_op));
            copy_fields(&request, &mut out, fields);
            return Ok(Value::Object(out));
        }
        let mut out = Map::new();
        out.insert("path".to_owned(), json!(self.path));
        out.insert("cache_root".to_owned(), json!(self.cache_root));
        out.insert("generation".to_owned(), json!(self.generation));

        let (mapped_op, fields): (&str, &[&str]) = match op {
            "volumes" => ("ctf_volumes_pinned", &[]),
            "search_fast" => (
                "ctf_search_pinned",
                &["query", "volume_id", "offset", "limit", "full_count"],
            ),
            "children" => (
                "ctf_children_pinned",
                &["volume_id", "parent_id", "offset", "limit"],
            ),
            "children_focused" => (
                "ctf_children_focused_pinned",
                &["volume_id", "parent_id", "entry_id", "limit"],
            ),
            "copy_candidates" => (
                "ctf_copy_candidates_pinned",
                &[
                    "entry_id",
                    "candidate_volume_id",
                    "other_disks_only",
                    "offset",
                    "limit",
                ],
            ),
            "export_csv" | "export_json" | "export_parquet" => {
                if request.get("overwrite").and_then(Value::as_bool) == Some(true) {
                    return Err("Linked CTF exports never overwrite an existing file".to_owned());
                }
                let format = match op {
                    "export_csv" => "csv",
                    "export_json" => "json",
                    _ => "parquet",
                };
                let output = request
                    .get("path")
                    .and_then(Value::as_str)
                    .filter(|path| !path.trim().is_empty())
                    .ok_or_else(|| "A new export output path is required".to_owned())?;
                if std::path::Path::new(output)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_none_or(|ext| !ext.eq_ignore_ascii_case(format))
                {
                    return Err("Export filename extension must match its format".to_owned());
                }
                if output.eq_ignore_ascii_case(&self.path) {
                    return Err("CTF export cannot target its source file".to_owned());
                }
                out.insert("op".to_owned(), json!("ctf_export_pinned"));
                out.insert("output".to_owned(), json!(output));
                out.insert("format".to_owned(), json!(format));
                out.insert("overwrite".to_owned(), json!(false));
                let fields = [
                    "entry_id",
                    "mode",
                    "query",
                    "volume_id",
                    "parent_id",
                    "duplicate_mode",
                    "candidate_volume_id",
                    "other_disks_only",
                ];
                copy_fields(&request, &mut out, &fields);
                return Ok(Value::Object(out));
            }
            _ => {
                return Err(format!(
                    "Operation {op} is not available for a read-only linked CTF"
                ));
            }
        };
        out.insert("op".to_owned(), json!(mapped_op));
        copy_fields(&request, &mut out, fields);
        Ok(Value::Object(out))
    }

    /// Unwrap a pinned service envelope only when it belongs to this exact
    /// source generation. The returned shape is the normal egui `{ok,data}`.
    pub fn unwrap_reply(&self, response: &Value) -> Result<Value, String> {
        if response.get("ok").and_then(Value::as_bool) != Some(true) {
            let message = response
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("Linked CTF request failed");
            return Err(message.to_owned());
        }
        let data = response
            .get("data")
            .ok_or_else(|| "Linked CTF response has no data".to_owned())?;
        if data.get("generation").and_then(Value::as_str) != Some(self.generation.as_str())
            || data.get("source_sha256").and_then(Value::as_str)
                != Some(self.source_sha256.as_str())
            || data.get("read_only").and_then(Value::as_bool) != Some(true)
        {
            return Err("Linked CTF response is stale or has invalid identity".to_owned());
        }
        let result = data
            .get("result")
            .cloned()
            .ok_or_else(|| "Linked CTF response has no result".to_owned())?;
        Ok(json!({"ok":true,"data":result}))
    }
}

fn digest_field(data: &Value, key: &str) -> Result<String, String> {
    let value = data
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| format!("CTF sync returned an invalid {key}"))?;
    Ok(value.to_ascii_lowercase())
}

fn copy_fields(request: &Value, output: &mut Map<String, Value>, fields: &[&str]) {
    for field in fields {
        if let Some(value) = request.get(*field) {
            output.insert((*field).to_owned(), value.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generation() -> String {
        "a".repeat(64)
    }

    fn source_sha() -> String {
        "b".repeat(64)
    }

    fn linked() -> LinkedCtf {
        LinkedCtf::parse_sync(
            "C:/fixtures/source.ctf".to_owned(),
            "C:/fixtures/cache".to_owned(),
            &json!({
                "state":"ready",
                "generation":generation(),
                "source_sha256":source_sha(),
                "original_ctf_read_only":true,
                "native_annotation_writes_allowed":false
            }),
        )
        .unwrap()
    }

    #[test]
    fn sync_requires_ready_immutable_generation_and_valid_digests() {
        let mut data = json!({
            "state":"ready", "generation":generation(), "source_sha256":source_sha(),
            "original_ctf_read_only":true, "native_annotation_writes_allowed":false
        });
        assert!(LinkedCtf::parse_sync("x.ctf".into(), "cache".into(), &data).is_ok());
        data["native_annotation_writes_allowed"] = json!(true);
        assert!(LinkedCtf::parse_sync("x.ctf".into(), "cache".into(), &data).is_err());
        data["native_annotation_writes_allowed"] = json!(false);
        data["generation"] = json!("bad");
        assert!(LinkedCtf::parse_sync("x.ctf".into(), "cache".into(), &data).is_err());
    }

    #[test]
    fn query_requests_are_pinned_and_keep_scope_without_db_path() {
        let linked = linked();
        let rewritten = linked
            .rewrite(json!({"op":"search_fast","db":"DO_NOT_USE.sqlite",
                "query":"ext:mp3 AND name:песня","volume_id":7,"offset":250,"limit":50}))
            .unwrap();
        assert_eq!(rewritten["op"], "ctf_search_pinned");
        assert_eq!(rewritten["path"], linked.path);
        assert_eq!(rewritten["generation"], linked.generation);
        assert!(rewritten.get("db").is_none());
        assert_eq!(rewritten["query"], "ext:mp3 AND name:песня");
        assert_eq!(rewritten["offset"], 250);
        let copies = linked
            .rewrite(json!({"op":"copy_candidates","entry_id":17,
                "other_disks_only":true,"offset":30,"limit":10}))
            .unwrap();
        assert_eq!(copies["op"], "ctf_copy_candidates_pinned");
        assert_eq!(copies["entry_id"], 17);
        assert_eq!(copies["other_disks_only"], true);
    }

    #[test]
    fn export_maps_output_and_scope_with_no_clobber() {
        let linked = linked();
        let export = linked
            .rewrite(json!({"op":"export_json","db":"never.sqlite",
                "path":"C:/fixtures/results.json","mode":"children",
                "volume_id":4,"parent_id":12,"overwrite":false}))
            .unwrap();
        assert_eq!(export["op"], "ctf_export_pinned");
        assert_eq!(export["path"], linked.path);
        assert_eq!(export["output"], "C:/fixtures/results.json");
        assert_eq!(export["overwrite"], false);
        assert_eq!(export["mode"], "children");
        assert_eq!(export["parent_id"], 12);
        assert!(export.get("db").is_none());
        let candidates = linked.rewrite(json!({"op":"export_parquet","path":"C:/fixtures/candidates.parquet", "mode":"copy_candidates", "entry_id":17, "other_disks_only":true, "overwrite":false})).unwrap();
        assert_eq!(candidates["mode"], "copy_candidates");
        assert_eq!(candidates["entry_id"], 17);
        assert_eq!(candidates["other_disks_only"], true);
        assert!(candidates.get("limit").is_none());
        assert!(linked
            .rewrite(json!({"op":"export_csv","path":"C:/fixtures/results.csv","overwrite":true}))
            .is_err());
        assert!(linked
            .rewrite(json!({"op":"export_parquet","path":"C:/fixtures/results.csv"}))
            .is_err());
    }

    #[test]
    fn mutations_and_unlisted_operations_are_refused() {
        let linked = linked();
        for op in [
            "scan",
            "import_ctf",
            "export_ctf",
            "disk_preferences_set",
            "categories",
            "assign_category",
        ] {
            assert!(linked.rewrite(json!({"op":op})).is_err(), "{op}");
        }
    }

    #[test]
    fn preview_routes_only_folder_browsing_and_pins_source_generation() {
        let preview_generation = "a".repeat(64);
        let preview = LinkedCtf::parse_preview(
            "source.ctf".into(),
            String::new(),
            &json!({
                "state":"browsable", "generation":preview_generation, "source_sha256":source_sha(),
                "source_bytes":140000000, "original_ctf_read_only":true,
                "native_annotation_writes_allowed":false
            }),
        )
        .unwrap();
        assert!(preview.preview);
        assert!(preview.matches_source_snapshot("source.ctf", &source_sha()));
        assert!(!preview.matches_source_snapshot("other.ctf", &source_sha()));
        assert!(!preview.matches_source_snapshot("source.ctf", &"c".repeat(64)));
        let volumes = preview
            .rewrite(json!({"op":"volumes","db":"ignored.sqlite"}))
            .unwrap();
        assert_eq!(volumes["op"], "ctf_preview_volumes");
        assert_eq!(volumes["path"], "source.ctf");
        assert_eq!(volumes["generation"], "a".repeat(64));
        assert!(volumes.get("db").is_none());
        let children = preview
            .rewrite(json!({"op":"children","volume_id":2,"parent_id":4,"offset":50,"limit":25}))
            .unwrap();
        assert_eq!(children["op"], "ctf_preview_children");
        assert_eq!(children["parent_id"], 4);
        assert_eq!(children["limit"], 25);
        let focused = preview
            .rewrite(json!({"op":"children_focused","volume_id":2,"parent_id":4,"entry_id":19,"limit":250}))
            .unwrap();
        assert_eq!(focused["op"], "ctf_preview_children_focused");
        assert_eq!(focused["entry_id"], 19);
        let search = preview
            .rewrite(json!({"op":"search_fast","query":"ext:mkv AND name:фильм",
                "volume_id":4,"offset":50,"limit":25,"job_id":"search-1"}))
            .unwrap();
        assert_eq!(search["op"], "ctf_preview_search");
        assert_eq!(search["path"], "source.ctf");
        assert_eq!(search["generation"], preview_generation);
        assert_eq!(search["query"], "ext:mkv AND name:фильм");
        assert_eq!(search["volume_id"], 4);
        assert_eq!(search["offset"], 50);
        assert_eq!(search["limit"], 25);
        assert_eq!(search["job_id"], "search-1");
        assert!(preview
            .rewrite(json!({"op":"categories","db":"ignored.sqlite"}))
            .unwrap_err()
            .contains("requires importing"));
        assert!(preview
            .rewrite(json!({"op":"update_ctf_description","entry_id":1}))
            .is_err());
    }

    #[test]
    fn preview_open_rejects_non_browsable_or_write_enabled_sources() {
        let data = json!({"state":"browsable","generation":source_sha(),"source_sha256":source_sha(),
            "original_ctf_read_only":true,"native_annotation_writes_allowed":true});
        assert!(LinkedCtf::parse_preview("x.ctf".into(), "cache".into(), &data).is_err());
    }

    #[test]
    fn reply_requires_matching_generation_source_and_read_only_marker() {
        let linked = linked();
        let response = json!({"ok":true,"data":{
            "generation":generation(),"source_sha256":source_sha(),"read_only":true,
            "result":{"items":[{"name":"песня.mp3"}],"total":1}
        }});
        assert_eq!(linked.unwrap_reply(&response).unwrap()["data"]["total"], 1);
        for bad in [
            json!({"ok":true,"data":{"generation":"stale","source_sha256":source_sha(),"read_only":true,"result":{}}}),
            json!({"ok":true,"data":{"generation":generation(),"source_sha256":"bad","read_only":true,"result":{}}}),
            json!({"ok":true,"data":{"generation":generation(),"source_sha256":source_sha(),"read_only":false,"result":{}}}),
        ] {
            assert!(linked.unwrap_reply(&bad).is_err());
        }
    }
}
