MACRO (
  description "Text with each run of whitespace collapsed to one space",
  arguments (text_value VARCHAR),
  returns VARCHAR,
);

REGEXP_REPLACE(
  @text_value,
  -- Each run of spaces, tabs and line breaks becomes one space
  '\\s+',
  ' '
)::VARCHAR
