<script lang="ts">
	import type { PermissionRisk } from '$lib/api';
	import { RISK_LABEL, RISK_TONE } from '$lib/platform/permission-catalog';

	let { risk }: { risk: PermissionRisk } = $props();
</script>

<span class="risk" style:--tone={RISK_TONE[risk]} title="Risk: {RISK_LABEL[risk]}">{RISK_LABEL[risk]}</span>

<style>
	.risk {
		display: inline-flex;
		align-items: center;
		padding: 0 6px;
		border: 1px solid color-mix(in srgb, var(--tone) 45%, var(--border));
		border-radius: 2px;
		background: color-mix(in srgb, var(--tone) 8%, transparent);
		color: color-mix(in srgb, var(--tone) 80%, var(--foreground));
		font-size: 11px;
		line-height: 17px;
		white-space: nowrap;
	}
</style>
