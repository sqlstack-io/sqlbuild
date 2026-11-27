<script lang="ts">
	import type { ConnectionTest } from '$lib/api';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { testTone } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import { checkOrigin, checkTitle, checkWords } from './utils/connections';

	// The last connection check: "● connected 3h ago" over "on deploy · 820 ms", details on hover, in the words every page uses.
	let { test }: { test: ConnectionTest | null } = $props();
</script>

{#if test}
	<span class="check" title={checkTitle(test)}>
		<StatusLabel tone={testTone[test.outcome]} label={checkWords(test, app.now)} />
		<span class="origin">{checkOrigin(test)}</span>
	</span>
{:else}
	<StatusLabel tone="var(--sb-text-faint)" label="not tested yet" muted />
{/if}

<style>
	.check {
		display: inline-flex;
		min-width: 0;
		flex-direction: column;
		gap: 1px;
		white-space: nowrap;
	}
	.origin {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.check .origin {
		padding-left: 15px;
	}
</style>
