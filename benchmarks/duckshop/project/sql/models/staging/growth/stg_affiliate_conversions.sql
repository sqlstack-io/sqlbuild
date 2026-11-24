MODEL (
  description "One row per live affiliate conversion with the click it is attributed to",
  columns (
    conversion_id (type VARCHAR, nullable false, audits [not_null, unique]),
    affiliate_code (type VARCHAR, audits [not_null]),
    order_id (type VARCHAR),
    commission_minor (type INT),
    currency_code (type VARCHAR),
    converted_at (type TIMESTAMP_TZ),
    clicked_at (type TIMESTAMP_TZ),
    click_to_conversion_hours (type INT),
    sub_id (type VARCHAR),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("growth__affiliate_conversions")
),

@sql.cdc_latest(src, src.key := (conversion_id))

clicks AS (
  SELECT
    c.conversion_id,
    TRY_TO_TIMESTAMP_TZ(c.click_payload:clicked_at::VARCHAR) AS clicked_at
  FROM cdc_latest AS c
), final AS (
  SELECT
    c.conversion_id,
    UPPER(c.affiliate_code)::VARCHAR AS affiliate_code,
    c.order_id,
    c.commission_minor::INT AS commission_minor,
    UPPER(c.currency_code)::VARCHAR AS currency_code,
    c.converted_at,
    k.clicked_at,
    DATEDIFF('hour', k.clicked_at, c.converted_at)::INT AS click_to_conversion_hours,
    REGEXP_SUBSTR(
      c.click_payload:landing_url::VARCHAR,
      '[?&]subid=([^&]+)',
      1,
      1,
      'e',
      1
    )::VARCHAR AS sub_id,
    @sql.load_metadata(c)
  FROM cdc_latest AS c
  INNER JOIN clicks AS k
    ON c.conversion_id = k.conversion_id
)
SELECT *
FROM final
