MODEL (
  description """
    One row per reconciliation check with whether its FLOAT ratio hashes like itself read back
    from text
  """,
  tags [edge_case],
  columns (
    check_id (type VARCHAR, nullable false, audits [not_null, unique]),
    hash_is_stable (type BOOLEAN),
  ),
);

WITH final AS (
  SELECT
    a.check_id,
    (HASH(a.ratio) = HASH(a.ratio::VARCHAR::FLOAT))::BOOLEAN AS hash_is_stable
  FROM __ref("stg_edge_amount_checks") AS a
)

SELECT *
FROM final
