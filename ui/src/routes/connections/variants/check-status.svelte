<script lang="ts">
	import type { ConnectionTest } from '$lib/api';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { testTone } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import { checkOrigin, checkTitle, checkWords } from './utils';

	// The last connection check on one line: "● Passed 3h ago  after deploy · 820 ms", details on hover.
	// `stacked` puts the origin under the status, for a table cell; `brief` leaves it to the tooltip.
	let { test, stacked = false, brief = false }: { test: ConnectionTest | null; stacked?: boolean; brief?: boolean } = $props();
</script>

{#if test}
	<span class="check" class:stacked title={checkTitle(test)}>
		<StatusLabel tone={testTone[test.outcome]} label={checkWords(test, app.now)} />
		{#if !brief}<span class="origin">{checkOrigin(test)}</span>{/if}
	</span>
{:else}
	<StatusLabel tone="var(--sb-text-faint)" label="Not checked yet" muted />
{/if}

<style>
	.check {
		display: inline-flex;
		min-width: 0;
		align-items: baseline;
		gap: 8px;
		white-space: nowrap;
	}
	.check.stacked {
		flex-direction: column;
		gap: 1px;
	}
	.origin {
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.stacked .origin {
		padding-left: 15px;
	}
</style>
