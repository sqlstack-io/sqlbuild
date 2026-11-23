MACRO (
  description "Trimmed text form of any scalar, or NULL when blank",
  arguments (value ANY),
  returns VARCHAR,
);

NULLIF(TRIM(CAST(@value AS VARCHAR)), '')
