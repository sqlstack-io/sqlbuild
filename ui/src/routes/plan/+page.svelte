<script lang="ts">
	import { copyText } from '$lib/browser/clipboard';
	import { noun, plural } from '$lib/format/plural';
	import PageBody from '$lib/components/page-body.svelte';
	import DataTable from '$lib/components/list/data-table.svelte';
	import type { TableColumn } from '$lib/components/list/table-columns';
	import Button from '$lib/components/ui/button/button.svelte';
	import { appPath, to } from '$lib/paths';
	import PlayIcon from '@lucide/svelte/icons/play';
	import HelpTip from '$lib/components/help-tip.svelte';
	// Every Run button opens this page with its command. A build is planned by the location (`sqb plan`)
	// from the server's synced copy of the warehouse metadata; other verbs show the server's dry run.
	// Nothing is queued until Run is pressed.
	// With a job, the command only narrows that job: Run starts a run of the job, not an ad-hoc command.
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import CopyIcon from '@lucide/svelte/icons/copy';
	import RotateCwIcon from '@lucide/svelte/icons/rotate-cw';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import XIcon from '@lucide/svelte/icons/x';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import { mergePlanPage, type AdhocRunCheck, type AdhocRunRequest, type LaunchVerb, type PlanDocument, type PlanPageSection, type PlatformEvent } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import SyncingNote from '$lib/components/platform/syncing-note.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import { kindLine } from '$lib/build-area/labels';
	import { formatAgo } from '$lib/format/time';
	import ScopeGraph from '$lib/lineage/scope-graph.svelte';
	import { VERBS } from '$lib/run/command-table';
	import { applySuggestion, commandTokens, jobBuildTokens, jobRunProblem, parseCommand, settledJobCommand, suggest, windowOf, withFullRefresh, withVerb, withWindow, type Suggestion } from '$lib/run/command';
	import DataRange from '$lib/run/data-range.svelte';
	import PlanBrowser from '$lib/run/plan-browser.svelte';
	import { cantRunKeys, cantRunReason, sinkBlocks, withoutNodes } from '$lib/run/cant-run';
	import PlanTimings from '$lib/run/plan-timings.svelte';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { checkOutcomeLabel, checkOutcomeTone } from '$lib/status';
	import { changedModelKeys, cloneCommands, jobRefreshText, loadedRows } from '$lib/run/plan-text';
	import CopyButton from '$lib/components/copy-button.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { useChecks } from '$lib/state/checks.svelte';
	import { useCatalogNodes } from '$lib/state/catalog.svelte';

	const snapshotNodes = createResource((client, scope) => client.listSnapshotNodes(scope), ['snapshot']);
	const catalog = useCatalogNodes();
	const checkList = useChecks();
	// A `--changed` plan names the job whose own run refreshes what it holds back, with that job's schedule.
	const triggerList = createResource((client, scope) => client.listTriggers(scope), ['triggers']);
	const schedulerStopped = $derived(app.platformEvents.some((e) => e.active && e.kind === 'scheduler_no_leader'));
	const refresh = (job: string) => jobRefreshText(job, triggerList.data ?? [], app.now, schedulerStopped);

	let cmd = $state(page.url.searchParams.get('cmd') ?? 'sqb build');
	let cmdEl = $state<HTMLInputElement>();
	/** The job this run belongs to; the command then only narrows it. */
	let job = $state<string | null>(page.url.searchParams.get('job'));
	/** The plan section shown, kept in the address with the command. */
	let section = $state<string | null>(page.url.searchParams.get('section'));

	const typed = $derived(parseCommand(cmd));
	/** For a job run, an option still being typed is left out until it is finished. */
	const settled = $derived(job ? settledJobCommand(typed) : { parts: typed, typing: false });
	const parts = $derived(settled.parts);
	const verb = $derived(parts.verb);
	const jobProblem = $derived(job ? jobRunProblem(parts) : null);
	const request = $derived<AdhocRunRequest>({ command: job ? jobBuildTokens(job, parts) : commandTokens(cmd), priority: 0 });
	const requestKey = $derived(JSON.stringify(request));
	const mayRun = $derived(app.can(job ? 'job.run' : 'run.adhoc'));
	const fullRefresh = $derived(parts.flags['--full-refresh'] === true);
	/** The date range a job run reprocesses, kept in the command as its cursor-window flags. */
	const runWindow = $derived(windowOf(parts));
	const jobDetail = createResource((client, scope) => (job ? client.getJob(scope, job) : Promise.resolve(null)), ['jobs', 'snapshot']);
	/** A job run's permissions, which the server checks when Run is pressed; the dry run only checks ad-hoc ones. */
	const jobPermissions = $derived(
		job
			? [
					{ action: 'job.run' as const, allowed: app.can('job.run'), reason: null },
					...(fullRefresh || runWindow ? [{ action: 'run.rewrite_history' as const, allowed: app.can('run.rewrite_history'), reason: null }] : [])
				]
			: []
	);

	/** Drops the job and keeps what the command selected, as an ad-hoc command. */
	function detachJob() {
		if (!job) return;
		cmd = `sqb ${jobBuildTokens(job, typed).join(' ')}`;
		job = null;
	}

	// The command stays in the address, so a plan can be shared or reopened.
	$effect(() => {
		const href = `/plan?cmd=${encodeURIComponent(cmd.trim())}${job ? `&job=${encodeURIComponent(job)}` : ''}${section ? `&section=${encodeURIComponent(section)}` : ''}`;
		const timer = setTimeout(() => {
			if (appPath(page.url.pathname) === '/plan' && appPath(page.url.pathname) + page.url.search !== href) void goto(to(href), { replaceState: true, keepFocus: true, noScroll: true });
		}, 400);
		return () => clearTimeout(timer);
	});

	// ---- server dry run (debounced; nothing is queued or recorded) ----
	let check = $state<AdhocRunCheck | null>(null);
	let checkedKey = $state('');
	let checking = $state(false);
	let checkError = $state<string | null>(null);
	let seq = 0;
	$effect(() => {
		const key = requestKey;
		const body = request;
		const scope = app.scope;
		if (!scope) return;
		const mine = ++seq;
		checking = true;
		const timer = setTimeout(async () => {
			try {
				const result = await app.client.checkAdhocRun(scope, body);
				if (mine !== seq) return;
				check = result;
				checkedKey = key;
				checkError = null;
				if (result.verb && !result.error) shown = result;
			} catch (e) {
				if (mine === seq) checkError = e instanceof Error ? e.message : String(e);
			} finally {
				if (mine === seq) checking = false;
			}
		}, 250);
		return () => clearTimeout(timer);
	});
	const fresh = $derived(!!check && checkedKey === requestKey && !checking);
	/** The last command that checked out; it stays on screen, dimmed, while the command is wrong or changing. */
	let shown = $state<AdhocRunCheck | null>(null);

	// ---- the location's plan of a build (one at a time; the latest command wins) ----
	let plan = $state<PlanDocument | null>(null);
	/** The server's plan request, which pages the plan's large sections. */
	let planRequestId = $state<string | null>(null);
	let plannedKey = $state('');
	let planning = $state(false);
	let planError = $state<{ key: string; message: string } | null>(null);
	/** Set by "Plan again": the next plan skips the plan the location kept for the same command. */
	let planFresh = false;
	const wantsPlan = $derived(fresh && check?.verb === 'build' && !check.error);
	$effect(() => {
		const key = requestKey;
		const scope = app.scope;
		if (!scope || !wantsPlan || planning || plannedKey === key || planError?.key === key) return;
		const timer = setTimeout(async () => {
			planning = true;
			try {
				const replanned = planFresh;
				planFresh = false;
				const result = await app.client.planBuild(scope, request.command, { fresh: replanned });
				if (result.plan) {
					plan = result.plan;
					planRequestId = result.requestId ?? null;
					plannedKey = key;
					planError = null;
				} else planError = { key, message: result.error };
			} catch (e) {
				planError = { key, message: e instanceof Error ? e.message : String(e) };
			} finally {
				planning = false;
			}
		}, 400);
		return () => clearTimeout(timer);
	});
	/** The next page of a large plan section, added to the plan shown. */
	async function morePlan(paged: PlanPageSection, limit: number) {
		const scope = app.scope;
		const requestId = planRequestId;
		if (!scope || !plan || !requestId) return;
		const page = await app.client.planRows(scope, requestId, paged, loadedRows(plan, paged), limit);
		if (planRequestId === requestId && plan) plan = mergePlanPage(plan, paged, page);
	}
	/** A page of a plan section's rows whose name contains `name`, for the plan list's filter. */
	async function findPlanRows(paged: PlanPageSection, offset: number, limit: number, name: string) {
		const scope = app.scope;
		const requestId = planRequestId;
		if (!scope || !requestId) throw new Error('The plan has expired; plan again.');
		return app.client.planRows(scope, requestId, paged, offset, limit, name);
	}
	/** A model's diff in the plan, read when its row opens it; null when the server has none to give. */
	async function planDiff(model: string) {
		const scope = app.scope;
		const requestId = planRequestId;
		if (!scope || !requestId) return null;
		return app.client.planDiff(scope, requestId, model);
	}
	function replan() {
		planFresh = true;
		plannedKey = '';
		planError = null;
	}
	const planCurrent = $derived(plannedKey === requestKey && !planning);

	// ---- what a non-build verb picks, from the catalog ----
	const byKey = $derived(new Map((catalog.data ?? []).map((n) => [n.key, n])));
	const picked = $derived(shown?.selection ?? []);
	let view = $state<'plan' | 'graph' | 'checks'>('plan');
	/** The most rows one read brings, as the server pages them. */
	const ROWS_READ = 1000;
	/** Under `--changed`, the nodes it skips: held back or unchanged, read from a server plan's pages when the Checks tab opens. */
	let skippedKeys = $state<{ requestId: string; keys: string[] | null; failed?: boolean } | null>(null);
	$effect(() => {
		const scope = app.scope;
		const requestId = planRequestId;
		const sections = plan?.changed ? plan.sections : undefined;
		if (view !== 'checks' || !scope || !requestId || !sections || skippedKeys?.requestId === requestId) return;
		skippedKeys = { requestId, keys: null };
		(async () => {
			const keys: string[] = [];
			for (const s of sections.filter((c) => (c.section === 'held_back' || c.section === 'unchanged') && c.total)) {
				for (let offset = 0; offset < s.total; offset += ROWS_READ) {
					const page = await app.client.planRows(scope, requestId, s.section, offset, ROWS_READ);
					keys.push(...(page.rows as (string | { node: string })[]).map((r) => (typeof r === 'string' ? r : r.node)));
				}
			}
			if (planRequestId === requestId) skippedKeys = { requestId, keys };
		})().catch(() => {
			// Not retried: a plan whose rows can't be read has usually expired.
			if (planRequestId === requestId) skippedKeys = { requestId, keys: null, failed: true };
		});
	});
	/** Node keys `--changed` skips; the default mock's whole plan lists them, a server plan's are read above (null until then). */
	const planSkipped = $derived.by(() => {
		if (!plan?.changed) return [];
		if (!plan.sections) return [...plan.changed.held_back.map((h) => h.node), ...plan.changed.unchanged];
		return skippedKeys?.requestId === planRequestId ? skippedKeys.keys : null;
	});
	const keyName = (key: string) => key.slice(key.indexOf(':') + 1);
	/** The recorded checks on the models and seeds the command selects, as the plan request froze the selection. */
	const selectedChecks = $derived.by(() => {
		if (!plan) return [];
		const selected = new Set(picked.map(keyName));
		return (checkList.data ?? []).filter((c) => (c.attachment === 'model' || c.attachment === 'seed') && !!c.node && selected.has(c.node));
	});
	/** Of those, the checks on nodes the build runs, which run after them: all but those `--changed` skips (null until read). */
	const planChecks = $derived.by(() => {
		if (!planSkipped) return null;
		const skipped = new Set(planSkipped.map(keyName));
		return selectedChecks.filter((c) => !skipped.has(c.node ?? ''));
	});
	/** The tab shown: a replan with no checks leaves the Checks tab for the plan. */
	const tab = $derived(view === 'checks' && !(shown?.verb === 'build' && selectedChecks.length && planChecks?.length !== 0) ? 'plan' : view);
	/** A large selection draws only what the plan changes; the whole of it would be unreadable. */
	const GRAPH_LIMIT = 150;
	/** Sections of models the plan changes, read a page each for the graph of a large selection. */
	const CHANGED_MODEL_SECTIONS: PlanPageSection[] = ['full_refresh', 'query_changed', 'function_changed', 'config_changed', 'schema_changed', 'first_run', 'renamed'];
	let changedKeys = $state<{ requestId: string; keys: string[] } | null>(null);
	$effect(() => {
		const scope = app.scope;
		const requestId = planRequestId;
		const sections = plan?.sections;
		if (view !== 'graph' || picked.length <= GRAPH_LIMIT || !scope || !requestId || !sections || changedKeys?.requestId === requestId) return;
		changedKeys = { requestId, keys: [] };
		(async () => {
			const keys: string[] = [];
			for (const s of sections.filter((c) => CHANGED_MODEL_SECTIONS.includes(c.section) && c.total)) {
				if (keys.length >= GRAPH_LIMIT) break;
				const page = await app.client.planRows(scope, requestId, s.section, 0, GRAPH_LIMIT - keys.length);
				keys.push(...(page.rows as { name: string }[]).map((m) => `model:${m.name}`));
			}
			if (planRequestId === requestId) changedKeys = { requestId, keys };
		})().catch(() => {});
	});
	/** The models the plan changes: read from the server a section at a time, or from the default mock's whole plan's rows. */
	const planChanged = $derived(plan?.sections ? (changedKeys?.requestId === planRequestId ? changedKeys.keys : []) : plan ? changedModelKeys(plan) : []);
	const graphKeys = $derived(picked.length > GRAPH_LIMIT && plan ? planChanged : picked);

	// ---- autocomplete ----
	let caret = $state(0);
	let acOpen = $state(false);
	let acIndex = $state(0);
	const suggestions = $derived<Suggestion[]>(acOpen ? suggest(cmd, caret, snapshotNodes.data ?? []) : []);

	function track() {
		caret = cmdEl?.selectionStart ?? cmd.length;
		acOpen = true;
		acIndex = 0;
	}

	function pick(s: Suggestion) {
		const next = applySuggestion(cmd, caret, s.value);
		cmd = next.text;
		acOpen = false;
		queueMicrotask(() => {
			cmdEl?.focus();
			cmdEl?.setSelectionRange(next.caret, next.caret);
			caret = next.caret;
		});
	}

	function onKeydown(e: KeyboardEvent) {
		if (!acOpen || !suggestions.length) return;
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			acIndex = (acIndex + 1) % suggestions.length;
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			acIndex = (acIndex - 1 + suggestions.length) % suggestions.length;
		} else if (e.key === 'Enter' || e.key === 'Tab') {
			e.preventDefault();
			pick(suggestions[acIndex]);
		} else if (e.key === 'Escape') {
			acOpen = false;
		}
	}

	function setVerb(v: LaunchVerb) {
		cmd = withVerb(cmd, v);
		cmdEl?.focus();
	}

	/** Appends an option and lists the options that can follow, as typing `--` does. */
	function insertFlag(flag: string) {
		cmd = `${cmd.trimEnd()} ${flag}`;
		queueMicrotask(() => {
			cmdEl?.focus();
			cmdEl?.setSelectionRange(cmd.length, cmd.length);
			track();
		});
	}

	let copied = $state(false);
	async function copy() {
		const command = job ? `sqb ${request.command.join(' ')}` : cmd.trim();
		copied = await copyText(`${command} --target ${app.target?.name ?? ''}`);
		setTimeout(() => (copied = false), 1500);
	}

	// ---- run ----
	let launching = $state(false);
	let launchError = $state<string | null>(null);
	const planned = $derived(check?.verb !== 'build' || planCurrent);
	const allowed = $derived(job ? jobPermissions.every((p) => p.allowed) : !!check?.launchable);
	/** Nodes the current plan can't run because a provider is missing secrets; Run waits until none are left. */
	const cantRun = $derived(planCurrent && check?.verb === 'build' ? (plan?.cant_run ?? []) : []);
	const blockedNodes = $derived(cantRunKeys(cantRun));
	const canLaunch = $derived(fresh && !check?.error && !jobProblem && !settled.typing && allowed && planned && !cantRun.length);
	const problem = $derived(jobProblem ?? checkError ?? (fresh ? check?.error : null) ?? (planError?.key === requestKey ? planError.message : null));
	/** One loading state over the whole result, from the first check until the plan arrives, so the page doesn't rebuild itself in steps. */
	const waiting = $derived<'check' | 'plan' | null>(
		problem ? null : !shown ? 'check' : shown.verb === 'build' && (planning || (wantsPlan && !planCurrent && !planError)) ? 'plan' : null
	);
	const blockedReason = $derived(
		!mayRun
			? job
				? "You don't have permission to run jobs in this target (job.run)"
				: "You don't have permission to start runs by hand in this target (run.adhoc)"
			: !fresh
				? undefined
				: (check?.error ??
					(job
						? jobPermissions.find((p) => !p.allowed) && "You don't have permission to rebuild or reprocess data (run.rewrite_history)"
						: check?.permissions.find((p) => !p.allowed)?.reason) ??
					(planned
						? settled.typing
							? 'Finish the option you are typing'
							: cantRun.length
								? cantRunReason(cantRun)
								: undefined
						: planError?.key === requestKey
							? planError.message
							: 'Waiting for the plan'))
	);

	/** The banner shows only problems that stop this run or touch what it builds; the rest stay on the Overview. */
	// The command's selection names every relation the plan can build; the loaded rows add their warehouse names.
	const planRelations = $derived(new Set([...(plan?.models ?? []).flatMap((m) => [m.name, m.qualified_name?.split('.').at(-1)]), ...picked.map((key) => key.slice(key.indexOf(':') + 1))].filter((n): n is string => !!n).map((n) => n.toLowerCase())));
	function affectsPlan(event: PlatformEvent): boolean {
		if (event.kind === 'scheduler_no_leader') return true;
		if (event.locationId !== app.location?.id) return false;
		if (event.kind === 'location_broken') return true;
		if (event.kind === 'launch_failed') return !!job && event.subject === job;
		if (event.kind === 'warehouse_sync_failed') return !!event.subject && planRelations.has(event.subject.toLowerCase().split('.').at(-1)!);
		return false;
	}

	async function launch() {
		const scope = app.scope;
		if (!scope || !canLaunch) return;
		launching = true;
		launchError = null;
		try {
			const narrowing = { select: parts.select, exclude: parts.exclude, window: runWindow ?? undefined };
			const run = job
				? await (fullRefresh ? app.client.launchJobFromScratch(scope, job, narrowing) : app.client.launchJob(scope, job, narrowing))
				: await app.client.launchAdhocRun(scope, request);
			await goto(to(`/runs/${run.id}`));
		} catch (e) {
			launchError = e instanceof Error ? e.message : String(e);
		} finally {
			launching = false;
		}
	}
	const checkColumns: TableColumn[] = [
		{ id: 'check', label: 'Check', minWidth: 200 },
		{ id: 'node', label: 'Node', width: '30%' },
		{ id: 'severity', label: 'Severity', width: '100px' },
		{ id: 'last', label: 'Last result', width: '150px' }
	];
	const nodeColumns: TableColumn[] = [
		{ id: 'node', label: 'Node', minWidth: 200 },
		{ id: 'kind', label: 'Kind', width: '30%' },
		{ id: 'built', label: 'Last built', width: '150px' }
	];
