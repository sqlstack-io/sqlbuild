MODEL (
  description """
    One row per reconciliation check with numbers of three or more digits in its key text
    replaced by its group's parts
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    rewritten_key (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    REGEXP_REPLACE(
      a.key_text,
      '\\d{3,}(,\\d{3,})*',
      ARRAY_TO_STRING(SPLIT(a.group_code, '-'), ',')
    ) AS rewritten_key
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
