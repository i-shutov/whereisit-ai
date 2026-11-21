//! Typed adapter for the existing SQLite category service.
//! No shadow SQL or mocked category source of truth in the desktop client.
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
}

impl Category {
    fn parse(value: &Value) -> Result<Self, String> {
        let id = value
            .get("id")
            .and_then(Value::as_i64)
            .filter(|id| *id > 0)
            .ok_or_else(|| "Invalid category ID from service".to_owned())?;
        let parent_id = match value.get("parent_id") {
            Some(Value::Null) => None,
            Some(parent) => Some(
                parent
                    .as_i64()
                    .filter(|parent| *parent > 0 && *parent != id)
                    .ok_or_else(|| "Invalid category parent from service".to_owned())?,
            ),
            None => return Err("Category parent is missing from service".to_owned()),
        };
        let name = value
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| "Invalid category name from service".to_owned())?
            .to_owned();
        Ok(Self {
            id,
            parent_id,
            name,
        })
    }

    /// Categories can share a name. The ID is the only stable search address.
    pub fn search_query(&self) -> String {
        format!("category_id:{}", self.id)
    }
}


/// Human-readable disambiguator for repeated category names. Always show the
/// immutable category ID and the full available ancestry, without unbounded
/// traversal when a malformed historical catalog contains a cycle.
pub fn display_path(
    category: &Category,
    by_id: &std::collections::HashMap<i64, &Category>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut cursor = Some(category.id);
    while let Some(id) = cursor {
        if !visited.insert(id) {
            parts.push(format!("[cycle #{id}]"));
            break;
        }
        if parts.len() >= 64 {
            parts.push("…".to_owned());
            break;
        }
        let item = if id == category.id {
            Some(category)
        } else {
            by_id.get(&id).copied()
        };
        match item {
            Some(item) => {
                parts.push(item.name.clone());
                cursor = item.parent_id;
            }
            None => {
                parts.push(format!("[missing #{id}]"));
                break;
            }
        }
    }
    parts.reverse();
    format!("{} · #{}", parts.join(" / "), category.id)
}

pub fn parse_categories(value: &Value) -> Result<Vec<Category>, String> {
    let list = value
        .as_array()
        .ok_or_else(|| "Service category list is not an array".to_owned())?;
    let categories: Vec<Category> = list.iter().map(Category::parse).collect::<Result<_, _>>()?;
    let mut ids = std::collections::HashSet::new();
    if categories.iter().any(|category| !ids.insert(category.id)) {
        return Err("Service returned duplicate category IDs".to_owned());
    }
    Ok(categories)
}

pub fn new_name(input: &str) -> Result<&str, &'static str> {
    let name = input.trim();
    if name.is_empty() {
        return Err("empty");
    }
    if name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err("invalid");
    }
    Ok(name)
}

pub fn list_request(db: &str) -> Value {
    json!({"op":"categories","db":db})
}

pub fn create_request(db: &str, name: &str, parent_id: Option<i64>) -> Value {
    json!({"op":"create_category","db":db,"name":name,"parent_id":parent_id})
}

pub fn assign_request(db: &str, entry_id: i64, category_id: i64) -> Value {
    json!({"op":"assign_category","db":db,"entry_id":entry_id,"category_id":category_id})
}

pub fn list_assigned_request(db: &str, entry_id: i64) -> Value {
    json!({"op":"entry_category_ids","db":db,"entry_id":entry_id})
}

pub fn unassign_request(db: &str, entry_id: i64, category_id: i64) -> Value {
    json!({"op":"unassign_category","db":db,"entry_id":entry_id,"category_id":category_id})
}

