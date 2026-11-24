AUDIT ();

-- Append-only feeds are delivered at least once, so an id may arrive again in a later load. Within
-- one load (one `_loaded_at`) each id must still appear once; staging keeps the latest delivery.
WITH final AS (
  SELECT
    @column,
    _loaded_at,
    COUNT(*) AS delivery_count
  FROM @relation
  WHERE @column IS NOT NULL
  GROUP BY
    @column,
    _loaded_at
  HAVING COUNT(*) > 1
)

SELECT *
FROM final
