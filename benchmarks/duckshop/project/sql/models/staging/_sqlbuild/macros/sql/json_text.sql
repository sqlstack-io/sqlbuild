MACRO (
  description "Text of a semi-structured value, treating JSON null and blank strings as SQL NULL",
  arguments (value VARIANT),
  returns VARCHAR,
);

NULLIF(
  TRIM(STRIP_NULL_VALUE(@value)::VARCHAR),
  ''
)