pub fn parse_assignment_ids(value: &Value) -> Result<std::collections::HashSet<i64>, String> {
    let ids = value
        .as_array()
        .ok_or_else(|| "Service category assignments must be an array".to_owned())?;
    let mut parsed = std::collections::HashSet::new();
    for id in ids {
        let id = id
            .as_i64()
            .filter(|id| *id > 0)
            .ok_or_else(|| "Service returned an invalid category assignment".to_owned())?;
        if !parsed.insert(id) {
            return Err("Service returned duplicate category assignment".to_owned());
        }
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_unicode_categories_and_rejects_inconsistent_ids() {
        let list = json!([
            {"id":2,"parent_id":null,"name":"Кино"},
            {"id":3,"parent_id":2,"name":"Семейное"}
        ]);
        let parsed = parse_categories(&list).unwrap();
        assert_eq!(parsed[1].parent_id, Some(2));
        assert_eq!(parsed[0].search_query(), "category_id:2");
        assert!(parse_categories(&json!([{"id":2,"parent_id":2,"name":"Cycle"}])).is_err());
        assert!(parse_categories(&json!([
            {"id":2,"parent_id":null,"name":"A"},
            {"id":2,"parent_id":null,"name":"B"}
        ]))
        .is_err());
    }

    #[test]
    fn category_query_uses_id_not_ambiguous_unicode_name() {
        let category = Category {
            id: 4,
            parent_id: None,
            name: "Sci-Fi / Кино \"Director's\\cut\"".into(),
        };
        assert_eq!(category.search_query(), "category_id:4");
        assert_eq!(new_name("  Кино  "), Ok("Кино"));
        assert_eq!(new_name(" \t\n"), Err("empty"));
        assert_eq!(new_name("x\nz"), Err("invalid"));
        assert_eq!(new_name(&"x".repeat(81)), Err("invalid"));
    }

    #[test]
    fn category_labels_disambiguate_duplicate_names_and_ancestry() {
        let items = parse_categories(&json!([
            {"id":17,"parent_id":null,"name":"Видео"},
            {"id":29,"parent_id":null,"name":"Видео"},
            {"id":31,"parent_id":17,"name":"Документальные"}
        ]))
        .unwrap();
        let lookup: std::collections::HashMap<_, _> =
            items.iter().map(|category| (category.id, category)).collect();
        assert_eq!(display_path(&items[0], &lookup), "Видео · #17");
        assert_eq!(display_path(&items[1], &lookup), "Видео · #29");
        assert_eq!(
            display_path(&items[2], &lookup),
            "Видео / Документальные · #31"
        );
        let cycle = vec![
            Category { id: 50, parent_id: Some(51), name: "A".into() },
            Category { id: 51, parent_id: Some(50), name: "B".into() }
        ];
        let cycle_lookup: std::collections::HashMap<_, _> =
            cycle.iter().map(|c| (c.id, c)).collect();
        let shown = display_path(&cycle[0], &cycle_lookup);
        assert!(shown.contains("[cycle #50]"));
        assert!(shown.ends_with("· #50"));
    }

    #[test]
    fn request_contracts_do_not_fake_category_or_entry_ids() {
        assert_eq!(list_request("catalog.sqlite")["op"], "categories");
        let created = create_request("catalog.sqlite", "Кино", None);
        assert_eq!(created["parent_id"], Value::Null);
        assert_eq!(created["name"], "Кино");
        let assigned = assign_request("catalog.sqlite", 42, 7);
        assert_eq!(assigned["entry_id"], 42);
        assert_eq!(assigned["category_id"], 7);
        assert_eq!(list_assigned_request("catalog.sqlite", 42)["entry_id"], 42);
        assert_eq!(unassign_request("catalog.sqlite", 42, 7)["category_id"], 7);
        assert_eq!(parse_assignment_ids(&json!([5, 7])).unwrap().len(), 2);
        assert!(parse_assignment_ids(&json!([5, 5])).is_err());
        assert!(parse_assignment_ids(&json!([0])).is_err());
        assert!(parse_assignment_ids(&json!({"id":5})).is_err());
    }

    #[test]
    fn native_service_category_round_trip_on_generated_sqlite() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("catalog.sqlite");
        let disk = tmp.path().join("generated-volume");
        std::fs::create_dir(&disk).unwrap();
        std::fs::write(disk.join("film.mkv"), b"synthetic fixture only").unwrap();
        let call = |request: Value| -> Value {
            serde_json::from_str(&atlas_service::dispatch_json(&request.to_string())).unwrap()
        };
        assert_eq!(call(json!({"op":"init","db":db}))["ok"], true);
        let scan = call(json!({"op":"scan","db":db,"root":disk,
            "label":"Synthetic","hash":false}));
        assert_eq!(scan["ok"], true, "{scan}");
        let movies =
            call(json!({"op":"search_fast","db":db,"query":"ext:mkv","offset":0,"limit":250}));
        let entry_id = movies["data"]["items"][0]["id"].as_i64().unwrap();
        let created = call(create_request(db.to_str().unwrap(), "Семейное Кино", None));
        assert_eq!(created["ok"], true, "{created}");
        let category_id = created["data"]["id"].as_i64().unwrap();
        let categories = call(list_request(db.to_str().unwrap()));
        let parsed = parse_categories(&categories["data"]).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].id, category_id);
        assert_eq!(parsed[0].name, "Семейное Кино");
        let assigned = call(assign_request(db.to_str().unwrap(), entry_id, category_id));
        assert_eq!(assigned["ok"], true, "{assigned}");
        let before = call(list_assigned_request(db.to_str().unwrap(), entry_id));
        assert_eq!(before["ok"], true, "{before}");
        assert!(parse_assignment_ids(&before["data"])
            .unwrap()
            .contains(&category_id));
        let matched = call(json!({"op":"search_fast","db":db,
            "query":parsed[0].search_query(),"offset":0,"limit":250}));
        assert_eq!(matched["ok"], true, "{matched}");
        assert_eq!(matched["data"]["items"][0]["id"], entry_id);
        assert_eq!(
            call(assign_request(db.to_str().unwrap(), entry_id, category_id))["ok"],
            true
        );
        // Two distinct categories may share an identical display name.
        let home = call(create_request(db.to_str().unwrap(), "Видео", None));
        let work = call(create_request(db.to_str().unwrap(), "Видео", None));
        assert_eq!(home["ok"], true, "{home}");
        assert_eq!(work["ok"], true, "{work}");
        let home_id = home["data"]["id"].as_i64().unwrap();
        let work_id = work["data"]["id"].as_i64().unwrap();
        assert_ne!(home_id, work_id);
        assert_eq!(
            call(assign_request(db.to_str().unwrap(), entry_id, home_id))["ok"],
            true
        );
        let home_results = call(json!({"op":"search_fast","db":db,
            "query":format!("category_id:{home_id}"),"offset":0,"limit":250}));
        let work_results = call(json!({"op":"search_fast","db":db,
            "query":format!("category_id:{work_id}"),"offset":0,"limit":250}));
        assert_eq!(home_results["ok"], true, "{home_results}");
        assert_eq!(work_results["ok"], true, "{work_results}");
        assert_eq!(home_results["data"]["items"][0]["id"], entry_id);
        assert_eq!(work_results["data"]["items"].as_array().unwrap().len(), 0);

        // A selected parent includes assignments made to child categories.
        let parent = call(create_request(db.to_str().unwrap(), "Архив", None));
        let parent_id = parent["data"]["id"].as_i64().unwrap();
        let child = call(create_request(
            db.to_str().unwrap(),
            "Видео",
            Some(parent_id),
        ));
        let child_id = child["data"]["id"].as_i64().unwrap();
        assert_eq!(
            call(assign_request(db.to_str().unwrap(), entry_id, child_id))["ok"],
            true
        );
        let inherited = call(json!({"op":"search_fast","db":db,
            "query":format!("category_id:{parent_id}"),"offset":0,"limit":250}));
        assert_eq!(inherited["ok"], true, "{inherited}");
        assert_eq!(inherited["data"]["items"][0]["id"], entry_id);
        let legacy_name = call(json!({"op":"search_fast","db":db,
            "query":"category:архив","offset":0,"limit":250}));
        assert_eq!(legacy_name["ok"], true, "{legacy_name}");
        assert_eq!(legacy_name["data"]["items"][0]["id"], entry_id);
        for invalid in ["category_id:0", "category_id:-5", "category_id:unknown"] {
            let result = call(json!({"op":"search_fast","db":db,
                "query":invalid,"offset":0,"limit":250}));
            assert_eq!(result["ok"], false, "Accepted invalid {invalid}: {result}");
        }

        // Explicit undo removes the relationship, not the file, and can be
        // restored without changing the generated SQLite catalog entries.
        let unassigned = call(unassign_request(
            db.to_str().unwrap(),
            entry_id,
            category_id,
        ));
        assert_eq!(unassigned["ok"], true, "{unassigned}");
        assert_eq!(unassigned["data"]["removed"], true);
        let after = call(list_assigned_request(db.to_str().unwrap(), entry_id));
        assert_eq!(after["ok"], true, "{after}");
        assert!(!parse_assignment_ids(&after["data"])
            .unwrap()
            .contains(&category_id));
        assert_eq!(
            call(unassign_request(
                db.to_str().unwrap(),
                entry_id,
                category_id
            ))["data"]["removed"],
            false
        );
        assert_eq!(
            call(assign_request(db.to_str().unwrap(), entry_id, category_id))["ok"],
            true
        );
        let restored = call(list_assigned_request(db.to_str().unwrap(), entry_id));
        assert!(parse_assignment_ids(&restored["data"])
            .unwrap()
            .contains(&category_id));
        let file_still_present = call(json!({"op":"search_fast","db":db,
            "query":"ext:mkv","offset":0,"limit":250}));
        assert_eq!(file_still_present["data"]["items"][0]["id"], entry_id);
    }
}
