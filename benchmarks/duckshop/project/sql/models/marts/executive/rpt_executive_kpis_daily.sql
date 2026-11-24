MODEL (
  description """
    One row per day, region and headline KPI across every business area, in long format, with the
    value a week earlier, the week-over-week change and a trailing 28-day average. Metrics without a
    natural region are reported under GLOBAL. This is deliberately the project's longest model: a
    wide union of thin metric streams, as found in executive reporting layers.
  """,
  constants (
    _global_region "GLOBAL",
    _history_days 120,
  ),
  columns (
    kpi_key (type VARCHAR, nullable false, audits [not_null, unique]),
    kpi_date (type DATE, audits [not_null]),
    region_code (type VARCHAR, audits [not_null]),
    business_area (type VARCHAR),
    metric_name (type VARCHAR, audits [not_null]),
    metric_label (type VARCHAR),
    unit (type VARCHAR),
    higher_is_better (type BOOLEAN),
    metric_value (type NUMBER(18, 4)),
    value_week_ago (type NUMBER(18, 4)),
    week_over_week_pct (type NUMBER(12, 2)),
    trailing_28d_average (type NUMBER(18, 4)),
    is_favourable_change (type BOOLEAN),
  ),
);

WITH orders AS (
  SELECT *
  FROM __ref("fct_orders")
),

refunds AS (
  SELECT *
  FROM __ref("fct_refunds")
),

web_sessions AS (
  SELECT *
  FROM __ref("fct_web_sessions")
),

tickets AS (
  SELECT *
  FROM __ref("fct_support_tickets")
),

shipments AS (
  SELECT *
  FROM __ref("fct_shipments")
),

warehouses AS (
  SELECT *
  FROM __ref("dim_warehouses")
),

campaign_spend AS (
  SELECT *
  FROM __ref("fct_campaign_spend")
),

inventory AS (
  SELECT *
  FROM __ref("fct_inventory_daily")
),

recognition AS (
  SELECT *
  FROM __ref("int_revenue_recognition_daily")
),

partner_orders AS (
  SELECT *
  FROM __ref("fct_partner_orders")
),

chargebacks AS (
  SELECT *
  FROM __ref("int_chargeback_exposure")
),

calendar AS (
  SELECT *
  FROM __ref("dim_date")
),

reporting_window AS (
  SELECT c.calendar_date
  FROM calendar AS c
  WHERE
    c.calendar_date <= '@@valuation_date'::DATE
    AND c.calendar_date > DATEADD(
      'day',
      -@const("_history_days"),
      '@@valuation_date'::DATE
    )
),

completed_orders AS (
  SELECT
    o.placed_date_local AS kpi_date,
    COALESCE(o.region_code, @const("_global_region")) AS region_code,
    o.amount_usd,
    o.is_first_order
  FROM orders AS o
  WHERE o.order_status NOT IN ('CANCELLED', 'PENDING')
),

order_metrics AS (
  SELECT
    c.kpi_date,
    c.region_code,
    COUNT(*) AS order_count,
    SUM(c.amount_usd) AS revenue_usd,
    AVG(c.amount_usd) AS average_order_usd,
    COUNT_IF(c.is_first_order) AS new_customers
  FROM completed_orders AS c
  GROUP BY
    c.kpi_date,
    c.region_code
),

order_regions AS (
  SELECT
    o.order_id,
    COALESCE(o.region_code, @const("_global_region")) AS region_code
  FROM orders AS o
),

refund_metrics AS (
  SELECT
    r.refunded_date AS kpi_date,
    COALESCE(o.region_code, @const("_global_region")) AS region_code,
    SUM(r.amount_usd) AS refund_usd,
    COUNT(*) AS refund_count
  FROM refunds AS r
  LEFT JOIN order_regions AS o
    ON r.order_id = o.order_id
  GROUP BY
    1,
    2
),

