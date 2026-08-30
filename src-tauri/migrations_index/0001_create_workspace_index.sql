CREATE TABLE IF NOT EXISTS workspace_tables (
  db_id TEXT NOT NULL,
  table_id TEXT NOT NULL,
  name TEXT NOT NULL,
  PRIMARY KEY (db_id, table_id)
);

CREATE TABLE IF NOT EXISTS workspace_field_links (
  db_id TEXT NOT NULL,
  field_id TEXT NOT NULL,
  target_db_id TEXT NOT NULL,
  target_table_id TEXT NOT NULL,
  target_field_id TEXT,
  cardinality TEXT NOT NULL,
  PRIMARY KEY (db_id, field_id)
);
