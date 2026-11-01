TEST (name "int_edge_note_patterns__searches_notes_with_edge_patterns");

WITH __ref__stg_edge_ticket_notes AS (
  SELECT
    v.note_id,
    IFF(v.body = '@long', REPEAT('pond duck ', 40), v.body) AS body,
    v.tags_text
  FROM VALUES
    ('TN-01', 'xaxax', NULL),
    ('TN-02', 'xab xa', NULL),
    ('TN-03', 'x+', NULL),
    ('TN-04', 'bxba', NULL),
    ('TN-05', 'bxb', NULL),
    ('TN-06', '', NULL),
    ('TN-07', 'baa', NULL),
    ('TN-08', 'a\r\nb', NULL),
    ('TN-09', '\u03bc', NULL),
    ('TN-10', '\u03c2', NULL),
    ('TN-11', 'sabc', NULL),
    ('TN-12', '\u017fabc', NULL),
    ('TN-13', '\u00c9\u00e9', '\u00e9\u00c9'),
    ('TN-14', '\u00c9mile \u00e9mile', '\u00e9mile \u00c9mile'),
    ('TN-15', 'Pintail', 'b3J\nkZXI='),
    ('TN-16', 'Teal', 'YQ=\n='),
    ('TN-17', 'Eider', 'YQ\n=='),
    ('TN-18', '@long', NULL),
    ('TN-19', NULL, NULL) AS v(note_id, body, tags_text)
),

__expected__int_edge_note_patterns AS (
  SELECT
    NULL AS note_id,
    NULL AS lazy_then_optional_count,
    NULL AS lazy_then_optional_match,
    NULL AS optional_twice_match,
    NULL AS bracket_close_match,
    NULL AS bracket_negated_match,
    NULL AS lazy_plus_position,
    NULL AS end_anchor_position,
    NULL AS star_position,
    NULL AS multiline_dot_match,
    NULL AS matches_micro_sign,
    NULL AS matches_sigma,
    NULL AS ilike_s_prefix,
    NULL AS ilike_long_s_escaped,
    NULL AS name_similarity,
    NULL AS decoded_tags,
    NULL AS ranked_word
)
