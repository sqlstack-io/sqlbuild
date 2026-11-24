SCHEMA (
  name audit_ts,
  description "When the source row behind a dimension record was last loaded",
  columns (
    source_loaded_at (type TIMESTAMP_NTZ, audits [not_null]),
  ),
);
