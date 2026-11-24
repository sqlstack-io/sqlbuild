SCHEMA (
  name address,
  description "A postal location reduced to the parts used for reporting",
  columns (
    city (type VARCHAR),
    postcode_canonical (type VARCHAR),
    country_code (type VARCHAR),
    region_code (type VARCHAR),
  ),
);
