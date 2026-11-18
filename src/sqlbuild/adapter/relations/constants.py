"""Shared relation metadata limits."""

from datetime import timedelta

INSPECTION_DESCRIBED_NAME_LIMIT: int = 3
INSPECTION_IN_LIST_LIMIT: int = 50
METADATA_NAME_FILTER_LIMIT: int = 250
RELATION_COLUMNS_LOOKUP: str = "columns"
RELATION_EXISTS_LOOKUP: str = "exists"
RELATION_TYPE_LOOKUP: str = "type"
SCHEMA_LISTING_LOOKUP_THRESHOLD: int = 2
SCHEMA_LISTING_MAX_AGE_SECONDS: float = 60.0
UNKNOWN_RELATION_READER: str = "unknown user"
SYNCED_CATALOG_ATTRIBUTE: str = "synced_catalog"
SYNCED_REFRESH_LIMIT: int = 128
SYNCED_DDL_CLOCK_SKEW: timedelta = timedelta(minutes=5)