session_metrics AS (
  SELECT
    s.session_start_utc::DATE AS kpi_date,
    COUNT(*) AS sessions,
    AVG(IFF(s.converted, 1, 0)) AS conversion_rate,
    AVG(IFF(s.is_bounce, 1, 0)) AS bounce_rate
  FROM web_sessions AS s
  GROUP BY
    1
),

ticket_metrics AS (
  SELECT
    t.created_at_utc::DATE AS kpi_date,
    COUNT(*) AS tickets,
    AVG(IFF(t.is_first_response_breached, 1, 0)) AS first_response_breach_rate,
    AVG(t.csat_score) AS average_csat
  FROM tickets AS t
  WHERE t.created_at_utc IS NOT NULL
  GROUP BY
    1
),

warehouse_regions AS (
  SELECT
    w.warehouse_code,
    COALESCE(w.region_code, @const("_global_region")) AS region_code
  FROM warehouses AS w
),

shipment_metrics AS (
  SELECT
    CONVERT_TIMEZONE('UTC', s.shipped_at)::DATE AS kpi_date,
    COALESCE(w.region_code, @const("_global_region")) AS region_code,
    COUNT(*) AS shipments,
    AVG(IFF(s.is_dispatch_on_time, 1, 0)) AS on_time_dispatch_rate,
    AVG(s.hours_to_deliver) AS average_hours_to_deliver
  FROM shipments AS s
  LEFT JOIN warehouse_regions AS w
    ON s.warehouse_code = w.warehouse_code
  WHERE s.shipped_at IS NOT NULL
  GROUP BY
    1,
    2
),

spend_metrics AS (
  SELECT
    s.spend_date AS kpi_date,
    SUM(s.spend_usd) AS spend_usd,
    SUM(s.clicks) AS paid_clicks
  FROM campaign_spend AS s
  GROUP BY
    1
),

stock_metrics AS (
  SELECT
    i.calendar_date AS kpi_date,
    COALESCE(w.region_code, @const("_global_region")) AS region_code,
    COUNT_IF(i.is_stocked_out) AS stocked_out_sku_days,
    SUM(i.stock_value_usd) AS stock_value_usd
  FROM inventory AS i
  LEFT JOIN warehouse_regions AS w
    ON i.warehouse_code = w.warehouse_code
  GROUP BY
    1,
    2
),

subscription_metrics AS (
  SELECT
    r.recognition_date AS kpi_date,
    r.region_code,
    SUM(r.revenue_usd) AS subscription_revenue_usd
  FROM recognition AS r
  WHERE r.revenue_stream = 'subscription'
  GROUP BY
    1,
    2
),

partner_metrics AS (
  SELECT
    p.placed_date AS kpi_date,
    COUNT(*) AS partner_orders,
    SUM(p.order_total_usd) AS partner_revenue_usd
  FROM partner_orders AS p
  GROUP BY
    1
),

chargeback_metrics AS (
  SELECT
    CONVERT_TIMEZONE('UTC', c.opened_at)::DATE AS kpi_date,
    COUNT(*) AS chargebacks_opened,
    SUM(c.amount_usd) AS chargeback_usd
  FROM chargebacks AS c
  GROUP BY
    1
),

