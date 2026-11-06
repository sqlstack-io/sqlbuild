TEST (mode udf, name "edge_tag_label__labels_missing_tags");

WITH input_values AS (
  SELECT v.tag
  FROM VALUES (' bath '), ('GIFT'), (''), (NULL) AS v(tag)
),

__udf_actual__ AS (
  SELECT
    i.tag,
    __udf("edge_tag_label")(i.tag) AS tag_label
  FROM input_values AS i
),

-- Not declared STRICT, so Snowflake calls the handler for NULL and returns its label.
__udf_expected__ AS (
  SELECT
    v.tag::VARCHAR AS tag,
    v.tag_label::VARCHAR AS tag_label
  FROM VALUES
    (' bath ', 'Bath'),
    ('GIFT', 'Gift'),
    ('', 'Untagged'),
    (NULL, 'Untagged') AS v(tag, tag_label)
)
