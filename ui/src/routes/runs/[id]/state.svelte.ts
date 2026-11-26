import { to } from '$lib/paths';
import type { RunEvent, RunNode, RunNodeError } from '$lib/api';
import { untrack } from 'svelte';
import { followEvents, followOutput, newFollow, type RunFollow } from './api/follow';
import { errorMessage } from '$lib/platform/labels';
import { app } from '$lib/state/app.svelte';
import { createResource } from '$lib/state/resource.svelte';
import { createSelection } from '$lib/state/selection.svelte';
import { nodeLabel } from '$lib/operate/labels';
import { planHref } from '$lib/run/command';
import { pageMemory } from './utils/page-memory';
import { runFailures, summaryOf, withNameToggled, type RunFailure } from './utils';
import { holdsText, QUICK, toggleText } from './utils/log-query';
import { isActiveRun } from '$lib/status';
import type { BottomTab } from './types';
import { bannersFor, groupNodes, progressOf, runCostOf } from './utils';

function message(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

/** Run detail state: data, the node selection that filters the bottom pane, and actions. */
export function createRunDetailState(runId: string) {
	// Back to this page restores it as it was.
	const memory = pageMemory(runId);
	const remembered = <T>(key: string, fallback: T): T => (key in memory ? (memory[key] as T) : fallback);
	const topics = [`run:${runId}`, 'runs'];
	// A running run changes about once a second. Its node states are small and reload after each burst;
	// its log, statements, output and cost read the whole run each time, so they reload at most every
	// few seconds, and the page keeps up instead of falling ever further behind.
	const BULKY = { every: 3000 };
	const detailRes = createResource((client, scope) => client.getRunDetail(scope, runId), topics);
	// A group run's own reads are nearly empty, so its log and statements gather its job runs' too.
	// A string, so the reads below reload only when the set of runs changes, not on every detail reload.
	const runKey = $derived([runId, ...(detailRes.data?.children ?? []).map((c) => c.id)].join(','));
	const eachRun = async <T>(read: (id: string) => Promise<T[]>): Promise<T[]> => (await Promise.all(runKey.split(',').map(read))).flat();
	// Each run's events and output carry on from where the last read stopped, and a run read once
	// after it ended is not read again: its record cannot change.
	const follows = new Map<string, RunFollow>();
	const followOf = (id: string) => follows.get(id) ?? follows.set(id, newFollow()).get(id)!;
	/** The runs whose events and output were complete when a read started: ended, by the detail last read. */
	const endedRuns = () => {
		const d = untrack(() => detailRes.data);
		return new Set([d?.run, ...(d?.children ?? [])].filter((r) => r && !isActiveRun(r.status)).map((r) => r!.id));
	};
	let nodeLogs = $state<RunEvent[]>([]);
	const eventsRes = createResource(
		(client, scope) => {
			if (!client.supports('listRunEvents')) return Promise.resolve([]);
			const ended = endedRuns();
			return eachRun((id) => followEvents(client, scope, followOf(id), { id, ended: ended.has(id) })).then((events) => events.sort((a, b) => a.occurredAt.localeCompare(b.occurredAt)));
		},
		topics,
		BULKY
	);
	const outputRes = createResource(
		(client, scope) => {
			if (!client.supports('listRunOutput') || !app.can('view.evidence')) return Promise.resolve([]);
			const ended = endedRuns();
			return eachRun((id) => followOutput(client, scope, followOf(id), { id, ended: ended.has(id) })).then((lines) => {
				nodeLogs = runKey.split(',').flatMap((id) => followOf(id).logs);
				return lines.sort((a, b) => a.at.localeCompare(b.at));
			});
		},
		topics,
		BULKY
	);
	/** The runs' events with the lines their nodes logged with `ctx.log`, in time order. */
	const events = $derived(nodeLogs.length ? [...(eventsRes.data ?? []), ...nodeLogs].sort((a, b) => a.occurredAt.localeCompare(b.occurredAt)) : (eventsRes.data ?? []));
	const statementsRes = createResource((client, scope) => (client.supports('listRunStatements') ? eachRun((id) => client.listRunStatements(scope, id)) : Promise.resolve([])), topics, BULKY);
	const costRes = createResource((client, scope) => (client.supports('listRunCost') ? eachRun((id) => client.listRunCost(scope, id)) : Promise.resolve([])), topics, BULKY);

	const detail = $derived(detailRes.data ?? null);
	const run = $derived(detail?.run ?? null);
	const nodes = $derived(detail?.nodes ?? []);
	const active = $derived(!!run && isActiveRun(run.status));
	const progress = $derived(progressOf(nodes));
	const nodeGroups = $derived(groupNodes(nodes, active));
	/** The timeline shows every node: quick filters narrow the log, through its filter text, and nothing else. */
	const shownNodes = $derived(nodes);
	// ---- the log's one filter: the text in its box ----
	// The header's figures, the Errors chip and the Pick chips only add their words to it or take them out;
	// what is shown, what is pressed and the "Showing …" line are read back from it.
	let logQuery = $state<string>(remembered('logQuery', ''));
	const cause = $derived(outputRes.data?.find((l) => l.error)?.text ?? null);
	// The failure summary leads when it knows what failed; the worker's own end line is never a reason.
	const housekeeping = (text: string | null) => !!text && /worker executor completed|closed its connections/.test(text);
	/** What made the run fail, with the error text read so far (behind view.evidence); the summary, the menu and the banners share it. */
	const failures = $derived(
		runFailures(nodes, events, statementsRes.data ?? [], run?.target ?? '', {
			node: (n) => {
				const read = nodeErrors[`${n.runId}/${n.nodeKey}`];
				return read && 'nodeKey' in read ? read.message : null;
			},
			audit: (eventId) => auditErrors[eventId] ?? null
		})
	);
	const failureCount = $derived(failures.length);
	const banners = $derived(
		detail
			? bannersFor(housekeeping(detail.run.statusReason) ? { ...detail, run: { ...detail.run, statusReason: null } } : detail, app.now, housekeeping(cause) ? null : cause).filter((b) => b.key !== 'failed' || !failureCount)
			: []
	);
	const statements = $derived(statementsRes.data ?? []);
	const cost = $derived(costRes.data ? runCostOf(costRes.data) : null);
	// Progress reported by a run with no signal is stale, so the live card only shows while it's running.
	const runningStatements = $derived(run?.status === 'running' ? statements.filter((s) => s.status === 'running') : []);
	let showDetails = $state(false);

	// ---- the selection to plan: the Lineage page's own selection, typed or picked ----
	// It reads like `sqb build -s …`; quick picks fill it, and ticking a node on the timeline or graph adds
	// or removes its name. The timeline, the graph and (by default) the log follow what it selects.
	const selection = createSelection({ initial: { select: remembered('select', ''), exclude: remembered('exclude', '') } });
	/** The run's nodes the selection selects, by key. */
	const selected = $derived(selection.keys ? nodes.filter((n) => selection.keys!.has(n.nodeKey)).map((n) => n.nodeKey) : []);
	let tab = $state<BottomTab>(remembered('tab', 'log'));
	/** The run's nodes the selection leaves out by name (`--exclude`), struck through on the timeline and graph. */
	const excluded = $derived.by(() => {
		const out = new Set(selection.exclude.split(/\s+/).filter(Boolean));
		return nodes.filter((n) => out.has(nodeLabel(n.nodeKey))).map((n) => n.nodeKey);
	});

	/**
	 * Ticking a node always toggles whether it is shown: its plain name comes out of, or goes
	 * into, the selection; a node selected through a pick such as `fct_order_margin+` is excluded instead,
	 * and ticking it again takes the exclude back out.
	 */
	function selectNode(key: string) {
		const name = nodeLabel(key);
		const words = (text: string) => text.split(/\s+/).filter(Boolean);
		if (words(selection.exclude).includes(name)) selection.exclude = withNameToggled(selection.exclude, name);
		else if (words(selection.select).includes(name) || !selected.includes(key)) selection.select = withNameToggled(selection.select, name);
		else selection.exclude = withNameToggled(selection.exclude, name);
	}

	/** A quick filter's click: its words go into the log's filter text, or come out again; the log shows. */
	function toggleLog(text: string) {
		logQuery = toggleText(logQuery, text);
		tab = 'log';
	}
	function clearSelection() {
		selection.select = '';
		selection.exclude = '';
	}

	// ---- Statements and Cost: a node picked for one of them; the log has its filter text instead ----
	// A jump's filter belongs to the tab it opened (`tab`); leaving that tab drops it.
	let focus = $state<{ label: string; keys: string[]; tab?: BottomTab } | null>(remembered('focus', null));

	/** The failure summary's Show in log: its words become the log's filter text. */
	function showInLog(query: string) {
		logQuery = `${query} `;
		tab = 'log';
	}

	/** A node picked for a tab: the log gets `node:<name>` in its text; Statements and Cost follow it as before. */
	function focusNode(key: string, nextTab: BottomTab) {
		if (nextTab === 'log') {
			logQuery = toggleText(logQuery.split(/\s+/).filter((w) => w && !w.toLowerCase().startsWith('node:')).join(' '), `node:${nodeLabel(key)}`);
			focus = null;
		} else focus = { label: nodeLabel(key), keys: [key], tab: nextTab };
		tab = nextTab;
	}

	function switchTab(next: BottomTab) {
		if (focus?.tab && focus.tab !== next) focus = null;
		if (next !== 'statements') statementFocus = null;
		tab = next;
	}

	// ---- failed nodes' error text ----
	// An audited evidence read: once per node, and only when the error is shown.
	const errorsReadable = $derived(app.client.supports('getRunNodeError') && app.can('view.evidence'));
	let nodeErrors = $state<Record<string, RunNodeError | { failure: string }>>({});
	const errorsAsked = new Set<string>();
	async function loadNodeError(n: RunNode) {
		const k = `${n.runId}/${n.nodeKey}`;
		if (!errorsReadable || n.errorDetail || errorsAsked.has(k) || !app.scope) return;
		errorsAsked.add(k);
		try {
			nodeErrors[k] = await app.client.getRunNodeError(app.scope, n.runId, n.nodeKey);
		} catch (error) {
			nodeErrors[k] = { failure: errorMessage(error) };
		}
	}

	// A failed audit's warehouse error: its result's evidence read, audited like a node's error, once per result.
	const auditErrorsReadable = $derived(app.client.supports('getCheckEvidence') && app.can('view.evidence'));
	let auditErrors = $state<Record<string, string | null>>({});
	const auditsAsked = new Set<string>();
	async function loadAuditError(eventId: string, bindingKey: string | null) {
		if (!auditErrorsReadable || !bindingKey || auditsAsked.has(eventId) || !app.scope) return;
		auditsAsked.add(eventId);
		try {
			auditErrors[eventId] = (await app.client.getCheckEvidence(app.scope, bindingKey, eventId)).executionError;
		} catch {
			auditErrors[eventId] = null;
		}
	}
	/** Reads a failure's error text, once, when it is shown. */
	function loadFailureText(f: RunFailure) {
		if (f.kind === 'check') return void loadAuditError(f.id, f.checkId);
		const n = nodes.find((x) => `${x.runId}/${x.nodeKey}` === f.id);
		if (n) void loadNodeError(n);
	}

	// ---- the node drawer ----
	let drawer = $state<string | null>(remembered('drawer', null));
	/** The statement a check's "In Statements" opened. */
	let statementFocus = $state<string | null>(null);
	/** A line of the drawer's node's code to show, from a traceback frame. */
	let codeLine = $state<number | null>(remembered('codeLine', null));
	$effect(() => {
		Object.assign(memory, { select: selection.select, exclude: selection.exclude, tab, focus: $state.snapshot(focus), logQuery, drawer, codeLine });
	});

	const summary = $derived(summaryOf(nodes, events, active));
	/** Plan these: the plan page for the selection, as the job (or the group) would build it. */
	const planLink = $derived(
		selection.select.trim()
			? planHref(selection.select.trim().split(/\s+/), 'build', { job: run?.kind === 'group' ? undefined : (run?.jobKey ?? undefined), exclude: selection.exclude.trim() ? selection.exclude.trim().split(/\s+/) : [] })
			: null
	);

	// ---- actions ----
	let busy = $state<'cancel' | null>(null);
	let actionError = $state<string | null>(null);
	let cancelArmed = $state(false);

	async function cancel() {
		const scope = app.scope;
		if (!scope || !run) return;
		if (!cancelArmed) {
			cancelArmed = true;
			return;
		}
		cancelArmed = false;
		busy = 'cancel';
		actionError = null;
		try {
			await app.client.cancelRun(scope, run.id);
			detailRes.refresh();
		} catch (e) {
			actionError = message(e);
		} finally {
			busy = null;
		}
	}

	return {
		detailRes,
		eventsRes,
		get events() {
			return events;
		},
		outputRes,
		statementsRes,
		costRes,
		get cost() {
			return cost;
		},
		get detail() {
			return detail;
		},
		get run() {
			return run;
		},
		get nodes() {
			return nodes;
		},
		get nodeGroups() {
			return nodeGroups;
		},
		get shownNodes() {
			return shownNodes;
		},
		/** The log's filter text: the one filter the log has. */
		get logQuery() {
			return logQuery;
		},
		set logQuery(text: string) {
			logQuery = text;
		},
		/** A quick filter's click: its words in the log's filter text, or out again. */
		toggleLog,
		/** Whether a quick filter's words are all in the log's filter text: it reads as pressed. */
		logHolds(text: string) {
			return holdsText(logQuery, text);
		},
		get active() {
			return active;
		},
		get progress() {
			return progress;
		},
		get banners() {
			return banners;
		},
		get statements() {
			return statements;
		},
		get runningStatements() {
			return runningStatements;
		},
		get selected() {
			return selected;
		},
		selection,
		get focus() {
			return focus;
		},
		clearFocus() {
			focus = null;
		},
		showInLog,
		get errorsReadable() {
			return errorsReadable;
		},
		/** What made the run fail, failed audits and nodes, with the error text read so far. */
		get failures() {
			return failures;
		},
		loadFailureText,
		loadNodeError,
		/** The error read for a failed node, once it has been. */
		nodeError(n: RunNode): RunNodeError | null {
			const found = nodeErrors[`${n.runId}/${n.nodeKey}`];
			return found && 'nodeKey' in found ? found : null;
		},
		/** Why a failed node's error could not be read. */
		nodeErrorFailure(n: RunNode): string | null {
			const found = nodeErrors[`${n.runId}/${n.nodeKey}`];
			return found && 'failure' in found ? found.failure : null;
		},
		get drawer() {
			return drawer;
		},
		openDrawer(key: string | null) {
			drawer = key;
			if (key) showDetails = false;
			codeLine = null;
		},
		get codeLine() {
			return codeLine;
		},
		/** A traceback frame in a node's own code: the drawer, at that line of its code. */
		openCode(key: string, line: number) {
			drawer = key;
			codeLine = line;
		},
		get excluded() {
			return excluded;
		},
		/** What this page remembers for Back: components keep their own parts here too. */
		memory,
		get statementFocus() {
			return statementFocus;
		},
		/** A check's query: the Statements tab, filtered to its node, with that statement open. */
		showStatement(statementId: string, nodeKey: string) {
			focus = { label: nodeLabel(nodeKey), keys: [nodeKey], tab: 'statements' };
			statementFocus = statementId;
			tab = 'statements';
		},
		/** A click on a node's row, bar or card: ticks it in the selection (or unticks it) and shows it in the drawer. */
		clickNode(key: string) {
			selectNode(key);
			drawer = key;
			showDetails = false;
			codeLine = null;
		},
		get summary() {
			return summary;
		},
		get planLink() {
			return planLink;
		},
		get showDetails() {
			return showDetails;
		},
		/** Details and the node drawer share the right-hand panel: opening one closes the other. */
		toggleDetails() {
			showDetails = !showDetails;
			if (showDetails) drawer = null;
		},
		get tab() {
			return tab;
		},
		set tab(value: BottomTab) {
			switchTab(value);
		},
		selectNode,
		clearSelection,
		focusNode,
		get busy() {
			return busy;
		},
		get actionError() {
			return actionError;
		},
		get cancelArmed() {
			return cancelArmed;
		},
		disarmCancel() {
			cancelArmed = false;
		},
		cancel
	};
}

export type RunDetailState = ReturnType<typeof createRunDetailState>;
