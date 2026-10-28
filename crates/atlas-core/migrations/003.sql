-- Fresh schema v3. Existing catalogs are NEVER migrated on open.
CREATE TABLE volumes(id INTEGER PRIMARY KEY,label TEXT NOT NULL,root_path TEXT NOT NULL,scanned_at INTEGER NOT NULL,entry_count INTEGER NOT NULL,total_bytes INTEGER NOT NULL,status TEXT NOT NULL);
CREATE TABLE entries_core(
 id INTEGER PRIMARY KEY,
 volume_id INTEGER NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
 parent_id INTEGER REFERENCES entries_core(id),
 name TEXT NOT NULL,relative_path TEXT NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN ('file','directory','symlink','archive')),
 size INTEGER NOT NULL CHECK(size>=0),modified_unix INTEGER,
 name_norm TEXT NOT NULL,ext TEXT NOT NULL
);
CREATE TABLE entry_payload(
 entry_id INTEGER PRIMARY KEY REFERENCES entries_core(id) ON DELETE CASCADE,
 description TEXT NOT NULL DEFAULT '',tags TEXT NOT NULL DEFAULT '',
 CHECK(description<>'' OR tags<>'')
);
-- Compatibility projection; the concatenated search document is no longer stored
-- once per entry. Payload is materialized only when its columns are requested.
CREATE VIEW entries AS SELECT c.id,c.volume_id,c.parent_id,c.name,c.relative_path,
 c.kind,c.size,c.modified_unix,COALESCE(p.description,'') AS description,
 COALESCE(p.tags,'') AS tags,
 unicode_lower(c.name||char(10)||c.relative_path||char(10)||COALESCE(p.description,'')||char(10)||COALESCE(p.tags,'')) AS search_text,
 c.name_norm,c.ext
 FROM entries_core c LEFT JOIN entry_payload p ON p.entry_id=c.id;
CREATE INDEX entries_parent ON entries_core(volume_id,parent_id,name,id);
CREATE INDEX entries_ext ON entries_core(ext);
CREATE INDEX entries_size ON entries_core(size);
CREATE INDEX entries_kind_size ON entries_core(kind,size);
CREATE INDEX entries_name_sort ON entries_core(name COLLATE NOCASE,id);
CREATE INDEX entries_name_size ON entries_core(kind,name_norm,size);
CREATE TABLE categories(id INTEGER PRIMARY KEY,parent_id INTEGER REFERENCES categories(id),name TEXT NOT NULL,name_norm TEXT NOT NULL);
CREATE INDEX categories_name ON categories(name_norm);
CREATE TABLE entry_categories(entry_id INTEGER NOT NULL REFERENCES entries_core(id) ON DELETE CASCADE,category_id INTEGER NOT NULL REFERENCES categories(id) ON DELETE CASCADE,PRIMARY KEY(entry_id,category_id));
CREATE INDEX entry_categories_category ON entry_categories(category_id,entry_id);
CREATE VIRTUAL TABLE entries_trigram USING fts5(search_text,content='entries',content_rowid='id',tokenize='trigram',detail=none);
CREATE TRIGGER core_ai AFTER INSERT ON entries_core BEGIN
 INSERT INTO entries_trigram(rowid,search_text) SELECT id,search_text FROM entries WHERE id=new.id;
END;
CREATE TRIGGER core_bd BEFORE DELETE ON entries_core BEGIN
 INSERT INTO entries_trigram(entries_trigram,rowid,search_text) SELECT 'delete',id,search_text FROM entries WHERE id=old.id;
END;
CREATE TRIGGER core_au AFTER UPDATE OF name,relative_path ON entries_core
 WHEN old.name IS NOT new.name OR old.relative_path IS NOT new.relative_path BEGIN
 INSERT INTO entries_trigram(entries_trigram,rowid,search_text)
 SELECT 'delete',old.id,unicode_lower(old.name||char(10)||old.relative_path||char(10)||COALESCE(p.description,'')||char(10)||COALESCE(p.tags,''))
 FROM entries_core c LEFT JOIN entry_payload p ON p.entry_id=c.id WHERE c.id=new.id;
 INSERT INTO entries_trigram(rowid,search_text) SELECT id,search_text FROM entries WHERE id=new.id;
END;
CREATE TRIGGER payload_ai AFTER INSERT ON entry_payload BEGIN
 INSERT INTO entries_trigram(entries_trigram,rowid,search_text)
 SELECT 'delete',id,unicode_lower(name||char(10)||relative_path||char(10)||char(10)) FROM entries_core WHERE id=new.entry_id;
 INSERT INTO entries_trigram(rowid,search_text) SELECT id,search_text FROM entries WHERE id=new.entry_id;
END;
CREATE TRIGGER payload_au AFTER UPDATE ON entry_payload BEGIN
 INSERT INTO entries_trigram(entries_trigram,rowid,search_text)
 SELECT 'delete',id,unicode_lower(name||char(10)||relative_path||char(10)||old.description||char(10)||old.tags) FROM entries_core WHERE id=old.entry_id;
 INSERT INTO entries_trigram(rowid,search_text) SELECT id,search_text FROM entries WHERE id=new.entry_id;
END;
CREATE TRIGGER payload_ad AFTER DELETE ON entry_payload
 WHEN EXISTS(SELECT 1 FROM entries_core WHERE id=old.entry_id) BEGIN
 INSERT INTO entries_trigram(entries_trigram,rowid,search_text)
 SELECT 'delete',id,unicode_lower(name||char(10)||relative_path||char(10)||old.description||char(10)||old.tags) FROM entries_core WHERE id=old.entry_id;
 INSERT INTO entries_trigram(rowid,search_text) SELECT id,search_text FROM entries WHERE id=old.entry_id;
END;
CREATE TRIGGER entries_update INSTEAD OF UPDATE ON entries BEGIN
 SELECT CASE WHEN new.id IS NOT old.id THEN RAISE(ABORT,'Entry IDs are immutable') END;
 UPDATE entries_core SET volume_id=new.volume_id,parent_id=new.parent_id,name=new.name,relative_path=new.relative_path,kind=new.kind,size=new.size,modified_unix=new.modified_unix,name_norm=new.name_norm,ext=new.ext WHERE id=old.id;
 INSERT INTO entry_payload(entry_id,description,tags)
 SELECT new.id,new.description,new.tags WHERE new.description<>'' OR new.tags<>''
 ON CONFLICT(entry_id) DO UPDATE SET description=excluded.description,tags=excluded.tags;
 DELETE FROM entry_payload WHERE entry_id=new.id AND new.description='' AND new.tags='';
END;
CREATE TRIGGER entries_delete INSTEAD OF DELETE ON entries BEGIN
 DELETE FROM entries_core WHERE id=old.id;
END;
PRAGMA user_version=3;