</script>

<svelte:head><title>Plan · {cmd.trim() || 'sqb'}</title></svelte:head>

<AppTopbar title="Plan" events={affectsPlan} />

<div class="flex min-h-0 flex-1 flex-col">
	<div class="px-4 pt-3 empty:hidden"><SyncingNote what="Plan previews" /></div>
	<div class="cmd">
		{#if job}
			<span class="jobchip" data-testid="plan-job">
				<span class="faint">Job</span><a href={to(`/jobs/${encodeURIComponent(job)}`)}>{job}</a>
				<Button variant="ghost" size="icon-xs" onclick={detachJob} title="Run the command on its own, not as part of {job}" aria-label="Remove the job"><XIcon /></Button>
			</span>
		{:else}
			<SegmentedFilter label="Command" value={verb} options={VERBS.map((v) => ({ value: v.name, label: v.name, title: v.description }))} onselect={setVerb} />
		{/if}
		<div class="line">
			<span class="faint">$</span>
			<input
				bind:this={cmdEl}
				bind:value={cmd}
				oninput={track}
				onclick={track}
				onkeyup={(e) => (e.key === 'ArrowLeft' || e.key === 'ArrowRight') && track()}
				onkeydown={onKeydown}
				onblur={() => setTimeout(() => (acOpen = false), 120)}
				spellcheck="false"
				aria-label="Command"
				placeholder={job ? 'sqb build --exclude orders' : 'sqb build --select +orders'}
			/>
			<Button variant="ghost" size="sm" onclick={() => insertFlag('--')} title="Add an option to the command"><PlusIcon />Option</Button>
			<Button variant="ghost" size="sm" onclick={copy} title="Copy the command, to run it with sqb yourself"><CopyIcon />{copied ? 'Copied' : 'Copy'}</Button>
			{#if acOpen && suggestions.length}
				<div class="ac">
					{#each suggestions as s, i (s.value)}
						<!-- raw-button: an option in the command's autocomplete list -->
						<button class:on={i === acIndex} onmouseenter={() => (acIndex = i)} onmousedown={(e) => (e.preventDefault(), pick(s))}>
							<code>{s.label}</code><span>{s.hint}</span>
						</button>
					{/each}
				</div>
			{/if}
		</div>
	</div>

	{#if job && jobDetail.data}
		<div class="data"><span class="faint">Data</span><DataRange cursorType={jobDetail.data.cursorType} window={runWindow} {fullRefresh} onchange={(c) => (cmd = withFullRefresh(withWindow(cmd, c.window), c.fullRefresh))} /></div>
	{/if}

	{#if problem}
		<div class="problem-bar" data-testid="adhoc-error">
			<p>{problem}</p>
			{#each cloneCommands(problem) as clone (clone.command)}<CopyButton value={clone.command} label="Copy clone command" text="copy clone from {clone.origin}" />{/each}
		</div>
	{/if}

	<div class="body">
		<div class="stage">
		<PageBody class={!!shown && (!!problem || !fresh || (planning && !!plan)) ? 'plan-main stale' : 'plan-main'} padding="14px 24px" inner="flex min-h-full flex-col">
			{#if shown?.verb}
				<div class="summary" class:capped={shown.verb === 'build' && tab !== 'graph'}>
					{#if shown.verb !== 'build'}
						<b>{picked.length}</b> {noun(picked.length, 'node')}{#if shown.writeSet.length}<span>·</span> updates <b>{shown.writeSet.length}</b>{/if}
					{/if}
					{#each Object.entries(shown.settings) as [name, value] (name)}<span class="sb-tag code" title="Option">{value === true ? name : `${name}=${value}`}</span>{/each}
					{#if shown.verb === 'build' && plan?.ignore_audit_failures}<span class="audits-note" data-testid="plan-ignores-audit-failures" title="An error-severity audit that fails is recorded, and the build goes on">Audit failures won't stop this build</span>{/if}
					<div class="tabs">
						<SegmentedFilter label="View" value={tab} options={[{ value: 'plan' as const, label: shown.verb === 'build' ? 'Plan' : 'Nodes' }, { value: 'graph' as const, label: 'Graph' }, ...(shown.verb === 'build' && selectedChecks.length && planChecks?.length !== 0 ? [{ value: 'checks' as const, label: 'Checks', ...(planChecks ? { count: planChecks.length } : {}) }] : [])]} onselect={(v) => (view = v)} />
					</div>
					{#if shown.verb === 'build' && plan}
						{#if plan.timings}<span class="ml-auto"><PlanTimings {plan} timings={plan.timings} now={app.now} /></span>{/if}
						<Button variant="ghost" size="sm" class={plan.timings ? '' : 'ml-auto'} onclick={replan} disabled={planning}><RotateCwIcon class={planning ? 'animate-spin' : ''} />{planning ? 'Planning…' : 'Plan again'}</Button>
					{/if}
				</div>
			{/if}
			{#if tab === 'graph' && shown?.verb && catalog.data && (shown.verb !== 'build' || plan)}
				{#if graphKeys.length < picked.length}
					<p class="faint graph-note">{picked.length} nodes are too many to draw, so the graph shows only the {graphKeys.length === 1 ? 'model' : `${graphKeys.length} models`} this plan changes.</p>
				{/if}
				{#if graphKeys.length}
					<div class="graph"><ScopeGraph keys={graphKeys} catalog={catalog.data} onselect={() => {}} /></div>
				{/if}
			{:else if tab === 'checks' && shown?.verb === 'build' && plan}
				<div class="checks">
				<p class="faint graph-note">{planChecks ? plural(planChecks.length, 'check') : skippedKeys?.failed ? "Couldn't read the nodes --changed skips; plan again." : 'Reading the nodes --changed skips…'} <HelpTip text="Checks run after the models they check, and a failing error check stops what depends on it." /></p>
				<DataTable class="static compact" embedded columns={checkColumns} rows={planChecks ?? []} key={(c) => c.id} empty="No checks in this plan.">
					{#snippet row(c)}
						<tr>
							<td><a href={to(`/checks/${encodeURIComponent(c.id)}`)} class="mono">{c.name}</a></td>
							<td class="faint mono">{c.node ? c.node.slice(c.node.indexOf(':') + 1) : ''}{c.column ? `.${c.column}` : ''}</td>
							<td class="faint">{c.severity}</td>
							<td>{#if c.latest}<StatusLabel tone={checkOutcomeTone[c.latest.outcome]} label="last {checkOutcomeLabel[c.latest.outcome].toLowerCase()}" />{/if}</td>
						</tr>
					{/snippet}
				</DataTable>
				</div>
			{:else if shown?.verb === 'build' && plan}
				<PlanBrowser
					{plan}
					{refresh}
					{section}
					onselect={(key) => (section = key)}
					more={planRequestId ? morePlan : undefined}
					load={planRequestId ? findPlanRows : undefined}
					diff={planRequestId ? planDiff : undefined}
					kindOf={(key) => {
						const node = byKey.get(key);
						return node ? kindLine(node) : '';
					}}
					upstreamOf={(key) => byKey.get(key)?.upstream ?? []}
				/>
			{:else if shown?.verb && shown.verb !== 'build'}
				<DataTable class="static compact" embedded columns={nodeColumns} rows={picked} key={(key) => key} rowHeight={32} empty="No nodes selected.">
					{#snippet row(key)}
						{@const node = byKey.get(key)}
						<tr>
							<td><a href={to(`/catalog/${key}`)} class="mono">{key.slice(key.indexOf(':') + 1)}</a></td>
							<td class="faint">{node ? kindLine(node) : ''}</td>
							<td class="faint">{node?.lastBuild ? `built ${formatAgo(node.lastBuild.builtAt, app.now)}` : ''}</td>
						</tr>
					{/snippet}
				</DataTable>
			{/if}
		</PageBody>

		{#if waiting}
			<div class="overlay" role="status" data-testid="plan-loading">
				<div class="spinner-card">
					<RotateCwIcon size={16} class="animate-spin" />
					<div>
						<div>{waiting === 'plan' ? 'Planning on the location…' : 'Checking the command…'}</div>
					</div>
				</div>
			</div>
		{/if}
		</div>

	</div>

	<div class="foot">
		<div class="why">
			{#if launchError}<span class="problem">{launchError}</span>{:else if !canLaunch && blockedReason && blockedReason !== problem}<span class="faint">{blockedReason}</span>{/if}
		</div>
		{#if blockedNodes.length && !sinkBlocks(cantRun)}
			<Button variant="outline" size="sm" data-testid="plan-without-blocked" onclick={() => (cmd = withoutNodes(cmd, blockedNodes))}>Plan without {blockedNodes.length === 1 ? 'this node' : `these ${blockedNodes.length} nodes`}</Button>
		{/if}
		<GatedButton requires={job ? 'job.run' : 'run.adhoc'} variant={canLaunch ? 'default' : 'outline'} disabled={!canLaunch} reason={blockedReason} busy={launching} onclick={launch}><PlayIcon class="size-3.5 fill-current stroke-none" />Run</GatedButton>
	</div>
</div>

<style>
	.cmd {
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 10px 24px;
		border-bottom: 1px solid var(--border);
	}
	.jobchip {
		display: inline-flex;
		flex-shrink: 0;
		align-items: center;
		gap: 6px;
		padding: 3px 4px 3px 9px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--sidebar-accent);
		font-size: 11.5px;
	}
	.jobchip a {
		font-family: var(--font-mono);
		color: var(--foreground);
	}
	.line {
		position: relative;
		display: flex;
		min-width: 0;
		flex: 1;
		align-items: center;
		gap: 10px;
		padding: 1px 2px 1px 10px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--sb-inset);
	}
	.line:focus-within {
		border-color: var(--primary);
	}
	.line input {
		min-width: 0;
		flex: 1;
		background: transparent;
		color: var(--foreground);
		font-family: var(--font-mono);
		font-size: 12.5px;
		outline: none;
	}
	.ac {
		position: absolute;
		top: 100%;
		left: 0;
		z-index: 60;
		width: 520px;
		max-height: 300px;
		overflow: auto;
		margin-top: 4px;
		padding: 4px 0;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--popover);
		box-shadow: var(--sb-elev);
	}
	.ac button {
		display: flex;
		width: 100%;
		align-items: baseline;
		gap: 8px;
		padding: 5px 12px;
		text-align: left;
	}
	.ac button.on {
		background: var(--sb-hover);
	}
	.ac span {
		overflow: hidden;
		color: var(--sb-text-faint);
		font-size: 11px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.body {
		display: flex;
		min-height: 0;
		flex: 1;
	}
	.stage {
		position: relative;
		display: flex;
		min-width: 0;
		flex: 1;
	}
	:global(.plan-main) {
		transition: opacity 0.15s;
	}
	:global(.plan-main.stale) {
		opacity: 0.55;
	}
	.problem-bar {
		margin: 0;
		padding: 7px 24px;
		border-bottom: 1px solid color-mix(in srgb, var(--sb-error) 35%, var(--border));
		background: color-mix(in srgb, var(--sb-error) 7%, transparent);
		color: var(--sb-error);
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.problem-bar p {
		margin: 0;
		white-space: pre-wrap;
	}
	.overlay {
		position: absolute;
		inset: 0;
		z-index: 5;
		display: grid;
		place-items: center;
		background: color-mix(in srgb, var(--background) 72%, transparent);
	}
	.spinner-card {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 14px 18px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--card);
		box-shadow: var(--sb-elev);
		font-size: 12.5px;
	}
	.summary {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		margin-bottom: 10px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	/* A build's toolbar ends where its plan does: the index, the gap and the capped pane (plan-browser). */
	.summary.capped,
	.checks {
		max-width: calc(230px + 24px + 1100px);
	}
	.summary b {
		color: var(--foreground);
		font-weight: 600;
	}
	.summary span {
		color: var(--sb-text-faint);
	}
	.data {
		display: flex;
		align-items: flex-start;
		gap: 14px;
		padding: 8px 24px;
		border-bottom: 1px solid var(--border);
	}
	.data > .faint {
		padding-top: 3px;
	}
	.tabs {
		margin-left: 12px;
	}
	.tabs:first-child {
		margin-left: 0;
	}
	.graph {
		min-height: 320px;
		flex: 1;
		border: 1px solid var(--border);
		border-radius: 2px;
	}
	.graph-note {
		margin: 0 0 8px;
	}
	.foot {
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 10px 24px;
		border-top: 1px solid var(--border);
		background: var(--card);
	}
	.why {
		min-width: 0;
		flex: 1;
		font-size: 12.5px;
	}
	code,
	.mono {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.faint {
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	.problem {
		color: var(--sb-error);
	}
	a:hover {
		color: var(--primary);
	}
	.audits-note {
		padding: 1px 8px;
		border-radius: 999px;
		background: color-mix(in srgb, var(--sb-warning) 13%, transparent);
		color: color-mix(in srgb, var(--sb-warning) 75%, var(--foreground));
		font-size: 12px;
		white-space: nowrap;
	}
</style>
