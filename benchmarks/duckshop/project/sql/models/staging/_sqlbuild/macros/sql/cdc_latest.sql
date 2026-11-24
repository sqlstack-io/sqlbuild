MACRO (
  description "Latest surviving version of each key in an append-only change log",
  arguments (src RELATION (_op VARCHAR, _seq NUMBER(38, 0), _loaded_at TIMESTAMP_NTZ, key COLUMNS)),
);

WITH cdc_versions AS (
  SELECT *
  FROM @src
  QUALIFY ROW_NUMBER() OVER (PARTITION BY @src.key ORDER BY _seq DESC) = 1
),

cdc_latest AS (
  SELECT *
  FROM cdc_versions
  WHERE _op <> @enum("cdc_op").DELETE
)
