<script lang="ts">
	import type { RepublishResult, Scope } from '$lib/api';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { errorMessage } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import GatedButton from './gated-button.svelte';

	/** One line, e.g. "published version 7 [Republish]"; publishing runs the discover functions. */
	type Props = { scope: Scope; ondone?: () => void };
	let { scope, ondone }: Props = $props();

	let publishing = $state(false);
	let result = $state<RepublishResult | null>(null);
	let failure = $state<string | null>(null);

	async function republish() {
		publishing = true;
		result = null;
		failure = null;
		try {
			result = await app.client.republish(scope);
			ondone?.();
		} catch (error) {
			failure = errorMessage(error);
		} finally {
			publishing = false;
		}
	}
</script>

<div class="republish">
	{#if publishing}
		<StatusLabel tone="var(--sb-running)" label="publishing…" />
	{:else if result?.state === 'published'}
		<StatusLabel tone="var(--sb-success)" label="published{result.generation ? ` version ${result.generation}` : ''}" />
	{:else if result?.state === 'failed'}
		<span title={result.error}><StatusLabel tone="var(--sb-error)" label="publish failed" /></span>
	{:else if result?.state === 'expired'}
		<StatusLabel tone="var(--sb-error)" label="request expired" />
	{/if}
	{#if failure}<span class="err" title={failure}>{failure}</span>{/if}
	<GatedButton requires="location.republish" busy={publishing} onclick={republish} title="Compile and publish again, running the discover functions">Republish</GatedButton>
</div>

<style>
	.republish {
		display: flex;
		align-items: center;
		gap: 8px;
		min-width: 0;
		margin-left: 12px;
	}
	.err {
		overflow: hidden;
		color: var(--sb-error);
		font-size: 12px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
