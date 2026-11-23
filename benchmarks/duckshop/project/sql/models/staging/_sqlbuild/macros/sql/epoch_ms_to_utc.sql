MACRO (
  description "UTC timestamp from epoch milliseconds",
  arguments (epoch_ms NUMBER(38, 0)),
  returns TIMESTAMP_NTZ,
);

TO_TIMESTAMP_NTZ(@epoch_ms, 3)
