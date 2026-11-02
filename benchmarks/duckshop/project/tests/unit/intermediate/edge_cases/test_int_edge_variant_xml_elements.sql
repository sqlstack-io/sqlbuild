TEST (name "int_edge_variant_xml_elements__reads_sku_elements");

WITH __ref__stg_edge_variant_values AS (
  SELECT
    v.value_id::VARCHAR AS value_id,
    v.group_code::VARCHAR AS group_code,
    v.sort_key::NUMBER(38, 0) AS sort_key,
    PARSE_XML(v.attribute_value) AS attribute_value,
    v.value_text::VARCHAR AS value_text
  FROM VALUES
    ('VX-01', 'X-1', 1, '<line><sku>DK-1</sku></line>', NULL),
    ('VX-02', 'X-1', 2, '<line><qty>2</qty></line>', NULL) AS v(value_id, group_code, sort_key, attribute_value, value_text)
),

__expected__int_edge_variant_xml_elements AS (
  SELECT
    NULL AS value_id,
    NULL AS sku_text
)
