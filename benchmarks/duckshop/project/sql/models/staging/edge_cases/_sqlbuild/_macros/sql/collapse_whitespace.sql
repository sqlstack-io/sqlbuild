MACRO (
  description "Text with each run of whitespace collapsed to one space",
  arguments (text_value VARCHAR),
  returns VARCHAR,
);

/* Spaces, tabs and line breaks all count as whitespace,
   so a value pasted from a spreadsheet collapses too */
REGEXP_REPLACE(
  @text_value,
  -- Each run of spaces, tabs and line breaks becomes one space
  '\\s+',
  ' '
)::VARCHAR
