MODEL (
  description """
    One row per SKU price window, with open ends closed by sentinel timestamps at microsecond
    precision so the window always has two bounds
  """,
  tags [edge_case],
  columns (
    window_id (type VARCHAR, nullable false, audits [not_null, unique]),
    sku (type VARCHAR),
    price_minor (type NUMBER(38, 0)),
    valid_from (type TIMESTAMP_NTZ(6)),
    valid_to (type TIMESTAMP_NTZ(6)),
    is_open_ended (type BOOLEAN),
    window_days (type INT),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("edge__price_windows")
),

final AS (
  SELECT
    s.window_id,
    s.sku,
    s.price_minor,
    COALESCE(s.valid_from, '0001-01-01 00:00:00'::TIMESTAMP_NTZ(6)) AS valid_from,
    COALESCE(s.valid_to, '9999-12-31 23:59:59.999999'::TIMESTAMP_NTZ(6)) AS valid_to,
    (
      s.valid_to IS NULL
      OR s.valid_to >= '9999-01-01'::TIMESTAMP_NTZ(6)
    ) AS is_open_ended,
    DATEDIFF(
      'day',
      COALESCE(s.valid_from, '0001-01-01 00:00:00'::TIMESTAMP_NTZ(6)),
      COALESCE(s.valid_to, '9999-12-31 23:59:59.999999'::TIMESTAMP_NTZ(6))
    )::INT AS window_days
  FROM src AS s
)

SELECT *
FROM final
