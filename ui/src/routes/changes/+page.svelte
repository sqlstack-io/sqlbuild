<script lang="ts">
	import { countBy } from '$lib/list/count-by';
	import { plural } from '$lib/format/plural';
	import PageBody from '$lib/components/page-body.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import Loading from '$lib/components/loading.svelte';
	import { to } from '$lib/paths';
	import DisclosureButton from '$lib/components/disclosure-button.svelte';
	import GitCommitIcon from '@lucide/svelte/icons/git-commit-horizontal';
	import PackageIcon from '@lucide/svelte/icons/package';
	import type { SnapshotChange } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import RefreshButton from '$lib/components/refresh-button.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { runStatusLabel, runStatusTone } from '$lib/status';
	import { changeOpLabel, changeOpTone, formatCredits } from '$lib/build-area/labels';
	import { NODE_LIST_LIMIT, nodeChangeGroups, summarizeChange } from '$lib/build-area/changes';
	import { quietDeployLine, recentDeployRows } from '$lib/build-area/overview-panels';
	import { shortRunId } from '$lib/operate/labels';
	import { nodeHref, orchestrationHref, percentChange, shortRevision } from './utils';

	type Area = 'all' | 'nodes' | 'orchestration' | 'roles' | 'environment';

	const changes = createResource((client, scope) => client.listSnapshotChanges(scope), ['snapshot', 'runs']);
	const all = $derived(changes.data ?? []);

	let query = $state('');
	let area = $state<Area>('all');
	const hasArea = (c: SnapshotChange, a: Area) =>
		a === 'all' ||
		(a === 'nodes' && c.nodes.length > 0) ||
		(a === 'orchestration' && c.orchestration.length > 0) ||
		(a === 'roles' && c.roles.length > 0) ||
		(a === 'environment' && !!c.environment);
	const filtered = $derived(
		all.filter((c) => {
			if (!hasArea(c, area)) return false;
			const q = query.trim().toLowerCase();
			if (!q) return true;
			return [c.gitRevision ?? '', c.gitAuthor ?? '', String(c.generation), ...c.nodes.map((n) => n.key), ...c.nodes.map((n) => n.formerKey ?? ''), ...c.orchestration.map((o) => o.key), ...c.roles.map((r) => r.role)].some((s) => s.toLowerCase().includes(q));
		})
	);
	/** Deploys, by snapshot, whose long node list is shown in full. */
	let listed = $state<Record<string, boolean>>({});
	/** Deploys that changed nothing are merged into one line per run of them, as on the overview. */
	const rows = $derived(recentDeployRows(filtered));
	/** Merged lines, by key, that are expanded; a search shows every match, so it expands them all. */
	let openQuiet = $state<Record<string, boolean>>({});
	const newest = $derived(all[0]?.snapshotId);
	const hasBody = (c: SnapshotChange) =>
		c.nodes.length > 0 || c.orchestration.length > 0 || c.roles.length > 0 || !!c.environment || !!c.costChanges?.length || c.followingRuns.length > 0;
	const areaOptions: { value: Area; label: string }[] = [
		{ value: 'all', label: 'All' },
		{ value: 'nodes', label: 'Nodes' },
		{ value: 'orchestration', label: 'Jobs and triggers' },
		{ value: 'roles', label: 'Roles' },
		{ value: 'environment', label: 'Environment' }
	];
	const byArea = $derived(countBy(all, (c) => areaOptions.filter((o) => hasArea(c, o.value)).map((o) => o.value)));
</script>

<AppTopbar title="Changes">
	<RefreshButton onclick={() => changes.refresh()} />
</AppTopbar>

