<script lang="ts">
	import { appPath, to } from '$lib/paths';
	// Every Run button opens this page with its command. A build is planned by the location against the
	// warehouse (`sqb plan`); other verbs show the server's dry run. Nothing is queued until Run is pressed.
	// With a job, the command only narrows that job: Run starts a run of the job, not an ad-hoc command.
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import CopyIcon from '@lucide/svelte/icons/copy';
	import RotateCwIcon from '@lucide/svelte/icons/rotate-cw';
	import type { AdhocRunCheck, AdhocRunRequest, LaunchVerb, PlanDocument, PlanModel } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import { kindLine } from '$lib/build-area/labels';
	import { formatAgo } from '$lib/format/time';
	import ScopeGraph from '$lib/lineage/scope-graph.svelte';
	import { PERMISSIONS } from '$lib/platform/permission-catalog';
	import { VERBS } from '$lib/run/command-table';
	import { applySuggestion, commandTokens, jobBuildTokens, jobRunProblem, parseCommand, suggest, withVerb, type Suggestion } from '$lib/run/command';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';

	const snapshotNodes = createResource((client, scope) => client.listSnapshotNodes(scope), ['snapshot']);
	const catalog = createResource((client, scope) => client.listCatalogNodes(scope), ['snapshot', 'runs', 'node_status']);

	let cmd = $state(page.url.searchParams.get('cmd') ?? 'sqb build');
	let cmdEl = $state<HTMLInputElement>();
	/** The job this run belongs to; the command then only narrows it. */
	let job = $state<string | null>(page.url.searchParams.get('job'));

	const parts = $derived(parseCommand(cmd));
	const verb = $derived(parts.verb);
	const jobProblem = $derived(job ? jobRunProblem(parts) : null);
	const request = $derived<AdhocRunRequest>({ command: job ? jobBuildTokens(job, parts) : commandTokens(cmd), priority: 0 });
	const requestKey = $derived(JSON.stringify(request));
	const mayRun = $derived(app.can(job ? 'job.run' : 'run.adhoc'));
	const fullRefresh = $derived(parts.flags['--full-refresh'] === true);
	/** A job run's permissions, which the server checks when Run is pressed; the dry run only checks ad-hoc ones. */
	const jobPermissions = $derived(
		job
			? [
					{ action: 'job.run' as const, allowed: app.can('job.run'), reason: null },
					...(fullRefresh ? [{ action: 'run.rewrite_history' as const, allowed: app.can('run.rewrite_history'), reason: null }] : [])
				]
			: []
	);

	/** Drops the job and keeps what the command selected, as an ad-hoc command. */
	function detachJob() {
		if (!job) return;
		cmd = `sqb ${jobBuildTokens(job, parts).join(' ')}`;
		job = null;
	}

	// The command stays in the address, so a plan can be shared or reopened.
	$effect(() => {
		const href = `/plan?cmd=${encodeURIComponent(cmd.trim())}${job ? `&job=${encodeURIComponent(job)}` : ''}`;
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
	let plannedKey = $state('');
	let planning = $state(false);
	let planError = $state<{ key: string; message: string } | null>(null);
	const wantsPlan = $derived(fresh && check?.verb === 'build' && !check.error);
	$effect(() => {
		const key = requestKey;
		const scope = app.scope;
		if (!scope || !wantsPlan || planning || plannedKey === key || planError?.key === key) return;
		const timer = setTimeout(async () => {
			planning = true;
			try {
				const result = await app.client.planBuild(scope, request.command);
				if (result.plan) {
					plan = result.plan;
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
	function replan() {
		plannedKey = '';
		planError = null;
	}
	const planCurrent = $derived(plannedKey === requestKey && !planning);

	const REASONS: { value: string; label: string; tone: string }[] = [
		{ value: 'source_freshness_error', label: 'Source freshness error', tone: 'var(--sb-error)' },
		{ value: 'external_upstream_failed', label: 'Upstream failed', tone: 'var(--sb-error)' },
		{ value: 'full_refresh', label: 'Full refresh', tone: 'var(--sb-warning)' },
		{ value: 'query_changed', label: 'Query changed', tone: 'var(--sb-warning)' },
		{ value: 'function_changed', label: 'Function changed', tone: 'var(--sb-warning)' },
		{ value: 'config_changed', label: 'Config changed', tone: 'var(--sb-warning)' },
		{ value: 'schema_changed', label: 'Schema changed', tone: 'var(--sb-warning)' },
		{ value: 'first_run', label: 'First run', tone: 'var(--sb-warning)' },
		{ value: 'renamed', label: 'Renamed', tone: 'var(--sb-warning)' },
		{ value: 'normal_incremental', label: 'Incremental', tone: 'var(--primary)' },
		{ value: 'no_change', label: 'Unchanged', tone: 'var(--sb-success)' },
		{ value: 'disabled', label: 'Disabled', tone: 'var(--sb-text-faint)' }
	];
	const QUIET = new Set(['no_change', 'disabled']);
	const ACTIONS: Record<string, string> = {
		create_view: 'create view',
		create_table: 'create table',
		incremental_append: 'append',
		incremental_delete_insert: 'delete + insert',
		incremental_merge: 'merge',
		load_seed: 'load seed'
	};
	const reasonGroups = $derived.by(() => {
		const models = plan?.models ?? [];
		const known = new Set(REASONS.map((r) => r.value));
		const extra = [...new Set(models.map((m) => m.reason).filter((r) => !known.has(r)))].map((r) => ({ value: r, label: r.replaceAll('_', ' '), tone: 'var(--sb-text-faint)' }));
		return [...REASONS, ...extra].map((r) => ({ ...r, models: models.filter((m) => m.reason === r.value) })).filter((g) => g.models.length);
	});
	const toBuild = $derived((plan?.models ?? []).filter((m) => !QUIET.has(m.reason)).length);
	function detail(m: PlanModel): string {
		const parts: string[] = [];
		if (m.backfill.action === 'full') parts.push('full backfill');
		if (m.backfill.action === 'bounded') parts.push(`backfill ${m.backfill.duration ?? ''}`.trim());
		if (m.cursor_column) {
			const b = m.cursor_bounds;
			parts.push(b && (b.start || b.end) ? `${m.cursor_column} ${b.start ?? '…'} → ${b.end ?? '…'}` : `cursor ${m.cursor_column}`);
		}
		if (m.changed_functions?.length) parts.push(`uses ${m.changed_functions.join(', ')}`);
		return parts.join(' · ');
	}
	const extras = $derived(
		plan
			? [
					{ title: 'Sources to load', rows: plan.source_loads.map((r) => [r.name, [r.write_strategy, r.cursor_column && `cursor ${r.cursor_column}`, r.is_reload && 'reload'].filter(Boolean).join(' · ')]) },
					{ title: 'Seeds', rows: plan.seeds.map((r) => [r.name, r.reason.replaceAll('_', ' ')]) },
					{ title: 'Functions', rows: plan.functions.map((r) => [r.name, [r.language, r.reason?.replaceAll('_', ' ')].filter(Boolean).join(' · ')]) },
					{ title: 'Python', rows: plan.python_nodes.map((r) => [r.name, `${r.kind} · ${r.identity_status}`]) },
					{ title: 'Migrations', rows: plan.migrations.map((r) => [r.model, [r.origin_model && `from ${r.origin_model}`, r.decision, r.completed_at ? 'completed' : 'pending'].filter(Boolean).join(' · ')]) },
					{ title: 'Column migrations', rows: plan.column_migrations.map((r) => [r.model, `${r.origin_column} → ${r.destination_column} · ${r.decision}`]) },
					{ title: 'Table type changes', rows: plan.table_type_conversions.map((r) => [r.model, [r.actual_type, r.desired_type].filter(Boolean).join(' → ')]) },
					{ title: 'Retention', rows: plan.retention.map((r) => [r.model, r.action ?? '']) }
				].filter((x) => x.rows.length)
			: []
	);

	// ---- what a non-build verb picks, from the catalog ----
	const byKey = $derived(new Map((catalog.data ?? []).map((n) => [n.key, n])));
	const picked = $derived(shown?.selection ?? []);
	const graphHeight = $derived(picked.length <= 3 ? 190 : picked.length <= 8 ? 270 : 360);

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
		await navigator.clipboard.writeText(`${command} --target ${app.target?.name ?? ''}`);
		copied = true;
		setTimeout(() => (copied = false), 1500);
	}

	// ---- run ----
	let launching = $state(false);
	let launchError = $state<string | null>(null);
	const planned = $derived(check?.verb !== 'build' || planCurrent);
	const allowed = $derived(job ? jobPermissions.every((p) => p.allowed) : !!check?.launchable);
	const canLaunch = $derived(fresh && !check?.error && !jobProblem && allowed && planned);
	const problem = $derived(jobProblem ?? checkError ?? (fresh ? check?.error : null) ?? (planError?.key === requestKey ? planError.message : null));
	const blockedReason = $derived(
		!mayRun
			? job
				? "You don't have permission to run jobs in this target (job.run)"
				: "You don't have permission to start runs by hand in this target (run.adhoc)"
			: !fresh
				? undefined
				: (check?.error ??
					(job
						? jobPermissions.find((p) => !p.allowed) && "You don't have permission to rebuild from scratch (run.rewrite_history)"
						: check?.permissions.find((p) => !p.allowed)?.reason) ??
					(planned ? undefined : (planError?.message ?? 'Waiting for the plan')))
	);

	async function launch() {
		const scope = app.scope;
		if (!scope || !canLaunch) return;
		launching = true;
		launchError = null;
		try {
			const narrowing = { select: parts.select, exclude: parts.exclude };
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
</script>

<AppTopbar title="Plan" />

<div class="flex min-h-0 flex-1 flex-col">
	<div class="cmd">
		{#if job}
			<span class="jobchip" data-testid="plan-job">
				<span class="faint">Job</span><a href={to(`/jobs/${encodeURIComponent(job)}`)}>{job}</a>
				<button onclick={detachJob} title="Run the command on its own, not as part of {job}" aria-label="Remove the job">×</button>
			</span>
		{:else}
			<div class="verbs" role="group" aria-label="Command">
				{#each VERBS as v (v.name)}
					<button class:on={verb === v.name} onclick={() => setVerb(v.name)} title={v.description}>{v.name}</button>
				{/each}
			</div>
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
			<button class="copy" onclick={() => insertFlag('--')} title="Add an option to the command">＋ Option</button>
			<button class="copy" onclick={copy} title="Copy the command, to run it with sqb yourself"><CopyIcon size={12} /> {copied ? 'Copied' : 'Copy'}</button>
			{#if acOpen && suggestions.length}
				<div class="ac">
					{#each suggestions as s, i (s.value)}
						<button class:on={i === acIndex} onmouseenter={() => (acIndex = i)} onmousedown={(e) => (e.preventDefault(), pick(s))}>
							<code>{s.label}</code><span>{s.hint}</span>
						</button>
					{/each}
				</div>
			{/if}
		</div>
	</div>

	{#if problem}<p class="problem-bar" data-testid="adhoc-error">{problem}</p>{/if}

	<div class="body">
		<div class="main sb-scroll" class:stale={!!shown && (!!problem || !fresh || (planning && !!plan))}>
			{#if shown?.verb === 'build'}
				{#if plan}
					<div class="summary">
						<b>{plan.selected_count}</b> selected <span>·</span> <b>{toBuild}</b> to build
						{#if plan.models.length - toBuild}<span>·</span> {plan.models.length - toBuild} unchanged{/if}
						{#if plan.warnings.length}<span>·</span> <i style:background="var(--sb-warning)"></i>{plan.warnings.length} warning{plan.warnings.length === 1 ? '' : 's'}{/if}
						<button class="replan" onclick={replan} disabled={planning}><RotateCwIcon size={11} class={planning ? 'animate-spin' : ''} /> {planning ? 'Planning…' : 'Plan again'}</button>
					</div>
					{#if plan.warnings.length}
						<ul class="warnings">
							{#each plan.warnings as w, i (i)}
								<li class:err={w.severity === 'error'}>{#if w.code}<code>{w.code}</code>{/if}{w.message}</li>
							{/each}
						</ul>
					{/if}
				{:else if planning || (wantsPlan && !planError)}
					<div class="planning" role="status" data-testid="plan-loading">
						<RotateCwIcon size={13} class="animate-spin" />
						<div>
							<div>Planning on the location…</div>
							<div class="faint">Reading the current warehouse state</div>
						</div>
					</div>
				{/if}
			{:else if shown?.verb}
				<div class="summary"><b>{picked.length}</b> node{picked.length === 1 ? '' : 's'}{#if shown.writeSet.length}<span>·</span> updates <b>{shown.writeSet.length}</b>{/if}</div>
			{:else if checking && !problem}
				<p class="msg faint">Checking the command…</p>
			{/if}
			{#if shown?.verb === 'build' && plan}
				<table class="nodes">
					{#each reasonGroups as g (g.value)}
						<tbody>
							<tr class="group"><td colspan="5"><i style:background={g.tone}></i>{g.label} · {g.models.length}</td></tr>
							{#each g.models as m (m.name)}
								<tr>
									<td><a href={to(`/catalog/model:${m.name}`)} class="mono">{m.name}</a></td>
									<td class="faint">{m.materialization_type}{m.incremental_strategy ? ` · ${m.incremental_strategy}` : ''}</td>
									<td>{ACTIONS[m.action] ?? m.action.replaceAll('_', ' ')}</td>
									<td class="faint wrap">{detail(m)}</td>
									<td class="faint right mono">{m.qualified_name ?? ''}</td>
								</tr>
							{/each}
						</tbody>
					{/each}
					{#each extras as x (x.title)}
						<tbody>
							<tr class="group"><td colspan="5">{x.title} · {x.rows.length}</td></tr>
							{#each x.rows as [name, what], i (i)}
								<tr><td class="mono">{name}</td><td class="faint" colspan="4">{what}</td></tr>
							{/each}
						</tbody>
					{/each}
				</table>
			{:else if shown?.verb && shown.verb !== 'build'}
				<table class="nodes">
					<tbody>
						{#each picked as key (key)}
							{@const node = byKey.get(key)}
							<tr>
								<td><a href={to(`/catalog/${key}`)} class="mono">{key.slice(key.indexOf(':') + 1)}</a></td>
								<td class="faint">{node ? kindLine(node) : ''}</td>
								<td class="faint right">{node?.lastBuild ? `built ${formatAgo(node.lastBuild.builtAt, app.now)}` : ''}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{/if}
			{#if shown?.verb && picked.length && catalog.data}
				<div class="graph" class:below={!!plan || shown?.verb !== 'build'} style:height="{graphHeight}px"><ScopeGraph keys={picked} catalog={catalog.data} onselect={() => {}} /></div>
			{/if}
		</div>

		{#if shown?.verb}
		<aside class="rail sb-scroll" class:stale={!fresh || !!problem}>
				<h3>Permissions</h3>
				{#each job ? jobPermissions : shown.permissions as p (p.action)}
					<p class="decision" data-testid="adhoc-permission">
						<span class={p.allowed ? 'ok-text' : 'bad-text'}>{p.allowed ? '✓' : '✕'}</span>
						<span title={p.action}>{PERMISSIONS[p.action].label}</span>
					</p>
					{#if p.reason}<p class="bad-text reason">{p.reason}</p>{/if}
				{/each}
				{#if Object.keys(shown.settings).length}
					<h3>Options</h3>
					<div class="chips">
						{#each Object.entries(shown.settings) as [name, value] (name)}<span class="sb-tag code">{value === true ? name : `${name}=${value}`}</span>{/each}
					</div>
				{/if}
		</aside>
		{/if}
	</div>

	<div class="foot">
		<div class="why">
			{#if launchError}<span class="problem">{launchError}</span>{:else if !canLaunch && blockedReason && blockedReason !== problem}<span class="faint">{blockedReason}</span>{/if}
		</div>
		<GatedButton requires={job ? 'job.run' : 'run.adhoc'} variant={canLaunch ? 'default' : 'outline'} disabled={!canLaunch} reason={blockedReason} busy={launching} onclick={launch}>▷ Run</GatedButton>
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
	.jobchip button {
		padding: 0 5px;
		color: var(--muted-foreground);
	}
	.jobchip button:hover {
		color: var(--foreground);
	}
	.verbs {
		display: flex;
		flex-shrink: 0;
		overflow: hidden;
		border: 1px solid var(--border);
		border-radius: 2px;
	}
	.verbs button {
		padding: 4px 11px;
		border-right: 1px solid var(--border);
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.verbs button:last-child {
		border-right: 0;
	}
	.verbs button.on {
		background: var(--sidebar-accent);
		color: var(--foreground);
	}
	.line {
		position: relative;
		display: flex;
		min-width: 0;
		flex: 1;
		align-items: center;
		gap: 10px;
		padding: 5px 10px;
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
	.copy {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.copy:hover {
		color: var(--foreground);
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
	.main {
		min-width: 0;
		flex: 1;
		overflow: auto;
		padding: 14px 24px;
		transition: opacity 0.15s;
	}
	.main.stale,
	.rail.stale {
		opacity: 0.55;
	}
	.msg {
		font-size: 13px;
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
	.planning {
		display: flex;
		align-items: center;
		gap: 10px;
		margin-bottom: 12px;
		padding: 12px 14px;
		border: 1px solid var(--border);
		border-radius: 2px;
		background: var(--sb-inset);
		font-size: 12.5px;
	}
	.replan {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		margin-left: auto;
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.replan:hover:not(:disabled) {
		color: var(--foreground);
	}
	.warnings {
		margin: 0 0 12px;
		padding: 0;
		list-style: none;
		font-size: 12.5px;
	}
	.warnings li {
		padding: 4px 0 4px 10px;
		border-left: 2px solid var(--sb-warning);
	}
	.warnings li.err {
		border-left-color: var(--sb-error);
	}
	.warnings code {
		margin-right: 8px;
		color: var(--sb-text-faint);
	}
	.wrap {
		white-space: normal;
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
	.summary b {
		color: var(--foreground);
		font-weight: 600;
	}
	.summary span {
		color: var(--sb-text-faint);
	}
	i {
		display: inline-block;
		width: 7px;
		height: 7px;
		margin-right: 6px;
		border-radius: 50%;
	}
	.summary i {
		margin-right: 0;
	}
	.graph {
		margin-bottom: 14px;
	}
	.graph.below {
		margin-top: 16px;
		border: 1px solid var(--border);
		border-radius: 2px;
	}
	.nodes {
		width: 100%;
		border-collapse: collapse;
		font-size: 12.5px;
	}
	.nodes td {
		padding: 5px 12px 5px 0;
		border-bottom: 1px solid var(--border-subtle);
		white-space: nowrap;
	}
	.nodes tr.group td {
		padding-top: 14px;
		color: var(--sb-text-faint);
		font-size: 12px;
		font-weight: 500;
	}
	.nodes tbody:first-child tr.group td {
		padding-top: 4px;
	}
	.right {
		text-align: right;
	}
	.rail {
		width: 280px;
		flex-shrink: 0;
		overflow: auto;
		padding: 14px 18px;
		border-left: 1px solid var(--border);
	}
	.rail h3 {
		margin: 16px 0 6px;
		color: var(--sb-text-faint);
		font-size: 12px;
		font-weight: 500;
	}
	.rail h3:first-child {
		margin-top: 0;
	}
	.decision {
		display: flex;
		align-items: baseline;
		gap: 8px;
		margin: 3px 0;
		font-size: 12.5px;
	}
	.reason {
		margin: 0 0 6px 18px;
		font-size: 11.5px;
	}
	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: 5px;
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
	.ok-text {
		color: var(--sb-success);
	}
	.bad-text {
		color: var(--sb-error);
	}
	.problem {
		color: var(--sb-error);
	}
	a:hover {
		color: var(--primary);
	}
	.sb-scroll {
		scrollbar-width: thin;
		scrollbar-color: var(--border-strong) transparent;
	}
</style>
