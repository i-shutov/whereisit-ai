//! Tolerant, typed presentation model for the existing atlas-service contract.
//! Catalog data and volume identities come only from the Rust backend.
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    pub id: i64,
    pub label: String,
    pub root_path: String,
    pub entry_count: u64,
    pub total_bytes: u64,
}

impl Volume {
    pub fn parse(value: &Value) -> Result<Self, String> {
        Ok(Self {
            id: positive_id(value, "id")?,
            label: string(value, "label")?.to_owned(),
            root_path: string(value, "root_path").unwrap_or("").to_owned(),
            entry_count: unsigned(value, "entry_count")?,
            total_bytes: unsigned(value, "total_bytes")?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: i64,
    pub volume_id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    pub relative_path: String,
    pub kind: String,
    pub size: u64,
    pub modified_unix: Option<i64>,
    pub description: String,
    pub tags: String,
}

impl Entry {
    pub fn parse(v: &Value) -> Result<Self, String> {
        Ok(Self {
            id: positive_id(v, "id")?,
            volume_id: positive_id(v, "volume_id")?,
            parent_id: v.get("parent_id").and_then(Value::as_i64),
            name: string(v, "name")?.to_owned(),
            relative_path: string(v, "relative_path")?.to_owned(),
            kind: string(v, "kind")?.to_owned(),
            size: unsigned(v, "size")?,
            modified_unix: v.get("modified_unix").and_then(Value::as_i64),
            description: string(v, "description").unwrap_or("").to_owned(),
            tags: string(v, "tags").unwrap_or("").to_owned(),
        })
    }

    pub fn is_directory(&self) -> bool {
        self.kind == "directory"
    }
}

#[derive(Clone, Debug, Default)]
pub struct Page {
    pub items: Vec<Entry>,
    pub offset: u64,
    pub limit: u64,
    pub has_more: bool,
}

impl Page {
    pub fn parse(v: &Value) -> Result<Self, String> {
        let raw = v
            .get("items")
            .and_then(Value::as_array)
            .ok_or_else(|| "service response is missing items[]".to_owned())?;
        let items = raw
            .iter()
            .map(Entry::parse)
            .collect::<Result<Vec<_>, _>>()?;
        let offset = unsigned(v, "offset")?;
        let limit = unsigned(v, "limit")?;
        let has_more = v
            .get("has_more")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| {
                v.get("total")
                    .and_then(Value::as_u64)
                    .is_some_and(|total| offset.saturating_add(limit) < total)
            });
        Ok(Self {
            items,
            offset,
            limit,
            has_more,
        })
    }
}

pub fn parse_volumes(v: &Value) -> Result<Vec<Volume>, String> {
    v.as_array()
        .ok_or_else(|| "service response is missing volumes[]".to_owned())?
        .iter()
        .map(Volume::parse)
        .collect()
}

fn positive_id(v: &Value, key: &str) -> Result<i64, String> {
    v.get(key)
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| format!("invalid {key}"))
}
fn unsigned(v: &Value, key: &str) -> Result<u64, String> {
    v.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("invalid {key}"))
}
fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("invalid {key}"))
}

pub fn bytes_compact(size: u64) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut value = size as f64;
    let mut index = 0;
    while value >= 1024.0 && index + 1 < units.len() {
        value /= 1024.0;
        index += 1;
    }
    if index == 0 {
        format!("{size} B")
    } else {
        format!("{value:.1} {}", units[index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn size_and_unicode_roundtrip() {
        assert_eq!(bytes_compact(0), "0 B");
        assert_eq!(bytes_compact(2 * 1024 * 1024 * 1024), "2.0 GiB");
        let input = json!({"id":2,"volume_id":4,"parent_id":null,"name":"Кино.mkv",
            "relative_path":"Видео/Кино.mkv","kind":"file","size":1024,"modified_unix":null,
            "description":"","tags":""});
        assert_eq!(Entry::parse(&input).unwrap().name, "Кино.mkv");
    }

    #[test]
    fn page_distinguishes_true_has_more_from_counted_paging() {
        let p = json!({"items":[],"offset":250,"limit":250,"has_more":true});
        assert!(Page::parse(&p).unwrap().has_more);
        let counted = json!({"items":[],"offset":250,"limit":250,"total":260});
        assert!(!Page::parse(&counted).unwrap().has_more);
        assert!(Page::parse(&json!({"offset":0,"limit":100})).is_err());
    }
}
