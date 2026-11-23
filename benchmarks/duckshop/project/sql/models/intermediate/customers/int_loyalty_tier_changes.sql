MODEL (
  description """
    One row per loyalty tier change, flattened from each account's tier history with direction and
    tenure
  """,
  columns (
    tier_change_key (type VARCHAR, nullable false, audits [not_null, unique]),
    loyalty_account_id (type VARCHAR, audits [not_null]),
    customer_id (type VARCHAR),
    change_sequence (type INT),
    tier_code (type VARCHAR),
    previous_tier_code (type VARCHAR),
    changed_at (type TIMESTAMP_TZ),
    direction (type VARCHAR, audits [
      accepted_values (values ["enrol", "upgrade", "downgrade", "same"]),
    ]),
    days_in_previous_tier (type INT),
  ),
);

WITH accounts AS (
  SELECT *
  FROM __ref("stg_loyalty_accounts")
),

history AS (
  SELECT
    a.loyalty_account_id,
    a.customer_id,
    h.index::INT AS change_sequence,
    UPPER(h.value:tier::VARCHAR) AS tier_code,
    TRY_TO_TIMESTAMP_TZ(h.value:at::VARCHAR) AS changed_at
  FROM accounts AS a, LATERAL FLATTEN(input => a.tier_history) AS h
),

with_previous AS (
  SELECT
    h.loyalty_account_id,
    h.customer_id,
    h.change_sequence,
    h.tier_code,
    h.changed_at,
    LAG(h.tier_code) OVER (
      PARTITION BY h.loyalty_account_id
      ORDER BY h.change_sequence
    ) AS previous_tier_code,
    LAG(h.changed_at) OVER (
      PARTITION BY h.loyalty_account_id
      ORDER BY h.change_sequence
    ) AS previous_changed_at
  FROM history AS h
),

final AS (
  SELECT
    (
      w.loyalty_account_id || ':' || w.change_sequence::VARCHAR
    )::VARCHAR AS tier_change_key,
    w.loyalty_account_id,
    w.customer_id,
    w.change_sequence,
    w.tier_code::VARCHAR AS tier_code,
    w.previous_tier_code::VARCHAR AS previous_tier_code,
    w.changed_at,
    CASE
      WHEN w.previous_tier_code IS NULL
      THEN 'enrol'
      WHEN DECODE(
        w.tier_code,
        'DUCKLING', 1,
        'DRAKE', 2,
        'GOLDEN_DUCK', 3,
        'PLATINUM_DUCK', 4,
        0
      ) > DECODE(
        w.previous_tier_code,
        'DUCKLING', 1,
        'DRAKE', 2,
        'GOLDEN_DUCK', 3,
        'PLATINUM_DUCK', 4,
        0
      )
      THEN 'upgrade'
      WHEN w.tier_code = w.previous_tier_code
      THEN 'same'
      ELSE 'downgrade'
    END::VARCHAR AS direction,
    DATEDIFF('day', w.previous_changed_at, w.changed_at)::INT AS days_in_previous_tier
  FROM with_previous AS w
)

SELECT *
FROM final