all_metrics AS (
  SELECT
    m.kpi_date::DATE AS kpi_date,
    m.region_code::VARCHAR AS region_code,
    'orders'::VARCHAR AS metric_name,
    m.order_count::NUMBER(18, 4) AS metric_value
  FROM order_metrics AS m
  UNION ALL
  SELECT
    m.kpi_date::DATE AS kpi_date,
    m.region_code::VARCHAR AS region_code,
    'revenue_usd'::VARCHAR AS metric_name,
    m.revenue_usd::NUMBER(18, 4) AS metric_value
  FROM order_metrics AS m
  UNION ALL
  SELECT
    m.kpi_date::DATE AS kpi_date,
    m.region_code::VARCHAR AS region_code,
    'average_order_usd'::VARCHAR AS metric_name,
    m.average_order_usd::NUMBER(18, 4) AS metric_value
  FROM order_metrics AS m
  UNION ALL
  SELECT
    m.kpi_date::DATE AS kpi_date,
    m.region_code::VARCHAR AS region_code,
    'new_customers'::VARCHAR AS metric_name,
    m.new_customers::NUMBER(18, 4) AS metric_value
  FROM order_metrics AS m
  UNION ALL
  SELECT
    r.kpi_date::DATE AS kpi_date,
    r.region_code::VARCHAR AS region_code,
    'refund_usd'::VARCHAR AS metric_name,
    r.refund_usd::NUMBER(18, 4) AS metric_value
  FROM refund_metrics AS r
  UNION ALL
  SELECT
    r.kpi_date::DATE AS kpi_date,
    r.region_code::VARCHAR AS region_code,
    'refunds'::VARCHAR AS metric_name,
    r.refund_count::NUMBER(18, 4) AS metric_value
  FROM refund_metrics AS r
  UNION ALL
  SELECT
    s.kpi_date::DATE AS kpi_date,
    @const("_global_region")::VARCHAR AS region_code,
    'web_sessions'::VARCHAR AS metric_name,
    s.sessions::NUMBER(18, 4) AS metric_value
  FROM session_metrics AS s
  UNION ALL
  SELECT
    s.kpi_date::DATE AS kpi_date,
    @const("_global_region")::VARCHAR AS region_code,
    'web_conversion_rate'::VARCHAR AS metric_name,
    s.conversion_rate::NUMBER(18, 4) AS metric_value
  FROM session_metrics AS s
  UNION ALL
  SELECT
    s.kpi_date::DATE AS kpi_date,
    @const("_global_region")::VARCHAR AS region_code,
    'web_bounce_rate'::VARCHAR AS metric_name,
    s.bounce_rate::NUMBER(18, 4) AS metric_value
  FROM session_metrics AS s
  UNION ALL
  SELECT
    t.kpi_date::DATE AS kpi_date,
    @const("_global_region")::VARCHAR AS region_code,
    'support_tickets'::VARCHAR AS metric_name,
    t.tickets::NUMBER(18, 4) AS metric_value
  FROM ticket_metrics AS t
  UNION ALL
  SELECT
    t.kpi_date::DATE AS kpi_date,
    @const("_global_region")::VARCHAR AS region_code,
    'first_response_breach_rate'::VARCHAR AS metric_name,
    t.first_response_breach_rate::NUMBER(18, 4) AS metric_value
  FROM ticket_metrics AS t
  UNION ALL
  SELECT
    t.kpi_date::DATE AS kpi_date,
    @const("_global_region")::VARCHAR AS region_code,
    'average_csat'::VARCHAR AS metric_name,
    t.average_csat::NUMBER(18, 4) AS metric_value
  FROM ticket_metrics AS t
  UNION ALL
  SELECT
    s.kpi_date::DATE AS kpi_date,
    s.region_code::VARCHAR AS region_code,
    'shipments'::VARCHAR AS metric_name,
    s.shipments::NUMBER(18, 4) AS metric_value
  FROM shipment_metrics AS s
  UNION ALL
  SELECT
    s.kpi_date::DATE AS kpi_date,
    s.region_code::VARCHAR AS region_code,
    'on_time_dispatch_rate'::VARCHAR AS metric_name,
    s.on_time_dispatch_rate::NUMBER(18, 4) AS metric_value
  FROM shipment_metrics AS s
  UNION ALL
  SELECT
    s.kpi_date::DATE AS kpi_date,
    s.region_code::VARCHAR AS region_code,
    'average_hours_to_deliver'::VARCHAR AS metric_name,
    s.average_hours_to_deliver::NUMBER(18, 4) AS metric_value
  FROM shipment_metrics AS s
  UNION ALL
  SELECT
    p.kpi_date::DATE AS kpi_date,
    @const("_global_region")::VARCHAR AS region_code,
    'marketing_spend_usd'::VARCHAR AS metric_name,
    p.spend_usd::NUMBER(18, 4) AS metric_value
  FROM spend_metrics AS p
  UNION ALL
  SELECT
    p.kpi_date::DATE AS kpi_date,
    @const("_global_region")::VARCHAR AS region_code,
    'paid_clicks'::VARCHAR AS metric_name,
    p.paid_clicks::NUMBER(18, 4) AS metric_value
  FROM spend_metrics AS p
  UNION ALL
  SELECT
    k.kpi_date::DATE AS kpi_date,
    k.region_code::VARCHAR AS region_code,
    'stocked_out_sku_days'::VARCHAR AS metric_name,
    k.stocked_out_sku_days::NUMBER(18, 4) AS metric_value
  FROM stock_metrics AS k
  UNION ALL
  SELECT
    k.kpi_date::DATE AS kpi_date,
    k.region_code::VARCHAR AS region_code,
    'stock_value_usd'::VARCHAR AS metric_name,
    k.stock_value_usd::NUMBER(18, 4) AS metric_value
  FROM stock_metrics AS k
  UNION ALL
  SELECT
    b.kpi_date::DATE AS kpi_date,
    b.region_code::VARCHAR AS region_code,
    'subscription_revenue_usd'::VARCHAR AS metric_name,
    b.subscription_revenue_usd::NUMBER(18, 4) AS metric_value
  FROM subscription_metrics AS b
  UNION ALL
  SELECT
    p.kpi_date::DATE AS kpi_date,
    @const("_global_region")::VARCHAR AS region_code,
    'partner_orders'::VARCHAR AS metric_name,
    p.partner_orders::NUMBER(18, 4) AS metric_value
  FROM partner_metrics AS p
  UNION ALL
  SELECT
    p.kpi_date::DATE AS kpi_date,
    @const("_global_region")::VARCHAR AS region_code,
    'partner_revenue_usd'::VARCHAR AS metric_name,
    p.partner_revenue_usd::NUMBER(18, 4) AS metric_value
  FROM partner_metrics AS p
  UNION ALL
  SELECT
    c.kpi_date::DATE AS kpi_date,
    @const("_global_region")::VARCHAR AS region_code,
    'chargebacks_opened'::VARCHAR AS metric_name,
    c.chargebacks_opened::NUMBER(18, 4) AS metric_value
  FROM chargeback_metrics AS c
  UNION ALL
  SELECT
    c.kpi_date::DATE AS kpi_date,
    @const("_global_region")::VARCHAR AS region_code,
    'chargeback_usd'::VARCHAR AS metric_name,
    c.chargeback_usd::NUMBER(18, 4) AS metric_value
  FROM chargeback_metrics AS c
),

