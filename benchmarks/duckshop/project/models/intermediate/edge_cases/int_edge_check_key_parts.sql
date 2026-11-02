MODEL (
  description """
    One row per reconciliation check with the number of parts of its key text, split on commas
    and read as numbers through a typed lambda
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    key_number_count (type NUMBER(38, 0)),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    ARRAY_SIZE(TRANSFORM(STRTOK_TO_ARRAY(a.key_text, ','), part VARCHAR -> TRY_TO_NUMBER(part)))::NUMBER(38, 0) AS key_number_count
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
