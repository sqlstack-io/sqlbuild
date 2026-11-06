MODEL (
  description """
    One row per web session with its cart activity worked out from event item arrays using FILTER
    and TRANSFORM lambdas: items added (distinct SKUs in sorted order), removed, net units, and the
    checkout steps reached.
  """,
  columns (
    session_id (type VARCHAR, nullable false, audits [not_null, unique]),
    anonymous_id (type VARCHAR),
    session_start_utc (type TIMESTAMP_NTZ),
    added_skus (type ARRAY),
    removed_sku_count (type INT),
    net_units (type INT),
    max_added_unit_minor (type INT),
    reached_checkout (type BOOLEAN),
    reached_payment (type BOOLEAN),
    converted (type BOOLEAN),
    funnel_stage (type VARCHAR, audits [
      accepted_values (values ["browse", "cart", "checkout", "payment", "order"]),
    ]),
  ),
);

WITH sessions AS (
  SELECT *
  FROM __ref("int_web_sessions")
),

web_events AS (
  SELECT *
  FROM __ref("stg_web_events")
),

checkout_steps AS (
  SELECT *
  FROM __ref("stg_web_checkout_steps")
),

cart_events AS (
  SELECT
    e.anonymous_id,
    e.event_at_utc,
    e.event_type,
    FILTER(
      e.items,
      i -> i:sku IS NOT NULL
      AND COALESCE(i:quantity::INT, 1) > 0
    ) AS valid_items
  FROM web_events AS e
  WHERE e.event_type IN ('add_to_cart', 'remove_from_cart')
),

session_cart AS (
  SELECT
    s.session_id,
    ARRAY_AGG(IFF(c.event_type = 'add_to_cart', c.valid_items, NULL)) AS added_item_lists,
    SUM(IFF(c.event_type = 'remove_from_cart', ARRAY_SIZE(c.valid_items), 0)) AS removed_sku_count,
    SUM(IFF(c.event_type = 'add_to_cart', 1, -1)
    * REDUCE(
      TRANSFORM(
        c.valid_items,
        i -> COALESCE(i:quantity::INT, 1)
      ),
      0,
      (acc, q) -> acc + q
    )) AS net_units,
    MAX(ARRAY_MAX(
      TRANSFORM(c.valid_items, i -> i:price_minor::INT)
    )) AS max_added_unit_minor
  FROM sessions AS s
  INNER JOIN cart_events AS c
    ON s.anonymous_id = c.anonymous_id
    AND c.event_at_utc BETWEEN s.session_start_utc AND s.session_end_utc
  GROUP BY
    s.session_id
),

session_checkout AS (
  SELECT
    s.session_id,
    MAX(cs.step_number) AS max_step
  FROM sessions AS s
  INNER JOIN checkout_steps AS cs
    ON s.anonymous_id = cs.anonymous_id
    AND cs.first_reached_at_utc BETWEEN s.session_start_utc AND s.session_end_utc
  GROUP BY
    s.session_id
),

final AS (
  SELECT
    s.session_id,
    s.anonymous_id,
    s.session_start_utc,
    -- ARRAY_AGG and ARRAY_DISTINCT leave element order unspecified; sort for a stable result.
    ARRAY_SORT(
      ARRAY_DISTINCT(TRANSFORM(
        ARRAY_FLATTEN(COALESCE(sc.added_item_lists, ARRAY_CONSTRUCT())),
        i -> UPPER(i:sku::VARCHAR)
      ))
    )::ARRAY AS added_skus,
    COALESCE(sc.removed_sku_count, 0)::INT AS removed_sku_count,
    COALESCE(sc.net_units, 0)::INT AS net_units,
    sc.max_added_unit_minor::INT AS max_added_unit_minor,
    (
      COALESCE(ck.max_step, 0) >= 1
    ) AS reached_checkout,
    (
      COALESCE(ck.max_step, 0) >= 3
    ) AS reached_payment,
    s.has_order AS converted,
    CASE
      WHEN s.has_order
      THEN 'order'
      WHEN COALESCE(ck.max_step, 0) >= 3
      THEN 'payment'
      WHEN COALESCE(ck.max_step, 0) >= 1
      THEN 'checkout'
      WHEN s.has_add_to_cart
      THEN 'cart'
      ELSE 'browse'
    END::VARCHAR AS funnel_stage
  FROM sessions AS s
  LEFT JOIN session_cart AS sc
    ON s.session_id = sc.session_id
  LEFT JOIN session_checkout AS ck
    ON s.session_id = ck.session_id
)

SELECT *
FROM final
