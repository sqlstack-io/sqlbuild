MODEL (
  description """
    One row per support ticket note searched with regular expressions at their edges: a lazy
    quantifier followed by another ?, a bracket class that opens with ] or ^], lazy one-or-more in a
    position search, positions of patterns that match empty text, . against a carriage return in
    multi-line mode, and case-insensitive matches of the micro sign and final sigma. Also ILIKE with
    a long s, alone and as the pattern beside an ESCAPE character, name similarity of accented names
    in both cases, Base64 text with line feeds inside groups and padding, and the word at a position
    numbered by a window ordered by a VARIANT
  """,
  tags [edge_case],
  columns (
    note_id (type VARCHAR, nullable false, audits [not_null, unique]),
    lazy_then_optional_count (type NUMBER(38, 0)),
    lazy_then_optional_match (type VARCHAR),
    optional_twice_match (type VARCHAR),
    bracket_close_match (type VARCHAR),
    bracket_negated_match (type VARCHAR),
    lazy_plus_position (type NUMBER(38, 0)),
    end_anchor_position (type NUMBER(38, 0)),
    star_position (type NUMBER(38, 0)),
    multiline_dot_match (type VARCHAR),
    matches_micro_sign (type BOOLEAN),
    matches_sigma (type BOOLEAN),
    ilike_s_prefix (type BOOLEAN),
    ilike_long_s_escaped (type BOOLEAN),
    name_similarity (type NUMBER(38, 0)),
    decoded_tags (type VARCHAR),
    ranked_word (type VARCHAR),
  ),
);

WITH notes AS (
  SELECT
    n.note_id,
    n.body,
    n.tags_text
  FROM __ref("stg_edge_ticket_notes") AS n
),

final AS (
  SELECT
    n.note_id,
    REGEXP_COUNT(n.body, '(xa)+??x')::NUMBER(38, 0) AS lazy_then_optional_count,
    REGEXP_SUBSTR(n.body, '(xa)+??x')::VARCHAR AS lazy_then_optional_match,
    REGEXP_SUBSTR(n.body, 'xa???')::VARCHAR AS optional_twice_match,
    REGEXP_SUBSTR(n.body, '[]x+?]+')::VARCHAR AS bracket_close_match,
    REGEXP_SUBSTR(n.body, '[^]+?]+')::VARCHAR AS bracket_negated_match,
    REGEXP_INSTR(n.body, 'ba+?')::NUMBER(38, 0) AS lazy_plus_position,
    REGEXP_INSTR(n.body, '$')::NUMBER(38, 0) AS end_anchor_position,
    REGEXP_INSTR(n.body, 'a*')::NUMBER(38, 0) AS star_position,
    REGEXP_SUBSTR(n.body, '^a.$', 1, 1, 'm')::VARCHAR AS multiline_dot_match,
    REGEXP_LIKE(n.body, '.*\u00b5.*', 'i')::BOOLEAN AS matches_micro_sign,
    REGEXP_LIKE(n.body, '.*\u03c3.*', 'i')::BOOLEAN AS matches_sigma,
    (
      n.body ILIKE 's%'
    )::BOOLEAN AS ilike_s_prefix,
    (
      n.body ILIKE '\u017f%' ESCAPE 's'
    )::BOOLEAN AS ilike_long_s_escaped,
    JAROWINKLER_SIMILARITY(n.body, n.tags_text)::NUMBER(38, 0) AS name_similarity,
    TRY_BASE64_DECODE_STRING(n.tags_text)::VARCHAR AS decoded_tags,
    SPLIT_PART(
      n.body,
      ' ',
      ROW_NUMBER() OVER (ORDER BY TO_VARIANT(n.note_id))
    )::VARCHAR AS ranked_word
  FROM notes AS n
)

SELECT *
FROM final
