// Where a warehouse shows one of its queries. Pure.

/**
 * A warehouse query's page in its console, or null where the adapter has none (only Snowflake's query detail so far).
 */
export function warehouseQueryUrl(adapter: string | null | undefined, queryId: string): string | null {
	if (adapter === 'snowflake') return `https://app.snowflake.com/#/compute/history/queries/${encodeURIComponent(queryId)}/detail`;
	return null;
}
