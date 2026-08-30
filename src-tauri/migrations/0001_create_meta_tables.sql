CREATE TABLE IF NOT EXISTS _tables (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  source_db_id TEXT,
  created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
  updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

CREATE TABLE IF NOT EXISTS _fields (
  id TEXT PRIMARY KEY,
  table_id TEXT NOT NULL,
  name TEXT NOT NULL,
  "type" TEXT NOT NULL,
  config TEXT NOT NULL DEFAULT '{}',
  position INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
  FOREIGN KEY (table_id) REFERENCES _tables(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS _relations (
  id TEXT PRIMARY KEY,
  source_table_id TEXT NOT NULL,
  source_field_id TEXT NOT NULL,
  target_db_id TEXT NOT NULL,
  target_table_id TEXT NOT NULL,
  cardinality TEXT NOT NULL,
  cascade_delete INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);

CREATE TABLE IF NOT EXISTS _views (
  id TEXT PRIMARY KEY,
  table_id TEXT NOT NULL,
  name TEXT NOT NULL,
  "type" TEXT NOT NULL,
  config TEXT NOT NULL DEFAULT '{}',
  is_default INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY (table_id) REFERENCES _tables(id) ON DELETE CASCADE
);
