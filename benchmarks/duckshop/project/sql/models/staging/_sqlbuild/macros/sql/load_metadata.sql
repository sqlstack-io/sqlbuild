MACRO (
  description "Standard lineage columns carried from a change-log row",
  arguments (src RELATION (_seq NUMBER(38, 0), _loaded_at TIMESTAMP_NTZ)),
);

SELECT
  @src._seq AS source_seq,
  TO_TIMESTAMP_NTZ(@src._loaded_at) AS source_loaded_at
