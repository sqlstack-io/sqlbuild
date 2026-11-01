<script lang="ts">
	import GitCommitIcon from '@lucide/svelte/icons/git-commit-horizontal';
	import PackageIcon from '@lucide/svelte/icons/package';
	import type { SnapshotChange } from '$lib/api';
	import AppTopbar from '$lib/components/app-topbar.svelte';
	import Button from '$lib/components/ui/button/button.svelte';
	import ListToolbar from '$lib/components/list/list-toolbar.svelte';
	import SegmentedFilter from '$lib/components/list/segmented-filter.svelte';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { runStatusLabel, runStatusTone } from '$lib/status';
	import { changeOpLabel, changeOpTone, formatCredits } from '$lib/build-area/labels';
	import { summarizeChange } from '$lib/build-area/changes';
	import { orchestrationHref, percentChange } from './utils';

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
			return [c.gitRevision, c.gitAuthor, String(c.generation), ...c.nodes.map((n) => n.key), ...c.nodes.map((n) => n.formerKey ?? ''), ...c.orchestration.map((o) => o.key), ...c.roles.map((r) => r.role)].some((s) => s.toLowerCase().includes(q));
		})
	);
	const areaOptions: { value: Area; label: string }[] = [
		{ value: 'all', label: 'All' },
		{ value: 'nodes', label: 'Nodes' },
		{ value: 'orchestration', label: 'Jobs and triggers' },
		{ value: 'roles', label: 'Roles' },
		{ value: 'environment', label: 'Environment' }
	];
</script>

<AppTopbar title="Changes">
	<Button variant="outline" size="sm" onclick={() => changes.refresh()}>↻ Refresh</Button>
</AppTopbar>

<ListToolbar bind:query placeholder="Search nodes, commits, authors" total="{filtered.length} of {all.length} deploys">
	{#snippet filters()}
		<SegmentedFilter bind:value={area} options={areaOptions.map((o) => ({ ...o, count: all.filter((c) => hasArea(c, o.value)).length }))} />
	{/snippet}
</ListToolbar>

<div class="feed sb-scroll">
	{#if changes.error}<div class="msg err">Couldn't load the change feed: {changes.error}</div>{/if}
	{#each filtered as c, i (c.snapshotId)}
		<article class="deploy" id="gen-{c.generation}">
			<header>
				<span class="icon"><GitCommitIcon size={15} /></span>
				<div class="title">
					<strong title="Snapshot {c.snapshotId}, generation {c.generation}">Version {c.generation}</strong>
					<code title="Commit">{c.gitRevision}{c.gitDirty ? ' · with uncommitted changes' : ''}</code>
					<span class="faint">{summarizeChange(c)}</span>
				</div>
				<div class="who">
					<span>{c.gitAuthor}</span>
					<span class="faint">deployed by {c.publishedBy.displayName} · <span title={formatStamp(c.publishedAt)}>{formatAgo(c.publishedAt, app.now)}</span></span>
				</div>
			</header>

			<div class="body">
				{#if c.nodes.length}
					<section>
						<h3>Nodes <span>{c.nodes.length}</span></h3>
						{#each c.nodes as n (n.key + n.op)}
							<div class="row">
								<span class="op" style:--tone={changeOpTone[n.op]}>{changeOpLabel[n.op]}</span>
								{#if n.op === 'removed'}<code class="gone">{n.key}</code>{:else}<a class="key" href="/catalog/{n.key}{n.op === 'changed' ? '?tab=drift' : ''}">{n.key}</a>{/if}
								<span class="detail">{n.detail}{#if n.compiledOnly}<span class="tag">compiled only</span>{/if}</span>
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
						{#each c.roles as r (r.role)}
							<div class="row">
								<span class="op" style:--tone={changeOpTone[r.op]}>{changeOpLabel[r.op]}</span>
								<a class="key" href="/admin/roles">{r.role}</a>
								<span class="detail">
									{#if r.coverage}<b>now covers {r.coverage.after} {r.coverage.after === 1 ? 'node' : 'nodes'}</b>{`${r.coverage.before !== r.coverage.after ? ` (was ${r.coverage.before})` : ''} · `}{/if}{r.detail}
									{#if r.approvalId}<a class="tag link" href="/admin/roles">needs approval</a>{/if}
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
								<a class="key" href="/jobs/{cc.jobKey}">{cc.jobKey}</a>
								<span class="detail">{formatCredits(cc.before)} → {formatCredits(cc.after)} per run <b class:up={pct > 0} class:down={pct < 0}>{pct > 0 ? '+' : ''}{pct}%</b> <span class="faint">over {cc.runsAfter} {cc.runsAfter === 1 ? 'run' : 'runs'} since revision {c.gitRevision}</span></span>
							</div>
						{/each}
					</section>
				{/if}

				<section>
					<h3>Runs that followed <span>{c.followingRuns.length}{i > 0 ? ' before the next deploy' : ''}</span></h3>
					{#if c.followingRuns.length}
						<div class="runs">
							{#each c.followingRuns as r (r.id)}
								<a class="run" href="/runs/{r.id}" title="{runStatusLabel[r.status]} · {formatStamp(r.startedAt ?? r.queuedAt)}">
									<i style:background={runStatusTone[r.status]}></i>
									<code>{r.id}</code>
									<span class="faint">{r.jobKey ?? 'started by hand'} · {formatAgo(r.startedAt ?? r.queuedAt, app.now)}</span>
								</a>
							{/each}
						</div>
					{:else}
						<p class="faint">No runs of this version yet.</p>
					{/if}
				</section>
			</div>
		</article>
	{:else}
		<div class="msg">{changes.loading ? 'Loading…' : all.length ? 'No deploys match the filter.' : 'Nothing has been deployed to this location yet.'}</div>
	{/each}
</div>

<style>
	.feed {
		min-height: 0;
		flex: 1;
		overflow-y: auto;
		padding: 18px 24px 48px;
	}
	.deploy {
		margin-bottom: 16px;
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
		padding: 12px 16px;
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
		align-items: baseline;
		gap: 10px;
		font-size: 13px;
	}
	.title code {
		color: var(--primary);
		font: 12px var(--font-mono);
	}
	.who {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		margin-left: auto;
		font-size: 12.5px;
	}
	.body {
		padding: 4px 16px 12px;
	}
	section {
		margin-top: 12px;
	}
	h3 {
		margin: 0 0 6px;
		color: var(--sb-text-faint);
		font-size: 12px;
		font-weight: 500;
	}
	h3 span {
		font-weight: 400;
	}
	.row {
		display: flex;
		align-items: baseline;
		gap: 10px;
		padding: 4px 0;
		font-size: 12.5px;
	}
	.row.sub {
		padding-left: 22px;
	}
	.op {
		min-width: 64px;
		color: var(--tone);
		font-size: 11.5px;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.03em;
	}
	.op.small {
		min-width: 64px;
		font-weight: 500;
		opacity: 0.85;
	}
	.kind {
		min-width: 52px;
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
	.tag {
		margin-left: 8px;
		padding: 0 6px;
		border: 1px solid var(--border);
		border-radius: 2px;
		color: var(--muted-foreground);
		font-size: 11px;
	}
	.tag.link {
		border-color: color-mix(in srgb, var(--sb-warning) 50%, var(--border));
		color: var(--sb-warning);
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
		font-size: 13px;
	}
	.err {
		color: var(--sb-error);
	}
</style>
