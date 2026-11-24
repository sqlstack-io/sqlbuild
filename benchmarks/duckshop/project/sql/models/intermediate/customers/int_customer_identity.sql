MODEL (
  description """
    One row per customer account with its identity keys across channels: a HASH-based dedupe key on
    the normalised email (HASH output is engine-specific, an expected divergence across engines),
    linked anonymous web visitors, newsletter and support footprints, and the most similar other
    account sharing a phone number.
  """,
  tags [known_divergence],
  audits [
    match_rate (
      name "email_resolved",
      matched "email_normalised IS NOT NULL",
      minimum_samples 50,
      evidence_limit 20,
      thresholds (warn (below 90), error (below 50)),
    ),
  ],
  columns (
    customer_id (type VARCHAR, nullable false, audits [not_null, unique]),
    email_normalised (type VARCHAR),
    email_domain (type VARCHAR),
    email_dedupe_key (type NUMBER(38, 0)),
    phone_normalised (type VARCHAR),
    full_name (type VARCHAR),
    anonymous_ids (type ARRAY),
    anonymous_id_count (type INT),
    newsletter_lists (type ARRAY),
    ticket_count (type INT),
    duplicate_candidate_id (type VARCHAR),
    duplicate_name_similarity (type INT),
    accounts_sharing_email (type INT),
  ),
);

WITH customers AS (
  SELECT *
  FROM __ref("stg_customers")
),

web_identities AS (
  SELECT *
  FROM __ref("stg_web_identifies")
),

newsletter AS (
  SELECT *
  FROM __ref("stg_newsletter_subscriptions")
),

tickets AS (
  SELECT *
  FROM __ref("stg_tickets")
),

base AS (
  SELECT
    c.customer_id,
    c.email_normalised,
    SPLIT_PART(c.email_normalised, '@', 2) AS email_domain,
    HASH(c.email_normalised) AS email_dedupe_key,
    c.phone_normalised,
    TRIM(CONCAT_WS(' ', c.first_name, c.last_name)) AS full_name
  FROM customers AS c
),

anonymous_links AS (
  SELECT
    w.customer_id,
    ARRAY_AGG(DISTINCT w.anonymous_id) WITHIN GROUP (ORDER BY w.anonymous_id) AS anonymous_ids
  FROM web_identities AS w
  GROUP BY
    w.customer_id
),

newsletter_links AS (
  SELECT
    b.customer_id,
    ARRAY_AGG(DISTINCT n.list_code) WITHIN GROUP (ORDER BY n.list_code) AS newsletter_lists
  FROM base AS b
  INNER JOIN newsletter AS n
    ON b.email_normalised = n.email
  WHERE n.list_status = 'subscribed'
  GROUP BY
    b.customer_id
),

ticket_links AS (
  SELECT
    b.customer_id,
    COUNT(DISTINCT t.ticket_id) AS ticket_count
  FROM base AS b
  INNER JOIN tickets AS t
    ON b.email_normalised = t.customer_email
  GROUP BY
    b.customer_id
),

shared_phone_pairs AS (
  SELECT
    a.customer_id,
    b.customer_id AS other_customer_id,
    JAROWINKLER_SIMILARITY(LOWER(a.full_name), LOWER(b.full_name)) AS name_similarity
  FROM base AS a
  INNER JOIN base AS b
    ON a.phone_normalised = b.phone_normalised
    AND a.customer_id <> b.customer_id
),

best_duplicate AS (
  SELECT
    p.customer_id,
    MAX_BY(p.other_customer_id, p.name_similarity) AS other_customer_id,
    MAX(p.name_similarity) AS name_similarity
  FROM shared_phone_pairs AS p
  WHERE p.name_similarity >= 85
  GROUP BY
    p.customer_id
),

email_collisions AS (
  SELECT
    b.email_dedupe_key,
    COUNT(*) AS accounts_sharing_email
  FROM base AS b
  WHERE b.email_normalised IS NOT NULL
  GROUP BY
    b.email_dedupe_key
),

final AS (
  SELECT
    b.customer_id,
    b.email_normalised,
    b.email_domain::VARCHAR AS email_domain,
    b.email_dedupe_key::NUMBER(38, 0) AS email_dedupe_key,
    b.phone_normalised,
    b.full_name::VARCHAR AS full_name,
    COALESCE(a.anonymous_ids, ARRAY_CONSTRUCT())::ARRAY AS anonymous_ids,
    ARRAY_SIZE(COALESCE(a.anonymous_ids, ARRAY_CONSTRUCT()))::INT AS anonymous_id_count,
    COALESCE(n.newsletter_lists, ARRAY_CONSTRUCT())::ARRAY AS newsletter_lists,
    COALESCE(t.ticket_count, 0)::INT AS ticket_count,
    d.other_customer_id::VARCHAR AS duplicate_candidate_id,
    d.name_similarity::INT AS duplicate_name_similarity,
    COALESCE(e.accounts_sharing_email, 1)::INT AS accounts_sharing_email
  FROM base AS b
  LEFT JOIN anonymous_links AS a
    ON b.customer_id = a.customer_id
  LEFT JOIN newsletter_links AS n
    ON b.customer_id = n.customer_id
  LEFT JOIN ticket_links AS t
    ON b.customer_id = t.customer_id
  LEFT JOIN best_duplicate AS d
    ON b.customer_id = d.customer_id
  LEFT JOIN email_collisions AS e
    ON b.email_dedupe_key = e.email_dedupe_key
)

SELECT *
FROM final
