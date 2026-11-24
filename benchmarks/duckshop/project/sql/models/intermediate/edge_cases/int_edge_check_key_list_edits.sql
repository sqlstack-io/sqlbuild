MODEL (
  description """
    One row per reconciliation check with its key parts less a fixed part, with its narrow key
    appended, and an object of its narrow key with the ratio inserted
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    kept_part_count (type NUMBER(38, 0)),
    appended_part_count (type NUMBER(38, 0)),
    check_json (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    ARRAY_SIZE(ARRAY_EXCEPT(SPLIT(a.key_text, ','), ['7']))::NUMBER(38, 0) AS kept_part_count,
    ARRAY_SIZE(ARRAY_APPEND(SPLIT(a.key_text, ','), a.narrow_key))::NUMBER(38, 0) AS appended_part_count,
    TO_JSON(OBJECT_INSERT(OBJECT_CONSTRUCT('k', a.narrow_key), 'r', a.ratio)) AS check_json
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
