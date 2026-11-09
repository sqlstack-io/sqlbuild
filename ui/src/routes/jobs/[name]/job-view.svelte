<script lang="ts">
	import { goto } from '$app/navigation';
	import type { Trigger } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import CodePreview from '$lib/components/code/code-preview.svelte';
	import GatedButton from '$lib/components/operate/gated-button.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { formatAgo, formatDuration, formatStamp } from '$lib/format/time';
	import { formatNodeCounts, runOriginLabel, runStatusLabel, runStatusTone } from '$lib/status';
	import { formatCost, onFailureHelp, onFailurePlain, overlapHelp, overlapPlain, shortRunId, slotLabel } from '$lib/operate/labels';
	import JobGraph from './job-graph.svelte';
	import LaunchForm from './launch-form.svelte';

	let { name }: { name: string } = $props();

	const job = createResource((client, scope) => client.getJob(scope, name), ['jobs', 'runs', 'snapshot']);
	const jobRuns = createResource((client, scope) => client.listRuns(scope, { jobKey: name, limit: 12 }), ['runs']);
	const triggers = createResource((client, scope) => client.listTriggers(scope), ['triggers', 'ticks']);
	const catalog = createResource((client, scope) => client.listCatalogNodes(scope), ['snapshot', 'runs', 'warehouse']);
	// Each member's nodes and last run, so a group's graph can box every member job around what it builds.
	// Reading `job.data` here reloads it with the job; it stays null until the job has loaded.
	const members = createResource(async (client, scope) => {
		const detail = job.data;
		if (!detail) return null;
		const keys = detail.groupMembers?.map((m) => m.jobKey) ?? [];
		if (!keys.length) return { nodes: new Map<string, string[]>(), lastRuns: new Map() };
		const jobs = await client.listJobs(scope);
		const details = await Promise.all(keys.map((k) => client.getJob(scope, k)));
		return {
			nodes: new Map(keys.map((k, i) => [k, details[i]?.resolvedNodes ?? []])),
			lastRuns: new Map(jobs.map((j) => [j.key, j.lastRun]))
		};
	}, ['jobs', 'runs', 'snapshot']);

	const def = $derived(job.data ?? null);
	const jobTriggers = $derived((triggers.data ?? []).filter((t) => def?.triggerIds.includes(t.id)));
	const runs = $derived(jobRuns.data ?? []);
	const showCost = $derived(runs.some((r) => r.cost !== null));
	const canToggle = $derived(app.can('trigger.toggle'));
	let launchOpen = $state(false);
	let actionError = $state<string | null>(null);

	async function toggleTrigger(t: Trigger) {
		const scope = app.scope;
		if (!scope) return;
		actionError = null;
		try {
			await app.client.setTriggerStates(scope, [t.id], t.state === 'running' ? 'paused' : 'running');
		} catch (e) {
			actionError = e instanceof Error ? e.message : String(e);
		}
	}

	const targetBlocked = $derived(!!def?.targets && !def.targets.includes(app.target?.name ?? ''));
</script>

