<script lang="ts">
	import type { RunStatus } from '$lib/api';
	import { isActiveRun, runStatusHelp, runStatusLabel, runStatusTone } from '$lib/status';

	let { status, size = 'sm' }: { status: RunStatus; size?: 'sm' | 'md' } = $props();
</script>

<span class="pill {size}" class:live={isActiveRun(status) && status !== 'queued'} style:--tone={runStatusTone[status]} title={runStatusHelp[status]}>
	<i></i>{runStatusLabel[status]}
</span>

<style>
	.pill {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 1px 8px 1px 7px;
		border-radius: 999px;
		background: color-mix(in srgb, var(--tone) 13%, transparent);
		color: var(--foreground);
		font-size: 12px;
		font-weight: 500;
		line-height: 18px;
		white-space: nowrap;
	}
	.pill.md {
		padding: 2px 10px 2px 9px;
		font-size: 12.5px;
		line-height: 20px;
	}
	i {
		width: 7px;
		height: 7px;
		flex: 0 0 7px;
		border-radius: 50%;
		background: var(--tone);
	}
	.live i {
		animation: pulse 1.6s ease-in-out infinite;
	}
	@keyframes pulse {
		0%,
		100% {
			box-shadow: 0 0 0 0 color-mix(in srgb, var(--tone) 55%, transparent);
		}
		50% {
			box-shadow: 0 0 0 4px color-mix(in srgb, var(--tone) 0%, transparent);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.live i {
			animation: none;
		}
	}
</style>