metric_catalogue AS (
  SELECT
    v.metric_name,
    v.business_area,
    v.metric_label,
    v.unit,
    v.higher_is_better
  FROM VALUES
    ('orders', 'orders', 'Orders', 'count', TRUE),
    ('revenue_usd', 'orders', 'Revenue', 'usd', TRUE),
    ('average_order_usd', 'orders', 'Average order value', 'usd', TRUE),
    ('new_customers', 'customers', 'New customers', 'count', TRUE),
    ('refund_usd', 'orders', 'Refunded', 'usd', FALSE),
    ('refunds', 'orders', 'Refunds', 'count', FALSE),
    ('web_sessions', 'clickstream', 'Web sessions', 'count', TRUE),
    ('web_conversion_rate', 'clickstream', 'Web conversion rate', 'ratio', TRUE),
    ('web_bounce_rate', 'clickstream', 'Web bounce rate', 'ratio', FALSE),
    ('support_tickets', 'support', 'Support tickets', 'count', FALSE),
    ('first_response_breach_rate', 'support', 'First response SLA breaches', 'ratio', FALSE),
    ('average_csat', 'support', 'Average CSAT', 'score', TRUE),
    ('shipments', 'fulfilment', 'Shipments dispatched', 'count', TRUE),
    ('on_time_dispatch_rate', 'fulfilment', 'On-time dispatch', 'ratio', TRUE),
    ('average_hours_to_deliver', 'fulfilment', 'Hours to deliver', 'hours', FALSE),
    ('marketing_spend_usd', 'growth', 'Marketing spend', 'usd', FALSE),
    ('paid_clicks', 'growth', 'Paid clicks', 'count', TRUE),
    ('stocked_out_sku_days', 'inventory', 'Stocked-out SKU days', 'count', FALSE),
    ('stock_value_usd', 'inventory', 'Stock value', 'usd', FALSE),
    ('subscription_revenue_usd', 'finance', 'Subscription revenue', 'usd', TRUE),
    ('partner_orders', 'partners', 'Partner orders', 'count', TRUE),
    ('partner_revenue_usd', 'partners', 'Partner revenue', 'usd', TRUE),
    ('chargebacks_opened', 'finance', 'Chargebacks opened', 'count', FALSE),
    ('chargeback_usd', 'finance', 'Chargeback amount', 'usd', FALSE) AS v(metric_name, business_area, metric_label, unit, higher_is_better)
),

