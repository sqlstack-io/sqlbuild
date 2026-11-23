MODEL (
  description """
    One row per month and contact reason with ticket volume, repeat contacts and IVR abandonment
  """,
  columns (
    reason_month_key (type VARCHAR, nullable false, audits [not_null, unique]),
    contact_month (type DATE),
    contact_reason (type VARCHAR),
    tickets (type INT),
    repeat_contacts (type INT),
    repeat_rate (type NUMBER(9, 4)),
    calls (type INT),
    abandoned_in_ivr (type INT),
    month_over_month_change (type INT),
  ),
);

WITH tickets AS (
  SELECT *
  FROM __ref("fct_support_tickets")
),

calls AS (
  SELECT *
  FROM __ref("int_call_ivr_paths")
),

ticket_reasons AS (
  SELECT
    t.ticket_id,
    DATE_TRUNC('month', t.created_at_utc)::DATE AS contact_month,
    COALESCE(t.contact_reason, 'unspecified') AS contact_reason,
    t.is_repeat_contact_7d
  FROM tickets AS t
  WHERE t.created_at_utc IS NOT NULL
),

call_counts AS (
  SELECT
    c.ticket_id,
    COUNT(*) AS calls,
    COUNT_IF(c.abandoned_in_ivr) AS abandoned_in_ivr
  FROM calls AS c
  WHERE c.ticket_id IS NOT NULL
  GROUP BY
    c.ticket_id
),

monthly AS (
  SELECT
    r.contact_month,
    r.contact_reason,
    COUNT(*) AS tickets,
    COUNT_IF(r.is_repeat_contact_7d) AS repeat_contacts,
    SUM(COALESCE(c.calls, 0)) AS calls,
    SUM(COALESCE(c.abandoned_in_ivr, 0)) AS abandoned_in_ivr
  FROM ticket_reasons AS r
  LEFT JOIN call_counts AS c
    ON r.ticket_id = c.ticket_id
  GROUP BY
    r.contact_month,
    r.contact_reason
),

final AS (
  SELECT
    (
      TO_VARCHAR(m.contact_month, 'YYYY-MM') || ':' || m.contact_reason
    )::VARCHAR AS reason_month_key,
    m.contact_month,
    m.contact_reason::VARCHAR AS contact_reason,
    m.tickets::INT AS tickets,
    m.repeat_contacts::INT AS repeat_contacts,
    ROUND(m.repeat_contacts / NULLIF(m.tickets, 0), 4)::NUMBER(9, 4) AS repeat_rate,
    m.calls::INT AS calls,
    m.abandoned_in_ivr::INT AS abandoned_in_ivr,
    (
      m.tickets - LAG(m.tickets) OVER (PARTITION BY m.contact_reason ORDER BY m.contact_month)
    )::INT AS month_over_month_change
  FROM monthly AS m
)

SELECT *
FROM final
