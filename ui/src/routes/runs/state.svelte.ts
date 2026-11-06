import { goto } from '$app/navigation';
import { page } from '$app/state';
import { untrack } from 'svelte';
import { channelMatches, type RunRow, type RunTabCounts } from '$lib/api';
import { app } from '$lib/state/app.svelte';
import type { BulkAction, DisplayRow, Freshness, ListParams } from './types';
import { eligibleIds, readParams, toQuery, writeParams } from './utils';

const PAGE = 50;
/** Live signals refetch at most this often: the first immediately, then one trailing. */
const SIGNAL_GAP_MS = 300;
/** Safety refresh in case a signal was missed. */
const SAFETY_MS = 5 * 60_000;

function message(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

/** Runs list state: URL-backed filters, cursor-paged rows, tab counts, selection and bulk actions. */
export function createRunsState() {
	const params = $derived(readParams(page.url.searchParams));
	/** Changes when the rows need reloading from the start (the view switch doesn't count). */
	const key = $derived(JSON.stringify({ ...params, view: undefined }));

	let rows = $state<RunRow[]>([]);
	let nextCursor = $state<string | null>(null);
	let loading = $state(true);
	let loadingMore = $state(false);
	let error = $state<string | null>(null);
	let updatedAt = $state<number | null>(null);
	let counts = $state<RunTabCounts | null>(null);
	let countsError = $state(false);
	let expanded = $state<Set<string>>(new Set());
	let children = $state<Record<string, RunRow[]>>({});
	let request = 0;

	function update(patch: Partial<ListParams>) {
		void goto(writeParams(page.url, { ...params, ...patch }), { replaceState: true, noScroll: true, keepFocus: true });
	}

	async function loadRows(mode: 'reset' | 'refresh' | 'more') {
		const scope = app.scope;
		if (!scope) return;
		const id = ++request;
		const query = toQuery(params, Date.now());
		if (mode === 'more') {
			if (!nextCursor || loadingMore) return;
			loadingMore = true;
		} else {
			loading = true;
		}
		try {
			const result = await app.client.listRunRows(scope, {
				...query,
				cursor: mode === 'more' ? nextCursor : null,
				limit: mode === 'refresh' ? Math.max(PAGE, rows.length) : PAGE
			});
			if (id !== request) return;
			rows = mode === 'more' ? [...rows, ...result.rows] : result.rows;
			nextCursor = result.nextCursor;
			error = null;
			updatedAt = Date.now();
		} catch (e) {
			if (id === request) error = message(e);
		} finally {
			if (id === request) {
				loading = false;
				loadingMore = false;
			}
		}
	}

	async function loadCounts() {
		const scope = app.scope;
		if (!scope) return;
		try {
			counts = await app.client.runTabCounts(scope, toQuery(params, Date.now(), 'all'));
			countsError = false;
		} catch {
			countsError = true;
		}
	}

	async function loadChildren(parentId: string) {
		const scope = app.scope;
		if (!scope) return;
		try {
			const result = await app.client.listRunRows(scope, { parentRunId: parentId, includeChildren: true, limit: 200 });
			children = { ...children, [parentId]: [...result.rows].sort((a, b) => a.run.queuedAt.localeCompare(b.run.queuedAt) || a.run.id.localeCompare(b.run.id)) };
		} catch {
			children = { ...children, [parentId]: [] };
		}
	}

	function refresh() {
		void loadRows('refresh');
		void loadCounts();
		for (const id of expanded) void loadChildren(id);
	}

	// Reload from the start whenever the scope or the filters change.
	$effect(() => {
		void key;
		if (!app.scope) return;
		untrack(() => {
			rows = [];
			nextCursor = null;
			selected = new Set();
			armed = null;
			void loadRows('reset');
			void loadCounts();
		});
	});

	// Live signals, rate-capped, paused while the tab is hidden; plus a slow safety refresh.
	$effect(() => {
		let last = 0;
		let trailing: ReturnType<typeof setTimeout> | null = null;
		let missed = false;
		const fire = () => {
			last = Date.now();
			refresh();
		};
		const onSignal = () => {
			if (document.hidden) {
				missed = true;
				return;
			}
			const wait = SIGNAL_GAP_MS - (Date.now() - last);
			if (wait <= 0) fire();
			else if (!trailing) trailing = setTimeout(() => ((trailing = null), fire()), wait);
		};
		const off = app.client.changes.subscribe((signal) => {
			if (channelMatches(signal.channel, ['runs'])) onSignal();
		});
		const onVisible = () => {
			if (!document.hidden && missed) {
				missed = false;
				fire();
			}
		};
		document.addEventListener('visibilitychange', onVisible);
		const safety = setInterval(() => !document.hidden && fire(), SAFETY_MS);
		return () => {
			off();
			document.removeEventListener('visibilitychange', onVisible);
			clearInterval(safety);
			if (trailing) clearTimeout(trailing);
		};
	});

	function toggleExpanded(id: string) {
		const next = new Set(expanded);
		if (next.has(id)) next.delete(id);
		else {
			next.add(id);
			void loadChildren(id);
		}
		expanded = next;
	}

	const display = $derived.by((): DisplayRow[] => {
		const out: DisplayRow[] = [];
		for (const row of rows) {
			out.push({ key: row.run.id, row, child: false });
			if (expanded.has(row.run.id)) for (const c of children[row.run.id] ?? []) out.push({ key: `${row.run.id}/${c.run.id}`, row: c, child: true });
		}
		return out;
	});

	// ---- live indicator ----
	const freshness = $derived.by((): Freshness => {
		if (app.connection === 'offline') return 'stale';
		if (updatedAt != null && app.now - updatedAt > SAFETY_MS + 30_000) return 'stale';
		if (app.connection === 'live') return 'live';
		return 'reconnecting';
	});

	// ---- selection and bulk actions ----
	let selected = $state<Set<string>>(new Set());
	let armed = $state<BulkAction | 'cancel_all' | null>(null);
	let busy = $state<BulkAction | 'cancel_all' | null>(null);
	let result = $state<{ text: string; failed: boolean } | null>(null);

	const known = $derived(new Map([...rows, ...Object.values(children).flat()].map((r) => [r.run.id, r])));
	const selectedRows = $derived([...selected].map((id) => known.get(id)).filter((r): r is RunRow => !!r));
	const can = (c: Parameters<typeof app.can>[0]) => app.can(c);
	const eligibleFor = $derived({
		cancel: eligibleIds('cancel', selectedRows, can),
		rerun: eligibleIds('rerun', selectedRows, can),
		from_failure: eligibleIds('from_failure', selectedRows, can)
	});

	function setSelected(next: Set<string>) {
		selected = next;
		armed = null;
	}

	function toggleSelected(id: string) {
		const next = new Set(selected);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		setSelected(next);
	}

	const allVisibleSelected = $derived(display.length > 0 && display.every((d) => selected.has(d.row.run.id)));
	function toggleAll() {
		setSelected(allVisibleSelected ? new Set() : new Set(display.map((d) => d.row.run.id)));
	}

	/** First click arms, second click runs; any selection change disarms. */
	async function bulk(action: BulkAction) {
		if (armed !== action) {
			armed = action;
			return;
		}
		const scope = app.scope;
		const ids = eligibleFor[action];
		if (!scope || !ids.length) return;
		armed = null;
		busy = action;
		result = null;
		const outcomes = await Promise.allSettled(
			ids.map((id) => (action === 'cancel' ? app.client.cancelRun(scope, id) : app.client.rerunRun(scope, id, action === 'rerun' ? 'all' : 'from_failure')))
		);
		const failed = ids.filter((_, i) => outcomes[i].status === 'rejected');
		const firstError = outcomes.find((o): o is PromiseRejectedResult => o.status === 'rejected');
		const verb = action === 'cancel' ? 'Cancel requested for' : 'Started reruns of';
		const ok = ids.length - failed.length;
		result = failed.length
			? { text: `${ok ? `${verb} ${ok} run${ok === 1 ? '' : 's'}. ` : ''}${failed.length} failed: ${message(firstError?.reason)}. The failed runs stay selected.`, failed: true }
			: { text: `${verb} ${ok} run${ok === 1 ? '' : 's'}.`, failed: false };
		selected = new Set(failed);
		busy = null;
		refresh();
	}

	async function cancelAll() {
		if (armed !== 'cancel_all') {
			armed = 'cancel_all';
			return;
		}
		const scope = app.scope;
		if (!scope) return;
		armed = null;
		busy = 'cancel_all';
		result = null;
		try {
			const res = await app.client.cancelMatchingRuns(scope, toQuery(params, Date.now()));
			result = res.failed.length
				? { text: `Cancel requested for ${res.done.length} runs; ${res.failed.length} failed: ${res.failed[0].error}`, failed: true }
				: { text: `Cancel requested for ${res.done.length} run${res.done.length === 1 ? '' : 's'}.`, failed: false };
			selected = new Set(res.failed.map((f) => f.runId));
		} catch (e) {
			result = { text: message(e), failed: true };
		} finally {
			busy = null;
			refresh();
		}
	}

	return {
		get params() {
			return params;
		},
		update,
		get rows() {
			return rows;
		},
		get display() {
			return display;
		},
		get loading() {
			return loading;
		},
		get loadingMore() {
			return loadingMore;
		},
		get hasMore() {
			return nextCursor !== null;
		},
		get error() {
			return error;
		},
		get updatedAt() {
			return updatedAt;
		},
		get counts() {
			return counts;
		},
		get countsError() {
			return countsError;
		},
		get freshness() {
			return freshness;
		},
		get expanded() {
			return expanded;
		},
		get selected() {
			return selected;
		},
		get selectedRows() {
			return selectedRows;
		},
		get eligibleFor() {
			return eligibleFor;
		},
		get allVisibleSelected() {
			return allVisibleSelected;
		},
		get armed() {
			return armed;
		},
		get busy() {
			return busy;
		},
		get result() {
			return result;
		},
		dismissResult() {
			result = null;
		},
		clearSelection() {
			setSelected(new Set());
		},
		loadMore: () => loadRows('more'),
		refresh,
		toggleExpanded,
		toggleSelected,
		toggleAll,
		bulk,
		cancelAll
	};
}

export type RunsState = ReturnType<typeof createRunsState>;
