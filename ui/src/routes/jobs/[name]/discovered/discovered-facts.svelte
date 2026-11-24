<script lang="ts">
	import type { DiscoveredSet } from '$lib/api';
	import BreakableName from '$lib/components/names/breakable-name.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { discoveredStatusLabel, discoveredStatusTone } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import { shortHash } from './utils';

	// A discovered set's facts, for the side panel.
	let { set }: { set: DiscoveredSet } = $props();

	/** Tags shown as their own facts; the rest are listed as tags. */
	const NAMED = new Set(['release', 'plan_digest', 'max_parallel']);
	const otherTags = $derived(Object.entries(set.tags).filter(([k]) => !NAMED.has(k)));
	const release = $derived(shortHash(set.tags.release));
	const digest = $derived(shortHash(set.tags.plan_digest ?? set.digest));
	const failed = $derived(set.status === 'failed');
</script>

<dl class="dlist">
	<div><dt>Name</dt><dd class="mono"><BreakableName text={set.name} /></dd></div>
	{#if set.assets.length}<div><dt>Asset</dt><dd class="mono">{set.assets.join(', ')}</dd></div>{/if}
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

<style>
	.dlist i {
		display: inline-block;
		width: 7px;
		height: 7px;
		flex: 0 0 7px;
		margin-right: 6px;
		border-radius: 50%;
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
