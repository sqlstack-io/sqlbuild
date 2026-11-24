AUDIT ();

WITH currencies AS (
  SELECT *
  FROM __seed("currencies")
),

final AS (
  SELECT r.*
  FROM @relation AS r
  LEFT JOIN currencies AS c
    ON r.@column = c.currency_code
  WHERE
    r.@column IS NOT NULL
    AND c.currency_code IS NULL
)

SELECT *
FROM final
