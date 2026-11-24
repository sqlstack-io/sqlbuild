MACRO (
  description "Whether a region code is one of a compile-time list of regions",
  arguments (region VARCHAR, regions LIST VARCHAR),
  returns BOOLEAN,
);

COALESCE(@region IN @regions, FALSE)
