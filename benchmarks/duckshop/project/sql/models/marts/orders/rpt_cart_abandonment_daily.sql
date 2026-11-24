MODEL (
  description """
    One row per storefront and day of last cart activity with abandonment and recovery rates
  """,
  columns (
    cart_day_key (type VARCHAR, nullable false, audits [not_null, unique]),
    activity_date (type DATE),
    storefront_code (type VARCHAR),
    carts (type INT),
    abandoned_carts (type INT),
    converted_carts (type INT),
    abandonment_rate (type NUMBER(9, 4)),
    abandoned_value_minor (type INT),
    median_hours_to_convert (type NUMBER(10, 2)),
  ),
);

WITH carts AS (
  SELECT *
  FROM __ref("int_cart_abandonment")
),

daily AS (
  SELECT
    CONVERT_TIMEZONE('UTC', c.last_activity_at)::DATE AS activity_date,
    c.storefront_code,
    COUNT(*) AS carts,
    COUNT_IF(c.is_abandoned) AS abandoned_carts,
    COUNT_IF(c.is_converted) AS converted_carts,
    SUM(IFF(c.is_abandoned, c.cart_value_minor, 0)) AS abandoned_value_minor,
    MEDIAN(c.hours_to_convert) AS median_hours_to_convert
  FROM carts AS c
  GROUP BY
    1,
    2
),

final AS (
  SELECT
    (
      d.storefront_code || ':' || TO_VARCHAR(d.activity_date, 'YYYY-MM-DD')
    )::VARCHAR AS cart_day_key,
    d.activity_date::DATE AS activity_date,
    d.storefront_code,
    d.carts::INT AS carts,
    d.abandoned_carts::INT AS abandoned_carts,
    d.converted_carts::INT AS converted_carts,
    ROUND(d.abandoned_carts / NULLIF(d.carts, 0), 4)::NUMBER(9, 4) AS abandonment_rate,
    d.abandoned_value_minor::INT AS abandoned_value_minor,
    ROUND(d.median_hours_to_convert, 2)::NUMBER(10, 2) AS median_hours_to_convert
  FROM daily AS d
)

SELECT *
FROM final
