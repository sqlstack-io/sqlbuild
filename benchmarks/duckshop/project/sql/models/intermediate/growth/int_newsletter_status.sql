MODEL (
  description """
    One row per subscriber email with the latest status on each newsletter list, the lists the
    address currently receives, and whether the address belongs to a customer account or is a
    prospect who has never ordered.
  """,
  columns (
    email (type VARCHAR, nullable false, audits [not_null, unique]),
    email_domain (type VARCHAR),
    is_role_address (type BOOLEAN),
    customer_id (type VARCHAR),
    is_prospect (type BOOLEAN),
    list_status_by_list (type OBJECT),
    subscribed_lists (type ARRAY),
    subscribed_list_count (type INT),
    first_subscribed_at (type TIMESTAMP_TZ),
    last_unsubscribed_at (type TIMESTAMP_TZ),
    is_globally_unsubscribed (type BOOLEAN),
    has_double_opt_in (type BOOLEAN),
    first_signup_form (type VARCHAR),
  ),
);

WITH subscriptions AS (
  SELECT *
  FROM __ref("stg_newsletter_subscriptions")
),

customers AS (
  SELECT *
  FROM __ref("stg_customers")
),

latest_per_list AS (
  SELECT
    s.email,
    s.list_code,
    s.list_status,
    s.subscribed_at,
    s.unsubscribed_at,
    s.is_double_opt_in
  FROM subscriptions AS s
  WHERE s.email IS NOT NULL
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY s.email, s.list_code
      ORDER BY COALESCE(
        s.unsubscribed_at,
        s.subscribed_at
      ) DESC NULLS LAST, s.list_subscription_id DESC
    ) = 1
),

first_signup AS (
  SELECT
    s.email,
    s.signup_form AS first_signup_form
  FROM subscriptions AS s
  WHERE s.email IS NOT NULL
  QUALIFY
    ROW_NUMBER() OVER (
      PARTITION BY s.email
      ORDER BY s.subscribed_at NULLS LAST, s.list_subscription_id
    ) = 1
),

by_email AS (
  SELECT
    l.email,
    OBJECT_AGG(l.list_code, l.list_status::VARIANT) AS list_status_by_list,
    ARRAY_AGG(
      IFF(l.list_status = 'subscribed', l.list_code, NULL)
    ) WITHIN GROUP (ORDER BY l.list_code) AS subscribed_lists,
    COUNT_IF(l.list_status = 'subscribed') AS subscribed_list_count,
    MIN(l.subscribed_at) AS first_subscribed_at,
    MAX(l.unsubscribed_at) AS last_unsubscribed_at,
    BOOLAND_AGG(l.list_status IN ('unsubscribed', 'cleaned')) AS is_globally_unsubscribed,
    BOOLOR_AGG(COALESCE(l.is_double_opt_in, FALSE)) AS has_double_opt_in
  FROM latest_per_list AS l
  GROUP BY
    l.email
),

customer_emails AS (
  SELECT
    c.email_normalised AS email,
    MIN(c.customer_id) AS customer_id
  FROM customers AS c
  WHERE c.email_normalised IS NOT NULL
  GROUP BY
    c.email_normalised
),

prospect_emails AS (
  SELECT b.email::VARCHAR AS email
  FROM by_email AS b
  MINUS
  SELECT c.email::VARCHAR AS email
  FROM customer_emails AS c
),

final AS (
  SELECT
    b.email,
    SPLIT_PART(b.email, '@', 2)::VARCHAR AS email_domain,
    REGEXP_LIKE(
      b.email,
      '^(info|admin|sales|support|hello|contact|noreply|no-reply)@.*'
    )::BOOLEAN AS is_role_address,
    c.customer_id,
    (
      p.email IS NOT NULL
    )::BOOLEAN AS is_prospect,
    b.list_status_by_list::OBJECT AS list_status_by_list,
    b.subscribed_lists::ARRAY AS subscribed_lists,
    b.subscribed_list_count::INT AS subscribed_list_count,
    b.first_subscribed_at,
    b.last_unsubscribed_at,
    b.is_globally_unsubscribed::BOOLEAN AS is_globally_unsubscribed,
    b.has_double_opt_in::BOOLEAN AS has_double_opt_in,
    f.first_signup_form::VARCHAR AS first_signup_form
  FROM by_email AS b
  LEFT JOIN customer_emails AS c
    ON b.email = c.email
  LEFT JOIN prospect_emails AS p
    ON b.email = p.email
  LEFT JOIN first_signup AS f
    ON b.email = f.email
)

SELECT *
FROM final