<AppTopbar title="Jobs">
	<div class="relative">
		<GatedButton
			requires="job.run"
			variant="default"
			disabled={!def || targetBlocked}
			reason={targetBlocked ? `This job only runs on ${def?.targets?.join(', ')}` : undefined}
			onclick={() => (launchOpen = !launchOpen)}
		>▷ Run now</GatedButton>
		{#if launchOpen && def}<LaunchForm job={def} onclose={() => (launchOpen = false)} />{/if}
	</div>
</AppTopbar>

<div class="flex min-h-0 flex-1 flex-col">
	<div class="head">
		<a href="/jobs" class="back">← Jobs</a>
		<span class="font-display text-[17px] font-semibold">{name}</span>
		{#if def}
			<span class="faint">{def.kind === 'group' ? (def.members ? `Group of ${def.members.length} jobs` : 'Group') : 'Job'} · {def.nodeCount} nodes</span>
			<span class="faint">{def.description}</span>
		{/if}
	</div>
	{#if job.error}
		<div class="msg err">Couldn't load this job: {job.error}</div>
	{:else if job.data === null}
		<div class="msg">There's no job or group named <code>{name}</code> in the version deployed to {app.location?.name}.</div>
	{/if}
	{#if actionError}<div class="msg err">{actionError}</div>{/if}

	{#if def}
		<div class="flex min-h-0 flex-1">
			<div class="main sb-scroll">
				<section>
					<h3>What it builds · {def.resolvedNodes.length} nodes <span>{def.select ? `--select ${def.select}` : def.kind === 'group' ? 'each box is one of its jobs; arrows are the data each node reads' : ''}{def.exclude.length ? ` --exclude ${def.exclude.join(' ')}` : ''}</span></h3>
					{#if catalog.data && (def.kind !== 'group' || members.data)}
						<JobGraph job={def} catalog={catalog.data} memberNodes={members.data?.nodes ?? new Map()} lastRuns={members.data?.lastRuns ?? new Map()} />
					{:else}
						<p class="faint">{catalog.error ?? members.error ?? 'Loading…'}</p>
					{/if}
					{#if def.criticalNodes.length}
						<p class="critical">
							Critical (if one fails, the whole group stops):
							{#each def.criticalNodes as n, i (n)}{i ? ', ' : ''}<a href="/catalog/{n}"><code>{n}</code></a>{/each}
						</p>
					{/if}
				</section>

				<section>
					<h3>Recent runs</h3>
					{#if runs.length}
						<table class="runs">
							<tbody>
								{#each runs as r (r.id)}
									<tr onclick={() => goto(`/runs/${r.id}`)}>
										<td><i style:background={runStatusTone[r.status]}></i>{runStatusLabel[r.status]}</td>
										<td><a href="/runs/{r.id}" class="mono" title={r.id}>{shortRunId(r.id)}</a></td>
										<td class="faint">{runOriginLabel[r.origin]}{r.parentRunId ? ` in group run ${shortRunId(r.parentRunId)}` : r.triggerName ? ` · ${r.triggerName}` : ''}</td>
										<td class="faint">{formatNodeCounts(r.nodeCounts)}</td>
										<td class="faint">{r.startedAt ? formatDuration(r.startedAt, r.endedAt, app.now) : '—'}</td>
										<td class="faint" title={formatStamp(r.queuedAt)}>{formatAgo(r.startedAt ?? r.queuedAt, app.now)}</td>
										<td class="faint" title={r.requestedBy.name}>{r.requestedBy.displayName}</td>
										{#if showCost}<td class="faint num" title={r.cost ? `Estimated warehouse cost: ${r.cost.source}` : 'No cost reported'}>{formatCost(r.cost)}</td>{/if}
									</tr>
								{/each}
							</tbody>
						</table>
					{:else}
						<p class="faint">{jobRuns.loading ? 'Loading…' : 'No runs yet. Use Run now, or turn on a schedule.'}</p>
					{/if}
				</section>

				{#if def.declaration}
					<section>
						<h3>Code <span>read-only; change it in the project and deploy</span></h3>
						<CodePreview file="{def.declaration.sourceFile}:{def.declaration.line}" code={def.declaration.code} language="text" maxLines={24} />
					</section>
				{/if}
			</div>

			<aside class="rail sb-scroll">
				<h3>How it runs</h3>
				<dl>
					<div><dt>Kind</dt><dd>{def.kind === 'group' ? 'Group of jobs' : 'Job'}</dd></div>
					{#if def.select}<div><dt>Selection</dt><dd><code>{def.select}</code></dd></div>{/if}
					{#if def.overlap}<div><dt>If already running</dt><dd title="overlap = {def.overlap}">{overlapPlain[def.overlap]}</dd></div>{/if}
					{#if def.onFailure}<div><dt>If something fails</dt><dd title="on_failure = {def.onFailure}">{onFailurePlain[def.onFailure]}</dd></div>{/if}
					{#if def.priority !== null}<div><dt>Priority</dt><dd title="Higher priority runs go first when they wait for the same slot">{def.priority}</dd></div>{/if}
					{#if def.declaration}<div><dt>Targets</dt><dd>{def.targets?.join(', ') ?? 'all targets'}</dd></div>{/if}
					{#if def.owner}<div><dt>Owner</dt><dd>{def.owner}</dd></div>{/if}
					{#if def.sourceFile}<div><dt>Defined in</dt><dd><code>{def.sourceFile}</code></dd></div>{/if}
					{#if def.memberOf?.length}
						<div><dt>In groups</dt><dd>{#each def.memberOf as g, i (g)}{i ? ', ' : ''}<a href="/jobs/{g}">{g}</a>{/each}</dd></div>
					{/if}
				</dl>
				{#if def.overlap && def.onFailure}<p class="help">{overlapHelp[def.overlap]}. {onFailureHelp[def.onFailure]}.</p>{/if}

				{#if def.settings}
					<h3>Extra options</h3>
					{#if Object.keys(def.settings).length}
						{#each Object.entries(def.settings) as [flag, value] (flag)}
							<p class="setting"><code>{flag}{value === true ? '' : ` ${value}`}</code></p>
						{/each}
					{:else}
						<p class="faint">None; it runs with the defaults.</p>
					{/if}
				{/if}

				<h3>Shared slots</h3>
				{#if def.concurrency === null}
					{#if def.concurrencyGroups.length}
						{#each def.concurrencyGroups as g (g)}<p class="setting" title="Concurrency group {g}"><span>Uses {slotLabel(g)} slots</span></p>{/each}
					{:else}
						<p class="faint">None. It only waits for runs that update the same data.</p>
					{/if}
				{:else if def.concurrency.length}
					{#each def.concurrency as g (g.key)}
						<p class="setting" title="Concurrency group {g.key}: at most {g.limit} runs at once">
							<span>{g.holders.length} of {g.limit} {slotLabel(g.key)} slot{g.limit === 1 ? '' : 's'} in use</span>
							{#each g.holders as h (h)}<a class="mono" href="/runs/{h}" title={h}>{shortRunId(h)}</a>{/each}
						</p>
					{/each}
				{:else}
					<p class="faint">None. It only waits for runs that update the same data.</p>
				{/if}

				<h3>Schedules and sensors · {jobTriggers.length}</h3>
				{#if jobTriggers.length === 0}
					<p class="faint">Nothing starts it automatically. Run it by hand{def.memberOf?.length ? ', or through its groups' : ''}.</p>
				{:else}
					{#each jobTriggers as t (t.id)}
						<div class="trigger" class:off={t.state === 'paused'}>
							<div class="min-w-0">
								<a href="/triggers/{t.id}">{t.name}</a>
								<div class="faint">{t.cadence}{t.state !== t.defaultState ? ` · ${t.defaultState === 'running' ? 'on' : 'paused'} in the code` : ''}</div>
							</div>
							<span title={canToggle ? (t.state === 'running' ? 'Pause: stop starting runs' : 'Turn on') : "You don't have permission to pause or turn on schedules and sensors (trigger.toggle)"}>
								<button class="sb-toggle" class:on={t.state === 'running'} disabled={!canToggle} onclick={() => toggleTrigger(t)} aria-label="{t.state === 'running' ? 'Pause' : 'Turn on'} {t.name}"></button>
							</span>
						</div>
					{/each}
				{/if}
			</aside>
		</div>
	{/if}
</div>

<style>
	.head {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px 24px;
		border-bottom: 1px solid var(--border);
		font-size: 12.5px;
	}
	.back {
		color: var(--muted-foreground);
		font-size: 13px;
	}
	.faint {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.msg {
		padding: 14px 24px;
		color: var(--muted-foreground);
		font-size: 13px;
	}
	.err {
		color: var(--sb-error);
	}
	.main {
		min-width: 0;
		flex: 1;
		overflow: auto;
		padding: 18px 24px;
	}
	section {
		margin-bottom: 26px;
	}
	h3 {
		display: flex;
		align-items: baseline;
		gap: 8px;
		margin-bottom: 8px;
		color: var(--sb-text-faint);
		font-size: 12px;
		font-weight: 500;
	}
	h3 span {
		font-weight: 400;
	}
	code,
	.mono {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.runs {
		width: 100%;
		border-collapse: collapse;
		font-size: 12.5px;
	}
	.runs tr {
		cursor: pointer;
	}
	.runs tr:hover {
		background: var(--sb-hover);
	}
	.runs td {
		padding: 7px 12px 7px 0;
		border-bottom: 1px solid var(--border-subtle);
		white-space: nowrap;
	}
	.runs td i {
		display: inline-block;
		width: 7px;
		height: 7px;
		margin-right: 7px;
		border-radius: 50%;
	}
	.num {
		text-align: right;
	}
	.critical {
		margin-top: 8px;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.rail {
		width: 320px;
		flex-shrink: 0;
		overflow: auto;
		padding: 18px 20px;
		border-left: 1px solid var(--border);
	}
	.rail h3 {
		margin-top: 18px;
	}
	.rail h3:first-child {
		margin-top: 0;
	}
	dl div {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		padding: 3px 0;
		font-size: 12.5px;
	}
	dt {
		color: var(--muted-foreground);
	}
	dd {
		text-align: right;
	}
	.help {
		margin-top: 6px;
		color: var(--sb-text-faint);
		font-size: 11.5px;
		line-height: 1.5;
	}
	.setting {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 8px;
		padding: 2px 0;
		font-size: 12px;
	}
	.trigger {
		display: flex;
		align-items: center;
		gap: 10px;
		margin-bottom: 6px;
		padding: 8px 10px;
		border: 1px solid var(--border-subtle);
		border-radius: 2px;
		font-size: 12.5px;
	}
	.trigger.off {
		opacity: 0.7;
	}
	.trigger span:last-child {
		margin-left: auto;
	}
	a:hover {
		color: var(--primary);
	}
	.sb-scroll {
		scrollbar-width: thin;
		scrollbar-color: var(--border-strong) transparent;
	}
</style>
