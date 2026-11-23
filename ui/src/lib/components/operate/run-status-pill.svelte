<script lang="ts">
	import type { RunStatus } from '$lib/api';
	import { ignoresAuditFailures, isActiveRun, runStatusHelp, runStatusLabel, runStatusTone } from '$lib/status';
	import { plural } from '$lib/format/plural';

	/** `ignored` counts the audit failures a finished run let through: it reads as a success with a warning, never as a failure. */
	/** `notStarted`: a group's member run that never started, which reads "Not started", never "Skipped". */
	let { status, size = 'sm', ignored = 0, notStarted = false }: { status: RunStatus; size?: 'sm' | 'md'; ignored?: number; notStarted?: boolean } = $props();
	const letThrough = $derived(ignoresAuditFailures(status, ignored));
</script>

<span
	class="pill {size}"
	class:hollow={notStarted}
	class:live={isActiveRun(status) && status !== 'queued'}
	style:--tone={letThrough ? 'var(--sb-warning)' : notStarted ? 'var(--sb-text-faint)' : runStatusTone[status]}
	title={notStarted ? 'The group stopped before this member job started' : letThrough ? `${runStatusLabel[status]}, letting through ${plural(ignored, 'error-severity audit failure')}: audit failures don't stop this job` : runStatusHelp[status]}
	data-testid="run-status"
>
	<i></i>{notStarted ? 'Not started' : runStatusLabel[status]}{#if letThrough}<span class="ignored-count">{` · ${plural(ignored, 'audit failure')} ignored`}</span>{/if}
</span>

<style>
	.pill.hollow {
		background: transparent;
		box-shadow: inset 0 0 0 1px var(--border);
	}
	.pill.hollow i {
		background: transparent !important;
		box-shadow: inset 0 0 0 1px var(--sb-text-faint);
	}
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
	.ignored-count {
		color: color-mix(in srgb, var(--sb-warning) 70%, var(--foreground));
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
