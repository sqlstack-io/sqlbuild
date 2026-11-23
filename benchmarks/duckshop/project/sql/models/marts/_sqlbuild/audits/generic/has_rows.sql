AUDIT ();

WITH counted AS (
  SELECT COUNT(*) AS row_count
  FROM @relation
),

final AS (
  SELECT c.row_count
  FROM counted AS c
  WHERE c.row_count < @minimum_rows
)

SELECT *
FROM final