<ListToolbar loading={changes.data === undefined} bind:query placeholder="Search nodes, commits, authors" total="{filtered.length} of {all.length} deploys">
	{#snippet filters()}
		<SegmentedFilter loading={changes.data === undefined} bind:value={area} options={areaOptions.map((o) => ({ ...o, count: byArea.get(o.value) ?? 0 }))} />
	{/snippet}
</ListToolbar>

{#snippet deploy(c: SnapshotChange)}
	<article class="deploy" id="gen-{c.generation}">
		<header>
			<span class="icon"><GitCommitIcon size={15} /></span>
			<div class="title">
				<strong title="Snapshot {c.snapshotId}, generation {c.generation}">Version {c.generation}</strong>
				{#if c.gitRevision}<code title="Commit {c.gitRevision}">{shortRevision(c.gitRevision)}</code>{/if}
				{#if c.gitDirty}<span class="faint">with uncommitted changes</span>{/if}
				<span class="faint">{summarizeChange(c)}</span>
				{#if !c.followingRuns.length}<span class="no-runs" title="No runs of this version yet">No runs</span>{/if}
			</div>
			<div class="who">
				{#if c.gitAuthor}<span>{c.gitAuthor}</span>{/if}
				<span class="faint">{c.publishedBy ? `deployed by ${c.publishedBy.displayName} · ` : 'published '}<span title={formatStamp(c.publishedAt)}>{formatAgo(c.publishedAt, app.now)}</span></span>
			</div>
		</header>

		{#if hasBody(c)}
			<div class="body">
				{#if c.nodes.length}
					{@const grouped = c.nodes.length > NODE_LIST_LIMIT && !listed[c.snapshotId] && !query.trim()}
					<section>
						<h3>Nodes <span>{c.nodes.length.toLocaleString('en')}</span></h3>
						{#if grouped}
							{#each nodeChangeGroups(c.nodes) as g (g.op + g.prefix)}
								<div class="row">
									<span class="op" style:--tone={changeOpTone[g.op]}>{changeOpLabel[g.op]}</span>
									<span>{g.count.toLocaleString('en')} {g.noun}</span>
								</div>
							{/each}
							<Button variant="link" size="inline" class="mt-1" onclick={() => (listed[c.snapshotId] = true)}>Show all {c.nodes.length.toLocaleString('en')} nodes</Button>
						{/if}
						{#each grouped ? [] : c.nodes as n (n.key + n.op)}
							{@const href = nodeHref(n, app.client.supports('getCatalogNode'))}
							<div class="row">
								<span class="op" style:--tone={changeOpTone[n.op]}>{changeOpLabel[n.op]}</span>
								{#if href}<a class="key" {href}>{n.key}</a>{:else}<code class:gone={n.op === 'removed'}>{n.key}</code>{/if}
								<span class="detail">{n.detail}{#if n.compiledOnly}<span class="sb-tag ml-2">compiled only</span>{/if}</span>
							</div>
							{#each n.columns as col (col.name)}
								<div class="row sub">
									<span class="op small" style:--tone={changeOpTone[col.op]}>{changeOpLabel[col.op]}</span>
									<code>{n.key}.{col.name}</code>
									<span class="detail">{col.detail}</span>
								</div>
							{/each}
						{/each}
					</section>
				{/if}

				{#if c.orchestration.length}
					<section>
						<h3>Jobs, triggers and sensors <span>{c.orchestration.length}</span></h3>
						{#each c.orchestration as o (o.kind + o.key)}
							{@const href = orchestrationHref(o)}
							<div class="row">
								<span class="op" style:--tone={changeOpTone[o.op]}>{changeOpLabel[o.op]}</span>
								<span class="faint kind">{o.kind}</span>
								{#if href}<a class="key" {href}>{o.key}</a>{:else}<code>{o.key}</code>{/if}
								<span class="detail">{o.detail}</span>
							</div>
						{/each}
					</section>
				{/if}

				{#if c.roles.length}
					<section>
						<h3>Roles <span>{c.roles.length}</span></h3>
						{#each c.roles as r, i (`${i}:${r.role}`)}
							<div class="row">
								<span class="op" style:--tone={changeOpTone[r.op]}>{changeOpLabel[r.op]}</span>
								{#if !r.role}<span class="faint" title="Only people who can give roles see which role changed">a role</span>
								{:else if app.client.supports('getRole')}<a class="key" href={to(`/admin/roles/${encodeURIComponent(r.role)}`)}>{r.role}</a>
								{:else}<code>{r.role}</code>{/if}
								<span class="detail">
									{#if r.coverage}<b>now covers {plural(r.coverage.after, 'node')}</b>{`${r.coverage.before !== r.coverage.after ? ` (was ${r.coverage.before})` : ''} · `}{/if}{r.detail}
								</span>
							</div>
						{/each}
					</section>
				{/if}

				{#if c.environment}
					<section>
						<h3>Environment</h3>
						<div class="row">
							<span class="icon-inline"><PackageIcon size={13} /></span>
							<span class="detail">{c.environment.detail}</span>
							<code class="faint">{c.environment.before} → {c.environment.after}</code>
						</div>
					</section>
				{/if}

				{#if c.costChanges?.length}
					<section>
						<h3>Cost since this deploy</h3>
						{#each c.costChanges as cc (cc.jobKey)}
							{@const pct = percentChange(cc.before, cc.after)}
							<div class="row">
								<a class="key" href={to(`/jobs/${cc.jobKey}`)}>{cc.jobKey}</a>
								<span class="detail">{formatCredits(cc.before)} → {formatCredits(cc.after)} per run <b class:up={pct > 0} class:down={pct < 0}>{pct > 0 ? '+' : ''}{pct}%</b> <span class="faint">over {plural(cc.runsAfter, 'run')} since {c.gitRevision ? `revision ${shortRevision(c.gitRevision)}` : 'this deploy'}</span></span>
							</div>
						{/each}
					</section>
				{/if}

				{#if c.followingRuns.length}
					<section>
						<h3>Runs that followed <span>{c.followingRuns.length}{c.snapshotId !== newest ? ' before the next deploy' : ''}</span></h3>
						<div class="runs">
							{#each c.followingRuns as r (r.id)}
								<a class="run" href={to(`/runs/${r.id}`)} title="{r.id} · {runStatusLabel[r.status]} · {formatStamp(r.startedAt ?? r.queuedAt)}">
									<i style:background={runStatusTone[r.status]}></i>
									<code>{shortRunId(r.id)}</code>
									<span class="faint">{r.jobKey ?? 'Manual'} · {formatAgo(r.startedAt ?? r.queuedAt, app.now)}</span>
								</a>
							{/each}
						</div>
					</section>
				{/if}
			</div>
		{/if}
	</article>
{/snippet}

<PageBody padding="18px 24px 48px">
	{#if changes.error}<div class="msg err">Couldn't load the change feed: {changes.error}</div>{/if}
	{#each rows as row (row.key)}
		{#if row.kind === 'deploy'}
			{@render deploy(row.change)}
		{:else if query.trim()}
			{#each row.changes as c (c.snapshotId)}{@render deploy(c)}{/each}
		{:else}
			{@const open = !!openQuiet[row.key]}
			<div class="quiet"><DisclosureButton {open} onclick={() => (openQuiet[row.key] = !open)} class="w-full px-4 py-2 text-[12px]">{quietDeployLine(row, app.now)}</DisclosureButton></div>
			{#if open}
				{#each row.changes as c (c.snapshotId)}{@render deploy(c)}{/each}
			{/if}
		{/if}
	{:else}
		{#if changes.data === undefined && !changes.error}<div class="msg"><Loading lines={4} /></div>{:else}<div class="msg">{all.length ? 'No deploys match the filter.' : 'Nothing has been deployed to this location yet.'}</div>{/if}
	{/each}
</PageBody>

<style>
	.deploy {
		margin-bottom: 12px;
		border: 1px solid var(--panel-border);
		border-radius: 2px;
		background: var(--card);
		box-shadow: var(--panel-shadow);
		scroll-margin-top: 16px;
	}
	.deploy:target {
		border-color: var(--primary);
	}
	header {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 11px 16px;
	}
	header:not(:last-child) {
		border-bottom: 1px solid var(--border);
	}
	.icon {
		display: grid;
		width: 28px;
		height: 28px;
		place-items: center;
		border-radius: 2px;
		background: color-mix(in srgb, var(--primary) 11%, transparent);
		color: var(--primary);
	}
	.title {
		display: flex;
		min-width: 0;
		flex: 1 1 auto;
		overflow-wrap: anywhere;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 4px 10px;
		font-size: 12.5px;
	}
	.title strong {
		font-size: 13px;
		font-weight: 600;
	}
	.no-runs {
		color: var(--sb-text-faint);
		font-size: 12px;
	}
	/* A run of deploys that changed nothing, as one line; it expands to list them. */
	.quiet {
		margin-bottom: 12px;
		border: 1px dashed var(--panel-border);
		border-radius: 2px;
	}
	.quiet:hover {
		background: var(--sb-hover);
	}
	.title code {
		color: var(--primary);
		font: 12px var(--font-mono);
	}
	/* Author and time wrap within the header rather than push it past the window's edge. */
	.who {
		display: flex;
		min-width: 0;
		max-width: 40%;
		flex: 0 1 auto;
		flex-direction: column;
		align-items: flex-end;
		margin-left: auto;
		overflow-wrap: anywhere;
		text-align: right;
		font-size: 12px;
		line-height: 1.45;
	}
	.body {
		padding: 4px 16px 12px;
	}
	section {
		margin-top: 12px;
	}
	h3 {
		margin: 0 0 4px;
		color: var(--sb-text-faint);
		font-size: 12px;
		font-weight: 500;
	}
	h3 span {
		margin-left: 4px;
		font-weight: 400;
		font-variant-numeric: tabular-nums;
	}
	.row {
		display: flex;
		align-items: baseline;
		gap: 10px;
		padding: 3px 0;
		font-size: 12.5px;
	}
	.row.sub {
		padding-left: 22px;
	}
	.op {
		flex: 0 0 64px;
		color: var(--tone);
		font-size: 11px;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}
	.op.small {
		font-weight: 500;
		opacity: 0.85;
	}
	.kind {
		flex: 0 0 52px;
		font-size: 12px;
	}
	.key {
		color: var(--primary);
		font: 12px var(--font-mono);
	}
	.key:hover {
		text-decoration: underline;
	}
	.gone {
		color: var(--muted-foreground);
		text-decoration: line-through;
	}
	code {
		font: 12px var(--font-mono);
	}
	.detail {
		color: var(--muted-foreground);
	}
	.detail b {
		color: var(--foreground);
		font-weight: 500;
	}
	.detail b.up {
		color: var(--sb-warning);
	}
	.detail b.down {
		color: var(--sb-success);
	}
	.icon-inline {
		color: var(--muted-foreground);
	}
	.runs {
		display: flex;
		flex-wrap: wrap;
		gap: 6px 14px;
	}
	.run {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		font-size: 12px;
	}
	.run i {
		width: 7px;
		height: 7px;
		border-radius: 50%;
	}
	.run code {
		color: var(--primary);
	}
	.faint {
		color: var(--muted-foreground);
	}
	.msg {
		padding: 14px 0;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
	.err {
		color: var(--sb-error);
	}
</style>
