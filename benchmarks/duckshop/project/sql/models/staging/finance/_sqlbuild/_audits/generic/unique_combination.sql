AUDIT ();

WITH final AS (
  SELECT
    @columns,
    COUNT(*) AS duplicate_count
  FROM @relation
  GROUP BY
    @columns
  HAVING COUNT(*) > 1
)

SELECT *
FROM final
