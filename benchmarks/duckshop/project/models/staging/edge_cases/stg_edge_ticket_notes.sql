MODEL (
  description """
    One row per support ticket note with order references and reasons found by regular expressions,
    written with escaped and dollar-quoted patterns, a SKU normalised by a macro after an escaped
    quote, campaign and discount codes matched by pattern lists and escapes, and SKUs split at edge
    positions; the note text and tags are passed on as written
  """,
  tags [edge_case],
  columns (
    note_id (type VARCHAR, nullable false, audits [not_null, unique]),
    first_order_ref (type VARCHAR),
    first_order_ref_dollar (type VARCHAR),
    second_order_number (type VARCHAR),
    mentions_order_rlike (type BOOLEAN),
    mentions_order_regexp_like (type BOOLEAN),
    order_ref_count (type INT),
    refund_word (type VARCHAR),
    body_collapsed (type VARCHAR),
    quotes_customer (type BOOLEAN),
    sku_canonical (type VARCHAR),
    second_hash_tag (type VARCHAR),
    is_newsletter_visit (type BOOLEAN),
    is_customer_code (type BOOLEAN),
    is_fifty_percent_code (type BOOLEAN),
    sku_part_zero (type VARCHAR),
    sku_part_last (type VARCHAR),
    sku_part_beyond (type VARCHAR),
    sku_part_no_delimiter (type VARCHAR),
    sku_from_zero (type VARCHAR),
    sku_last_two (type VARCHAR),
    sku_before_start (type VARCHAR),
    body (type VARCHAR),
    tags_text (type VARCHAR),
  ),
);

WITH src AS (
  SELECT *
  FROM __source("edge__ticket_notes")
),

final AS (
  SELECT
    s.note_id,
    REGEXP_SUBSTR(s.body, 'DS-O-\\d+')::VARCHAR AS first_order_ref,
    REGEXP_SUBSTR(s.body, $$DS-O-\d+$$)::VARCHAR AS first_order_ref_dollar,
    REGEXP_SUBSTR(s.body, 'DS-O-(\\d+)', 1, 2, 'e', 1)::VARCHAR AS second_order_number,
    (
      s.body RLIKE 'DS-O-\\d+'
    )::BOOLEAN AS mentions_order_rlike,
    REGEXP_LIKE(s.body, '.*ds-o-\\d+.*', 'is') AS mentions_order_regexp_like,
    REGEXP_COUNT(s.body, 'DS-O-\\d+')::INT AS order_ref_count,
    REGEXP_SUBSTR(s.body, 'refund(ed)?', 1, 1, 'i')::VARCHAR AS refund_word,
    REGEXP_REPLACE(s.body, '\\s+', ' ')::VARCHAR AS body_collapsed,
    // A backslash-escaped quote inside a single-quoted literal, then a macro call to expand
    CONTAINS(s.body, 'O\'Brien')::BOOLEAN AS quotes_customer,
    @sql.normalise_sku(s.sku) AS sku_canonical,
    SPLIT_PART(s.tags_text, '#', 2)::VARCHAR AS second_hash_tag,
    (
      s.utm_source ILIKE ANY ('%mail%', 'news%')
    ) AS is_newsletter_visit,
    (
      NOT s.discount_code LIKE ANY ('TEST%', 'STAFF%')
    ) AS is_customer_code,
    s.discount_code LIKE '50!%%' ESCAPE '!' AS is_fifty_percent_code,
    SPLIT_PART(s.sku, '-', 0)::VARCHAR AS sku_part_zero,
    SPLIT_PART(s.sku, '-', -1)::VARCHAR AS sku_part_last,
    SPLIT_PART(s.sku, '-', 5)::VARCHAR AS sku_part_beyond,
    SPLIT_PART(s.sku, '', 1)::VARCHAR AS sku_part_no_delimiter,
    SUBSTR(s.sku, 0, 3)::VARCHAR AS sku_from_zero,
    SUBSTR(s.sku, -2)::VARCHAR AS sku_last_two,
    SUBSTR(s.sku, -10, 3)::VARCHAR AS sku_before_start,
    s.body,
    s.tags_text
  FROM src AS s
)

SELECT *
FROM final
