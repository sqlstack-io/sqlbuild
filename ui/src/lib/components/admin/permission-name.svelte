<script lang="ts">
	import type { Capability } from '$lib/api';
	import { PERMISSIONS } from '$lib/platform/permission-catalog';
	import RiskTag from './risk-tag.svelte';

	// An action in plain words, with its code beside it and optionally its risk.
	let { action, code = true, risk = false }: { action: Capability; code?: boolean; risk?: boolean } = $props();
	const info = $derived(PERMISSIONS[action]);
</script>

<span class="perm" title={info.description}>
	<span class="label">{info.label}</span>
	{#if code}<code>{action}</code>{/if}
	{#if risk}<RiskTag risk={info.risk} />{/if}
</span>

<style>
	.perm {
		display: inline-flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 3px 7px;
	}
	code {
		color: var(--muted-foreground);
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
</style>
