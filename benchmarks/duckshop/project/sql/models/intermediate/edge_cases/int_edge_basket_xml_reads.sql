MODEL (
  description """
    One row per basket line written as a small XML document and read back: CHECK_XML of the text and
    of a broken copy, PARSE_XML, and XMLGET of the line's tag and SKU elements, including a line
    whose tag element is empty
  """,
  tags [edge_case],
  columns (
    line_id (type VARCHAR, nullable false, audits [not_null, unique]),
    document_is_valid (type BOOLEAN),
    broken_copy_is_valid (type BOOLEAN),
    root_name (type VARCHAR),
    tag_text (type VARCHAR),
    sku_text (type VARCHAR),
  ),
);

WITH lines AS (
  SELECT
    l.line_id,
    l.sku,
    l.tag
  FROM __ref("stg_edge_basket_lines") AS l
),

documents AS (
  SELECT
    l.line_id,
    (
      '<line><sku>' || l.sku || '</sku><tag>' || COALESCE(l.tag, '') || '</tag></line>'
    )::VARCHAR AS document_text
  FROM lines AS l
),

parsed AS (
  SELECT
    d.line_id,
    d.document_text,
    PARSE_XML(d.document_text) AS document
  FROM documents AS d
),

final AS (
  SELECT
    p.line_id,
    (CHECK_XML(p.document_text) IS NULL)::BOOLEAN AS document_is_valid,
    (CHECK_XML(REPLACE(p.document_text, '</line>', '')) IS NULL)::BOOLEAN AS broken_copy_is_valid,
    GET(p.document, '@')::VARCHAR AS root_name,
    XMLGET(p.document, 'tag'):"$"::VARCHAR AS tag_text,
    XMLGET(p.document, 'sku'):"$"::VARCHAR AS sku_text
  FROM parsed AS p
)

SELECT *
FROM final
