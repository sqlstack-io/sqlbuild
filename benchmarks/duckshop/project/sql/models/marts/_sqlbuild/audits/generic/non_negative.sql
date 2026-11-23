AUDIT ();

WITH final AS (
  SELECT *
  FROM @relation
  WHERE @column < 0
)

SELECT *
FROM final
