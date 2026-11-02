MODEL (
  description """
    One row per reconciliation check with the parts of its key text, split on commas, read as
    integers through a typed lambda
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    key_parts (type ARRAY),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    TRANSFORM(STRTOK_TO_ARRAY(a.key_text, ','), part VARCHAR -> TRY_TO_NUMBER(part))::ARRAY AS key_parts
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
