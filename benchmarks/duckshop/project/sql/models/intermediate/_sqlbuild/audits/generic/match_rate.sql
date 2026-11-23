AUDIT (
  evaluation measurement,
  value match_rate,
  sample_count total_rows,
  sample_unit rows,
);

MEASURE (
  WITH final AS (
  SELECT
    COUNT(*) AS total_rows,
    100.0 * AVG(IFF(@matched, 1, 0)) AS match_rate
  FROM @relation
)

SELECT *
FROM final
);

EVIDENCE (
  WITH final AS (
  SELECT *
  FROM @relation
  WHERE NOT COALESCE(@matched, FALSE)
)

SELECT *
FROM final
);
