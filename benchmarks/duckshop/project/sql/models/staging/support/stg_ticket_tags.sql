MODEL (
  description """
    One row per distinct tag on a ticket, from the tag array and the comma-separated labels custom
    field
  """,
  columns (
    ticket_tag_key (type VARCHAR, nullable false, audits [not_null, unique]),
    ticket_id (type VARCHAR, audits [not_null]),
    tag (type VARCHAR, audits [not_null]),
  ),
);

WITH tickets AS (
  SELECT *
  FROM __ref("stg_tickets")
),

array_tags AS (
  SELECT
    t.ticket_id,
    f.value::VARCHAR AS raw_tag
  FROM tickets AS t, LATERAL FLATTEN(input => t.tag_list) AS f
  WHERE f.value IS NOT NULL
),

label_tags AS (
  SELECT
    t.ticket_id,
    s.value::VARCHAR AS raw_tag
  FROM tickets AS t, LATERAL FLATTEN(input => SPLIT(
    t.custom_fields:labels::VARCHAR,
    ','
  )) AS s
  WHERE t.custom_fields:labels IS NOT NULL
),

combined AS (
  SELECT
    a.ticket_id,
    LOWER(REGEXP_REPLACE(TRIM(a.raw_tag), '[\\s_]+', '-')) AS tag
  FROM array_tags AS a
  UNION DISTINCT
  SELECT
    l.ticket_id,
    LOWER(REGEXP_REPLACE(TRIM(l.raw_tag), '[\\s_]+', '-')) AS tag
  FROM label_tags AS l
),

final AS (
  SELECT
    (
      c.ticket_id || ':' || c.tag
    )::VARCHAR AS ticket_tag_key,
    c.ticket_id,
    c.tag::VARCHAR AS tag
  FROM combined AS c
  WHERE c.tag <> ''
)

SELECT *
FROM final
