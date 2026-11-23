<script lang="ts">
	import type { DiscoveredSet } from '$lib/api';
	import BreakableName from '$lib/components/names/breakable-name.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { plural } from '$lib/format/plural';
	import { discoveredStatusLabel, discoveredStatusTone } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import { shortHash } from './utils';

	// Mockup: a discovered set's facts, as one row under the job's title or as a side-panel list.
	let { set, layout }: { set: DiscoveredSet; layout: 'row' | 'list' } = $props();

	/** Tags shown as their own facts; the rest are listed as tags. */
	const NAMED = new Set(['release', 'plan_digest', 'max_parallel']);
	const otherTags = $derived(Object.entries(set.tags).filter(([k]) => !NAMED.has(k)));
	const release = $derived(shortHash(set.tags.release));
	const digest = $derived(shortHash(set.tags.plan_digest ?? set.digest));
	const failed = $derived(set.status === 'failed');
	const tagTitle = $derived(Object.entries(set.tags).map(([k, v]) => `${k} = ${v}`).join('\n'));
</script>

{#if layout === 'row'}
	<div class="drow" class:failed>
		<span class="k">Discovered</span>
		<code title="Discovered set{set.checkEverySeconds ? `, checked every ${set.checkEverySeconds / 60} min` : ''}">{set.name}</code>
		<span class="fact" title={set.error ?? (set.fetchedAt ? formatStamp(set.fetchedAt) : undefined)}>
			<i style:background={discoveredStatusTone[set.status]}></i>{discoveredStatusLabel[set.status]}{#if set.fetchedAt}&nbsp;<span class="faint">{formatAgo(set.fetchedAt, app.now)}</span>{/if}
		</span>
		{#if failed && set.lastGoodAt}<span class="fact warn" title="Steps from the last good fetch, {formatStamp(set.lastGoodAt)}">Last good · {formatAgo(set.lastGoodAt, app.now)}</span>{/if}
		{#if release}<span class="fact"><span class="faint">Release</span> <code title={set.tags.release}>{release}</code></span>{/if}
		{#if digest}<span class="fact"><span class="faint">Plan</span> <code title={set.tags.plan_digest ?? set.digest ?? undefined}>{digest}</code></span>{/if}
		<span class="fact">{plural(set.items, 'step')}</span>
		{#if set.tags.max_parallel}<span class="fact" title="max_parallel = {set.tags.max_parallel}"><span class="faint">Max parallel</span> {set.tags.max_parallel}</span>{/if}
		{#if otherTags.length}<span class="fact faint" title={tagTitle}>{plural(otherTags.length, 'more tag')}</span>{/if}
	</div>
{:else}
	<dl class="dlist">
		<div><dt>Set</dt><dd class="mono"><BreakableName text={set.name} /></dd></div>
		<div>
			<dt>Status</dt>
			<dd title={set.fetchedAt ? formatStamp(set.fetchedAt) : undefined}><i style:background={discoveredStatusTone[set.status]}></i>{discoveredStatusLabel[set.status]}{set.fetchedAt ? ` · ${formatAgo(set.fetchedAt, app.now)}` : ''}</dd>
		</div>
		{#if failed}<div><dt>Serving</dt><dd title={set.lastGoodAt ? formatStamp(set.lastGoodAt) : undefined}>{set.lastGoodAt ? `Last good · ${formatAgo(set.lastGoodAt, app.now)}` : 'No steps'}</dd></div>{/if}
		{#if release}<div><dt>Release</dt><dd class="mono" title={set.tags.release}>{release}</dd></div>{/if}
		{#if digest}<div><dt>Plan digest</dt><dd class="mono" title={set.tags.plan_digest ?? set.digest ?? undefined}>{digest}</dd></div>{/if}
		<div><dt>Steps</dt><dd>{set.items}</dd></div>
		{#if set.tags.max_parallel}<div><dt>Max parallel</dt><dd title="max_parallel">{set.tags.max_parallel}</dd></div>{/if}
		{#if set.checkEverySeconds}<div><dt>Checked</dt><dd title="check_every = {set.checkEverySeconds}s">Every {set.checkEverySeconds / 60} min</dd></div>{/if}
	</dl>
	{#if otherTags.length}
		<div class="tags">{#each otherTags as [k, v] (k)}<code title="{k} = {v}">{k}={v}</code>{/each}</div>
	{/if}
	{#if failed && set.error}<p class="error" title={set.error}><code>{set.error}</code></p>{/if}
{/if}

<style>
	.drow {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 4px 14px;
		padding: 0 24px 10px;
		color: var(--foreground);
		font-size: 12px;
	}
	.k {
		color: var(--sb-text-faint);
		font-size: 11.5px;
	}
	.fact {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		white-space: nowrap;
	}
	.fact i,
	.dlist i {
		display: inline-block;
		width: 7px;
		height: 7px;
		flex: 0 0 7px;
		border-radius: 50%;
	}
	.dlist i {
		margin-right: 6px;
	}
	.warn {
		color: color-mix(in srgb, var(--sb-warning) 75%, var(--foreground));
	}
	.faint {
		color: var(--muted-foreground);
	}
	code,
	.mono {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.dlist div {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		padding: 3px 0;
	}
	.dlist dt {
		flex-shrink: 0;
		color: var(--muted-foreground);
	}
	.dlist dd {
		min-width: 0;
		text-align: right;
	}
	.tags {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
		margin-top: 6px;
	}
	.tags code {
		padding: 1px 5px;
		border: 1px solid var(--border);
		border-radius: 2px;
		color: var(--muted-foreground);
	}
	.error {
		margin-top: 8px;
		padding: 4px 8px;
		border-left: 3px solid var(--sb-warning);
		background: color-mix(in srgb, var(--sb-warning) 10%, transparent);
		overflow-wrap: anywhere;
	}
</style>
