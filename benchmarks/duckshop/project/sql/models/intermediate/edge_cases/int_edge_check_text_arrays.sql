MODEL (
  description """
    One row per reconciliation check with the number of parts of its key text, split into an
    array cast through the structured type ARRAY(VARCHAR)
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    key_part_count (type NUMBER(38, 0)),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    ARRAY_SIZE(CAST(CAST(SPLIT(a.key_text, ',') AS ARRAY(VARCHAR)) AS ARRAY))::NUMBER(38, 0) AS key_part_count
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
