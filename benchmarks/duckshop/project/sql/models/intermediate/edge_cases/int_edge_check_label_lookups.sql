MODEL (
  description """
    One row per reconciliation check with a label looked up in an object by the first part of
    its key text
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    key_label (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    OBJECT_CONSTRUCT('7', 'seven', '101', 'one hundred and one')[SPLIT(a.key_text, ',')[0]]::VARCHAR AS key_label
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
