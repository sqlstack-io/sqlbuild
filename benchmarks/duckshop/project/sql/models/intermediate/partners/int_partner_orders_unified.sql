MODEL (
  description """
    One row per partner order across both payload versions in one shape: total in minor units and
    USD, UTC placement time, line totals checked against the header, and the buyer matched to a
    customer account by email (version 1) or by email hash (version 2).
  """,
  audits [
    match_rate (
      name "buyer_matched",
      matched "customer_id IS NOT NULL",
      minimum_samples 50,
      evidence_limit 20,
      thresholds (warn (below 40)),
    ),
  ],
  columns (
    partner_order_key (type VARCHAR, nullable false, audits [not_null, unique]),
    source_relation (type VARCHAR),
    payload_version (type INT),
    partner_code (type VARCHAR, audits [not_null]),
    external_order_id (type VARCHAR),
    currency_code (type VARCHAR),
    placed_at_utc (type TIMESTAMP_NTZ),
    placed_date (type DATE),
    order_total_minor (type INT),
    order_total_usd (type NUMBER(18, 2)),
    line_count (type INT),
    units (type INT),
    lines_total_minor (type INT),
    lines_match_total (type BOOLEAN),
    customer_id (type VARCHAR),
    is_known_customer (type BOOLEAN),
    order_rank_for_partner (type INT),
  ),
);

WITH orders_v1 AS (
  SELECT *
  FROM __ref("stg_partner_orders_v1")
),

orders_v2 AS (
  SELECT *
  FROM __ref("stg_partner_orders_v2")
),

order_lines AS (
  SELECT *
  FROM __ref("stg_partner_order_lines")
),

identities AS (
  SELECT *
  FROM __ref("int_customer_identity")
),

fx_rates AS (
  SELECT *
  FROM __ref("stg_fx_rates")
),

currencies AS (
  SELECT *
  FROM __seed("currencies")
),

headers AS (
  @py.union_by_columns(
    [__ref("stg_partner_orders_v1"), __ref("stg_partner_orders_v2")],
    ["partner_order_key", "partner_code", "external_order_id", "currency_code"],
  )
),

v1_details AS (
  SELECT
    o.partner_order_key,
    1 AS payload_version,
    CONVERT_TIMEZONE('UTC', o.placed_at)::TIMESTAMP_NTZ AS placed_at_utc,
    ROUND(o.order_total * POWER(10, c.minor_unit_exponent)) AS order_total_minor,
    o.buyer_email,
    SHA2(o.buyer_email, 256) AS buyer_email_hash
  FROM orders_v1 AS o
  LEFT JOIN currencies AS c
    ON o.currency_code = c.currency_code
),

v2_details AS (
  SELECT
    o.partner_order_key,
    2 AS payload_version,
    o.placed_at_utc,
    o.order_total_minor,
    NULL::VARCHAR AS buyer_email,
    o.buyer_email_hash
  FROM orders_v2 AS o
),

details AS (
  SELECT
    d.partner_order_key::VARCHAR AS partner_order_key,
    d.payload_version::INT AS payload_version,
    d.placed_at_utc::TIMESTAMP_NTZ AS placed_at_utc,
    d.order_total_minor::INT AS order_total_minor,
    d.buyer_email::VARCHAR AS buyer_email,
    d.buyer_email_hash::VARCHAR AS buyer_email_hash
  FROM v1_details AS d
  UNION ALL
  SELECT
    d.partner_order_key::VARCHAR AS partner_order_key,
    d.payload_version::INT AS payload_version,
    d.placed_at_utc::TIMESTAMP_NTZ AS placed_at_utc,
    d.order_total_minor::INT AS order_total_minor,
    d.buyer_email::VARCHAR AS buyer_email,
    d.buyer_email_hash::VARCHAR AS buyer_email_hash
  FROM v2_details AS d
),

line_totals AS (
  SELECT
    l.partner_order_key,
    COUNT(*) AS line_count,
    SUM(l.quantity) AS units,
    SUM(l.quantity * l.unit_amount_minor) AS lines_total_minor
  FROM order_lines AS l
  GROUP BY
    l.partner_order_key
),

identity_hashes AS (
  SELECT
    SHA2(i.email_normalised, 256) AS email_hash,
    MIN(i.customer_id) AS customer_id
  FROM identities AS i
  WHERE i.email_normalised IS NOT NULL
  GROUP BY
    SHA2(i.email_normalised, 256)
),

combined AS (
  SELECT
    h.partner_order_key,
    h.source_relation,
    d.payload_version,
    h.partner_code,
    h.external_order_id,
    h.currency_code,
    d.placed_at_utc,
    d.placed_at_utc::DATE AS placed_date,
    d.order_total_minor,
    COALESCE(t.line_count, 0) AS line_count,
    COALESCE(t.units, 0) AS units,
    COALESCE(t.lines_total_minor, 0) AS lines_total_minor,
    i.customer_id,
    c.minor_unit_exponent
  FROM headers AS h
  INNER JOIN details AS d
    ON h.partner_order_key = d.partner_order_key
  LEFT JOIN line_totals AS t
    ON h.partner_order_key = t.partner_order_key
  LEFT JOIN identity_hashes AS i
    ON d.buyer_email_hash = i.email_hash
  LEFT JOIN currencies AS c
    ON h.currency_code = c.currency_code
),

with_rate AS (
  SELECT
    c.partner_order_key,
    c.source_relation,
    c.payload_version,
    c.partner_code,
    c.external_order_id,
    c.currency_code,
    c.placed_at_utc,
    c.placed_date,
    c.order_total_minor,
    c.line_count,
    c.units,
    c.lines_total_minor,
    c.customer_id,
    c.minor_unit_exponent,
    fx.rate_to_usd
  FROM combined AS c
  ASOF JOIN fx_rates AS fx MATCH_CONDITION (c.placed_date >= fx.rate_date)
    ON c.currency_code = fx.currency_code
),

final AS (
  SELECT
    w.partner_order_key,
    w.source_relation::VARCHAR AS source_relation,
    w.payload_version,
    w.partner_code::VARCHAR AS partner_code,
    w.external_order_id::VARCHAR AS external_order_id,
    w.currency_code::VARCHAR AS currency_code,
    w.placed_at_utc,
    w.placed_date::DATE AS placed_date,
    w.order_total_minor,
    ROUND(
      @sql.money_from_minor(w.order_total_minor, COALESCE(w.minor_unit_exponent, 2), places := 6) * w.rate_to_usd,
      2
    )::NUMBER(18, 2) AS order_total_usd,
    w.line_count::INT AS line_count,
    w.units::INT AS units,
    w.lines_total_minor::INT AS lines_total_minor,
    (
      w.lines_total_minor = w.order_total_minor
    )::BOOLEAN AS lines_match_total,
    w.customer_id::VARCHAR AS customer_id,
    (
      w.customer_id IS NOT NULL
    )::BOOLEAN AS is_known_customer,
    COUNT(*) OVER (
      PARTITION BY w.partner_code
      ORDER BY w.placed_at_utc
      ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    )::INT AS order_rank_for_partner
  FROM with_rate AS w
)

SELECT *
FROM final
