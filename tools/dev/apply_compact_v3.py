#!/usr/bin/env python3
"""One-time source transformation, not a user-database migration.

Executed only by the branch-restricted development workflow. The generated
Rust changes are formatted, tested, and committed to that branch, never main.
No private catalog is required or accessed.
"""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
changes = {}


def load(path):
    return (ROOT / path).read_text(encoding="utf-8")


def replace(text, old, new, count=1):
    actual = text.count(old)
    if actual != count:
        raise RuntimeError(f"Source anchor mismatch: expected {count}, got {actual}: {old[:90]}")
    return text.replace(old, new)


def transform():
    core_path = "crates/atlas-core/src/lib.rs"
    s = load(core_path)
    if "pub fn query_table(" in s:
        print("compact-v3 source transformation already applied")
        return
    s = replace(s, "if v > 1 {", "if v > 3 {")
    s = replace(s, "} else if current > 1 {", "} else if current > 3 {")
    s = replace(s, 'include_str!("../migrations/001.sql")', 'include_str!("../migrations/003.sql")')
    s = replace(s, '"init" => Ok(json!({"schema_version":1})),', '"init" => Ok(json!({"schema_version":c.conn().pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0))?})),')
    marker = "    pub fn volumes(&self) -> Result<Vec<Volume>> {"
    helper = '''    /// Hot predicates select the indexed core directly. Annotations are joined
    /// only for the final display page, or when requested by the predicate.
    pub fn query_table(&self, query: &str) -> Result<&'static str> {
        let version: i64 = self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        Ok(if version >= 3 && !query::needs_payload(query)? { "entries_core" } else { "entries" })
    }
'''
    s = replace(s, marker, helper + marker)
    s = replace(s, "let (filter, mut args) = query::compile(query)?;", "let (filter, mut args) = query::compile(query)?;\n        let table = self.query_table(query)?;", count=2)
    s = replace(s, 'let total = self.conn.query_row(\n            "SELECT COUNT(*) FROM entries WHERE volume_id=?1 AND parent_id IS ?2",', 'let table = self.query_table("")?;\n        let total = self.conn.query_row(\n            &format!("SELECT COUNT(*) FROM {table} WHERE volume_id=?1 AND parent_id IS ?2"),')
    s = replace(s, 'format!("SELECT COUNT(*) FROM entries WHERE {filter}")', 'format!("SELECT COUNT(*) FROM {table} WHERE {filter}")')
    fields = "e.id,e.volume_id,e.parent_id,e.name,e.relative_path,e.kind,e.size,e.modified_unix,e.description,e.tags"
    lines = s.splitlines()
    children = search = fast = 0
    for i, line in enumerate(lines):
        if 'let mut s=self.conn.prepare("SELECT id,volume_id,parent_id,name,relative_path,kind,size,modified_unix,description,tags FROM entries WHERE volume_id=?1' in line:
            lines[i] = '        let mut s=self.conn.prepare(&format!("WITH page AS MATERIALIZED (SELECT id FROM {table} WHERE volume_id=?1 AND parent_id IS ?2 ORDER BY CASE kind WHEN \'directory\' THEN 0 ELSE 1 END,name COLLATE NOCASE,id LIMIT ?3 OFFSET ?4) SELECT ' + fields + ' FROM page p JOIN entries e ON e.id=p.id ORDER BY CASE e.kind WHEN \'directory\' THEN 0 ELSE 1 END,e.name COLLATE NOCASE,e.id"))?;'
            children += 1
        elif 'let sql=format!("SELECT id,volume_id,parent_id,name,relative_path,kind,size,modified_unix,description,tags FROM entries WHERE {filter}' in line:
            lines[i] = '        let sql=format!("WITH page AS MATERIALIZED (SELECT id FROM {table} WHERE {filter} ORDER BY name COLLATE NOCASE,id LIMIT ? OFFSET ?) SELECT ' + fields + ' FROM page p JOIN entries e ON e.id=p.id ORDER BY e.name COLLATE NOCASE,e.id");'
            search += 1
        elif 'let sql=format!("SELECT id,volume_id,parent_id,name,relative_path,kind,size,modified_unix,description,tags FROM entries WHERE {}' in line:
            lines[i] = '        let sql=format!("WITH page AS MATERIALIZED (SELECT id FROM {table} WHERE {} ORDER BY name COLLATE NOCASE,id LIMIT ? OFFSET ?) SELECT ' + fields + ' FROM page p JOIN entries e ON e.id=p.id ORDER BY e.name COLLATE NOCASE,e.id",clauses.join(" AND "));'
            fast += 1
    if (children, search, fast) != (1, 1, 1):
        raise RuntimeError("Page-query source shape changed")
    s = "\n".join(lines) + "\n"
    s = replace(s, 'let sql = format!("SELECT id FROM entries WHERE {filter}");', 'let table = self.query_table(q)?;\n        let sql = format!("SELECT id FROM {table} WHERE {filter}");')
    s = replace(s, '''            volume_id: id,
            count: 0,
            bytes: 0,
''', '''            volume_id: id,
            count: 0,
            bytes: 0,
            sparse: tx.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))? >= 3,
''')
    s = replace(s, '''                volume_id: id,
                count: 0,
                bytes: 0,
''', '''                volume_id: id,
                count: 0,
                bytes: 0,
                sparse: self.tx.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))? >= 3,
''')
    s = replace(s, '''    pub volume_id: i64,
    count: u64,
    bytes: u64,
}''', '''    pub volume_id: i64,
    count: u64,
    bytes: u64,
    sparse: bool,
}''')
    matches = [line for line in s.splitlines() if 'self.tx.prepare_cached("INSERT INTO entries(volume_id' in line]
    if len(matches) != 1:
        raise RuntimeError("Entry-insert anchor mismatch")
    old = matches[0]
    insert = '''        let entry_id = if self.sparse {
            self.tx.prepare_cached("INSERT INTO entries_core(volume_id,parent_id,name,relative_path,kind,size,modified_unix,name_norm,ext) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)")?.execute(params![self.volume_id,e.parent_id,e.name,e.relative_path,e.kind,size,e.modified_unix,e.name.to_lowercase(),Path::new(&e.name).extension().and_then(|x|x.to_str()).unwrap_or("").to_lowercase()])?;
            let entry_id = self.tx.last_insert_rowid();
            if !e.description.is_empty() || !e.tags.is_empty() {
                self.tx.prepare_cached("INSERT INTO entry_payload(entry_id,description,tags) VALUES(?1,?2,?3)")?.execute(params![entry_id,e.description,e.tags])?;
            }
            entry_id
        } else {
''' + old + '''
            self.tx.last_insert_rowid()
        };'''
    s = replace(s, old, insert)
    s = replace(s, "        Ok(self.tx.last_insert_rowid())\n    }\n}\nfn search_text", "        Ok(entry_id)\n    }\n}\nfn search_text")
    changes[core_path] = s

    path = "crates/atlas-core/src/query.rs"
    s = load(path)
    s += '''
/// This is a physical-plan hint, never a semantic rewrite. Unknown fields are
/// still rejected by compile(). Unqualified/free-text terms include payload.
pub fn needs_payload(input: &str) -> Result<bool> {
    Ok(lex(input)?.iter().any(|token| match token {
        Token::Word(word) => match word.split_once(':') {
            None => true,
            Some((field, _)) => matches!(field.to_ascii_lowercase().as_str(), "text" | "description" | "tag"),
        },
        _ => false,
    }))
}
'''
    changes[path] = s

    path = "crates/atlas-service/src/legacy.rs"
    s = load(path)
    helper = '''
/// Raw CTF records are dense provenance, not sparse optional annotations.
/// Keep v1 and experimental v2 compatible; v3 refers to the real core table.
pub(crate) fn ensure_legacy_schema(c: &Catalog) -> Result<bool> {
    let version: i64 = c.conn().pragma_query_value(None, "user_version", |r| r.get(0))?;
    let mut schema = LEGACY_SCHEMA.to_owned();
    if version >= 3 {
        schema = schema.replace("REFERENCES entries(id)", "REFERENCES entries_core(id)");
    }
    if version == 2 {
        schema = schema.replace(",opaque BLOB NOT NULL", "");
        schema.push_str("CREATE TABLE IF NOT EXISTS ctf_entry_payload(entry_id INTEGER PRIMARY KEY REFERENCES ctf_entries(entry_id) ON DELETE CASCADE,opaque BLOB NOT NULL CHECK(length(opaque)>0));");
    }
    c.conn().execute_batch(&schema)?;
    Ok(version != 2)
}
'''
    s = replace(s, "fn fail(e: impl std::fmt::Display) -> Error {", helper + "fn fail(e: impl std::fmt::Display) -> Error {")
    s = replace(s, "c.conn().execute_batch(LEGACY_SCHEMA)?;", "let dense_opaque = ensure_legacy_schema(c)?;", count=2)
    matches = [line for line in s.splitlines() if 'prepare_cached("INSERT INTO ctf_entries(entry_id,attributes,modified_dos,created_dos,accessed_dos,opaque)' in line]
    if len(matches) != 1:
        raise RuntimeError("CTF insertion anchor changed")
    old = matches[0]
    s = replace(s, old, '''    if dense_opaque {
''' + old + '''
    } else {
      session.transaction().prepare_cached("INSERT INTO ctf_entries(entry_id,attributes,modified_dos,created_dos,accessed_dos) VALUES(?1,?2,?3,?4,?5)")?.execute(params![entry_id,e.attributes,e.modified_dos,e.created_dos,e.accessed_dos])?;
      if !e.opaque.is_empty() {
        session.transaction().prepare_cached("INSERT INTO ctf_entry_payload(entry_id,opaque) VALUES(?1,?2)")?.execute(params![entry_id,e.opaque])?;
      }
    }''')
    matches = [line for line in s.splitlines() if 'let mut stmt=snapshot.prepare("SELECT e.id,e.parent_id' in line]
    if len(matches) != 1:
        raise RuntimeError("CTF export anchor changed")
    old = matches[0]
    sql = old.split('snapshot.prepare("', 1)[1].rsplit('")?;', 1)[0]
    sparse_sql = sql.replace("l.opaque FROM", "p.opaque FROM").replace(" WHERE e.volume_id", " LEFT JOIN ctf_entry_payload p ON p.entry_id=e.id WHERE e.volume_id")
    s = replace(s, old, '        let sql = if dense_opaque { "' + sql + '" } else { "' + sparse_sql + '" };\n        let mut stmt = snapshot.prepare(sql)?;')
    changes[path] = s

    path = "crates/atlas-service/src/duplicates.rs"
    s = load(path)
    s = replace(s, '    c.conn().execute_batch(r#"', '    let mut schema = r#"')
    s = replace(s, '"#)?;\n    Ok(())', '''"#.to_owned();
    if c.query_table("")? == "entries_core" {
        schema = schema.replace(" ON entries BEGIN", " ON entries_core BEGIN");
        let payload_triggers = "CREATE TRIGGER IF NOT EXISTS atlas_rev_payload_insert AFTER INSERT ON entry_payload BEGIN UPDATE atlas_meta SET revision=revision+1 WHERE id=1; END; CREATE TRIGGER IF NOT EXISTS atlas_rev_payload_update AFTER UPDATE ON entry_payload BEGIN UPDATE atlas_meta SET revision=revision+1 WHERE id=1; END; CREATE TRIGGER IF NOT EXISTS atlas_rev_payload_delete AFTER DELETE ON entry_payload BEGIN UPDATE atlas_meta SET revision=revision+1 WHERE id=1; END; ";
        schema = schema.replace("CREATE TRIGGER IF NOT EXISTS atlas_rev_link_update", &format!("{payload_triggers}CREATE TRIGGER IF NOT EXISTS atlas_rev_link_update"));
    }
    c.conn().execute_batch(&schema)?;
    Ok(())''')
    anchor = "    let req = request(v, force)?;\n    ensure_schema(c)?;"
    s = replace(s, anchor, '    let req = request(v, force)?;\n    let table = c.query_table(v.get("query").and_then(Value::as_str).unwrap_or(""))?;\n    ensure_schema(c)?;')
    s = replace(s, "FROM entries WHERE {} GROUP BY", "FROM {table} WHERE {} GROUP BY")
    s = replace(s, "FROM entries e JOIN (SELECT id AS group_id", "FROM {table} e JOIN (SELECT id AS group_id")
    changes[path] = s

    for path, text in changes.items():
        (ROOT / path).write_text(text, encoding="utf-8")
    print("Transformed source files:", ", ".join(changes))


if __name__ == "__main__":
    transform()
