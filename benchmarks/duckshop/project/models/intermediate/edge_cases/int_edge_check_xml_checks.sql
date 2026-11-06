MODEL (
  description """
    One row per reconciliation check with its key text stripped of its group code read as a
    pattern, and CHECK_XML of the key text
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    stripped_key (type VARCHAR),
    xml_error (type VARCHAR),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    REGEXP_REPLACE(a.key_text, a.group_code, '') AS stripped_key,
    CHECK_XML(a.key_text)::VARCHAR AS xml_error
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