metric_regions AS (
  SELECT DISTINCT
    a.region_code,
    a.metric_name
  FROM all_metrics AS a
),

dense_grid AS (
  SELECT
    w.calendar_date AS kpi_date,
    r.region_code,
    r.metric_name
  FROM reporting_window AS w
  CROSS JOIN metric_regions AS r
),

filled AS (
  SELECT
    g.kpi_date,
    g.region_code,
    g.metric_name,
    COALESCE(a.metric_value, 0) AS metric_value
  FROM dense_grid AS g
  LEFT JOIN all_metrics AS a
    ON g.kpi_date = a.kpi_date
    AND g.region_code = a.region_code
    AND g.metric_name = a.metric_name
),

compared AS (
  SELECT
    f.kpi_date,
    f.region_code,
    f.metric_name,
    f.metric_value,
    SUM(f.metric_value) OVER (
      PARTITION BY f.region_code, f.metric_name
      ORDER BY f.kpi_date
      RANGE BETWEEN INTERVAL '7 days' PRECEDING AND INTERVAL '7 days' PRECEDING
    ) AS value_week_ago,
    AVG(f.metric_value) OVER (
      PARTITION BY f.region_code, f.metric_name
      ORDER BY f.kpi_date
      RANGE BETWEEN INTERVAL '27 days' PRECEDING AND CURRENT ROW
    ) AS trailing_28d_average
  FROM filled AS f
),

final AS (
  SELECT
    (
      TO_VARCHAR(c.kpi_date, 'YYYY-MM-DD') || ':' || c.region_code || ':' || c.metric_name
    )::VARCHAR AS kpi_key,
    c.kpi_date::DATE AS kpi_date,
    c.region_code::VARCHAR AS region_code,
    m.business_area::VARCHAR AS business_area,
    c.metric_name::VARCHAR AS metric_name,
    m.metric_label::VARCHAR AS metric_label,
    m.unit::VARCHAR AS unit,
    m.higher_is_better::BOOLEAN AS higher_is_better,
    c.metric_value::NUMBER(18, 4) AS metric_value,
    c.value_week_ago::NUMBER(18, 4) AS value_week_ago,
    ROUND(100
    * (
      c.metric_value / NULLIF(c.value_week_ago, 0) - 1
    ), 2)::NUMBER(12, 2) AS week_over_week_pct,
    ROUND(c.trailing_28d_average, 4)::NUMBER(18, 4) AS trailing_28d_average,
    IFF(
      m.higher_is_better,
      c.metric_value >= c.value_week_ago,
      c.metric_value <= c.value_week_ago
    )::BOOLEAN AS is_favourable_change
  FROM compared AS c
  LEFT JOIN metric_catalogue AS m
    ON c.metric_name = m.metric_name
)

SELECT *
FROM final
