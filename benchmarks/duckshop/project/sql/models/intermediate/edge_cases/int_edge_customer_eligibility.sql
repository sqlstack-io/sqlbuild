MODEL (
  description """
    One row per customer with whether a blocklist excludes them, checked with NOT IN against a
    blocklist that may hold a missing customer id, and with NOT EXISTS
  """,
  tags [edge_case],
  columns (
    customer_id (type VARCHAR, nullable false, audits [not_null, unique]),
    passes_not_in (type BOOLEAN),
    passes_not_exists (type BOOLEAN),
  ),
);

WITH profiles AS (
  SELECT p.customer_id
  FROM __ref("stg_edge_customer_profiles") AS p
),

blocked AS (
  SELECT s.customer_id
  FROM __source("edge__customer_profiles") AS s
  WHERE s.loyalty_tier = 'BLOCKED'
),

passes_not_in AS (
  SELECT p.customer_id
  FROM profiles AS p
  WHERE
    p.customer_id NOT IN (
      SELECT b.customer_id
      FROM blocked AS b
    )
),

passes_not_exists AS (
  SELECT p.customer_id
  FROM profiles AS p
  WHERE
    NOT EXISTS(
      SELECT 1
      FROM blocked AS b
      WHERE b.customer_id = p.customer_id
    )
),

final AS (
  SELECT
    p.customer_id,
    (
      i.customer_id IS NOT NULL
    ) AS passes_not_in,
    (
      e.customer_id IS NOT NULL
    ) AS passes_not_exists
  FROM profiles AS p
  LEFT JOIN passes_not_in AS i
    ON p.customer_id = i.customer_id
  LEFT JOIN passes_not_exists AS e
    ON p.customer_id = e.customer_id
)

SELECT *
FROM final
