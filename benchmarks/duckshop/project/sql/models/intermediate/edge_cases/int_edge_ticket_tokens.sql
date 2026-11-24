MODEL (
  description """
    One row per basket line with values from volatile project Python functions: a ticket token from
    the line id, from a constant and from no argument at all, and half a ticket number through DIV0,
    beside a column and text that only look like the function's name. Snowflake runs a Python
    function once per call and row, so the values change on every run; the test checks only facts
    that hold for any values, such as every token being distinct
  """,
  tags [edge_case, known_divergence],
  columns (
    line_id (type VARCHAR, nullable false, audits [not_null, unique]),
    line_token (type VARCHAR),
    constant_seed_token (type VARCHAR),
    next_ticket (type NUMBER(38, 0)),
    half_ticket (type NUMBER(38, 6)),
    edge_next_ticket_source (type VARCHAR),
    look_alike_text (type VARCHAR),
    tag_label (type VARCHAR),
  ),
);

WITH lines AS (
  SELECT
    l.line_id,
    l.tag
  FROM __ref("stg_edge_basket_lines") AS l
),

final AS (
  SELECT
    l.line_id,
    __udf("edge_ticket_token")(l.line_id)::VARCHAR AS line_token,
    __udf("edge_ticket_token")('x')::VARCHAR AS constant_seed_token,
    __udf("edge_next_ticket")()::NUMBER(38, 0) AS next_ticket,
    DIV0(__udf("edge_next_ticket")(), 2)::NUMBER(38, 6) AS half_ticket,
    l.line_id AS edge_next_ticket_source,
    'edge_next_ticket edge_next_ticket'::VARCHAR AS look_alike_text,
    __udf("edge_tag_label")(l.tag)::VARCHAR AS tag_label
  FROM lines AS l
)

SELECT *
FROM final
