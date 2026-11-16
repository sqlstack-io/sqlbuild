/**
 * How many items fall under each key, in one pass. Use it for filter option counts instead of
 * filtering the whole list once per option, which is quadratic when the options grow with the
 * project (one per node or folder).
 */
export function countBy<T, K>(items: readonly T[], key: (item: T) => K | K[] | null | undefined): Map<K, number> {
	const counts = new Map<K, number>();
	for (const item of items) {
		const keys = key(item);
		if (keys === null || keys === undefined) continue;
		for (const k of Array.isArray(keys) ? keys : [keys]) counts.set(k, (counts.get(k) ?? 0) + 1);
	}
	return counts;
}
