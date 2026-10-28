-- v2: new SQLite catalogs store optional CTF unknown bytes in a sparse table.
-- v1 catalogs remain unchanged; opening a user catalog never migrates it.
PRAGMA user_version=2;
