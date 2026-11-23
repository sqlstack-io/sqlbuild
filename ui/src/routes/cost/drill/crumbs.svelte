<script lang="ts">
	import { page } from '$app/state';
	import { scopeParams, type Crumb } from '../utils';

	// Where the explorer is, from All down to the open level; each step opens that level.
	let { crumbs }: { crumbs: Crumb[] } = $props();
	const href = (c: Crumb) => `?${scopeParams(page.url.searchParams, c.scope)}`;
</script>

<nav class="crumbs" aria-label="Breadcrumb">
	{#each crumbs as c, i (i)}
		{#if i}<span class="sep">/</span>{/if}
		{#if i < crumbs.length - 1}<a href={href(c)} data-sveltekit-noscroll>{c.label}</a>{:else}<span class="here" title={c.label}>{c.label}</span>{/if}
	{/each}
</nav>

<style>
	.crumbs { display: flex; min-width: 0; flex-wrap: wrap; align-items: baseline; gap: 4px 6px; color: var(--muted-foreground); font-size: 12.5px; }
	.crumbs a:hover { color: var(--foreground); text-decoration: underline; }
	.sep { color: var(--sb-text-faint); }
	.here { color: var(--foreground); font-weight: 600; }
</style>
