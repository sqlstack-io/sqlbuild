MODEL (
  description """
    One row per add-to-cart or remove-from-cart item on web, flattened from the event items
  """,
  columns (
    cart_event_key (type VARCHAR, nullable false, audits [not_null, unique]),
    event_id (type VARCHAR, audits [not_null]),
    anonymous_id (type VARCHAR),
    event_at_utc (type TIMESTAMP_NTZ),
    action (type VARCHAR, audits [accepted_values (values ["add", "remove"])]),
    sku (type VARCHAR),
    quantity_delta (type INT),
    unit_amount_minor (type INT),
    currency_code (type VARCHAR),
  ),
);

WITH web_events AS (
  SELECT *
  FROM __ref("stg_web_events")
),

cart_events AS (
  SELECT
    e.event_id,
    e.anonymous_id,
    e.event_at_utc,
    IFF(e.event_type = 'add_to_cart', 'add', 'remove') AS action,
    UPPER(e.properties:currency::VARCHAR) AS currency_code,
    e.items
  FROM web_events AS e
  WHERE e.event_type IN ('add_to_cart', 'remove_from_cart')
),

final AS (
  SELECT
    (
      c.event_id || ':' || i.index::VARCHAR
    )::VARCHAR AS cart_event_key,
    c.event_id,
    c.anonymous_id,
    c.event_at_utc,
    c.action::VARCHAR AS action,
    @sql.normalise_sku(i.value:sku::VARCHAR) AS sku,
    (
      IFF(c.action = 'add', 1, -1) * COALESCE(i.value:quantity::INT, 1)
    )::INT AS quantity_delta,
    i.value:price_minor::INT AS unit_amount_minor,
    c.currency_code::VARCHAR AS currency_code
  FROM cart_events AS c, LATERAL FLATTEN(input => c.items) AS i
)

SELECT *
FROM final
