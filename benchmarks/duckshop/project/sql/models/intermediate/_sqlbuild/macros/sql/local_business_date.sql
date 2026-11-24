MACRO (
  description "Calendar date of a timestamp in a storefront's IANA time zone",
  arguments (ts TIMESTAMP_TZ, tz VARCHAR),
  returns DATE,
);

CONVERT_TIMEZONE(@tz, @ts)::DATE
