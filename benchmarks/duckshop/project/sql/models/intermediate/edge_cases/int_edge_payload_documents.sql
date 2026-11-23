MODEL (
  description """
    One row per partner payload message with the payload written back as text for change detection,
    the colour field read as a JSON null, a missing key or a value, and merchant-defined attribute
    names read with literal keys
  """,
  tags [edge_case],
  columns (
    message_id (type VARCHAR, nullable false, audits [not_null, unique]),
    payload_json (type VARCHAR),
    payload_text_cast (type VARCHAR),
    payload_md5 (type VARCHAR),
    text_payload_json (type VARCHAR),
    text_payload_kind (type VARCHAR),
    colour_is_json_null (type BOOLEAN),
    colour_kind (type VARCHAR),
    colour_text (type VARCHAR),
    colour_json (type VARCHAR),
    colour_is_missing (type BOOLEAN),
    colour_or_default (type VARCHAR),
    size_cm (type VARCHAR),
    dotted_attribute (type VARCHAR),
    discount_attribute (type VARCHAR),
    empty_attribute (type VARCHAR),
    star_attribute (type VARCHAR),
    path_like_attribute (type VARCHAR),
    sku_any_case (type VARCHAR),
    tag_list (type VARCHAR),
  ),
);

WITH payloads AS (
  SELECT
    s.message_id,
    s.payload,
    s.text_payload
  FROM __ref("stg_edge_partner_payloads") AS s
),

final AS (
  SELECT
    p.message_id,
    TO_JSON(p.payload)::VARCHAR AS payload_json,
    p.payload::VARCHAR AS payload_text_cast,
    MD5(TO_VARCHAR(p.payload))::VARCHAR AS payload_md5,
    TO_JSON(p.text_payload)::VARCHAR AS text_payload_json,
    TYPEOF(p.text_payload)::VARCHAR AS text_payload_kind,
    IS_NULL_VALUE(p.payload:colour)::BOOLEAN AS colour_is_json_null,
    TYPEOF(p.payload:colour)::VARCHAR AS colour_kind,
    p.payload:colour::VARCHAR AS colour_text,
    TO_JSON(p.payload:colour)::VARCHAR AS colour_json,
    p.payload:colour IS NULL AS colour_is_missing,
    COALESCE(p.payload:colour::VARCHAR, 'undyed')::VARCHAR AS colour_or_default,
    GET(p.payload:attrs, 'Size (cm)')::VARCHAR AS size_cm,
    GET(p.payload:attrs, 'a.b')::VARCHAR AS dotted_attribute,
    p.payload:attrs['$discount']::VARCHAR AS discount_attribute,
    GET(p.payload:attrs, '')::VARCHAR AS empty_attribute,
    GET(p.payload:attrs, '*')::VARCHAR AS star_attribute,
    GET(p.payload:attrs, '$.a')::VARCHAR AS path_like_attribute,
    GET_IGNORE_CASE(p.payload:attrs, 'SKU')::VARCHAR AS sku_any_case,
    ARRAY_TO_STRING(
      ARRAY_COMPACT(p.payload:tags::ARRAY),
      '|'
    )::VARCHAR AS tag_list
  FROM payloads AS p
)

SELECT *
FROM final
