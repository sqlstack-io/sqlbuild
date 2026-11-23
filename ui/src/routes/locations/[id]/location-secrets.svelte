<script lang="ts">
	// Providers the location's environment leaves secrets unset for: a problem for those the target
	// doesn't allow, with each variable, the field that reads it and the nodes that can't run; a quiet note
	// for those its `allow_missing_secrets` lists.
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import InfoIcon from '@lucide/svelte/icons/info';
	import type { MissingProviderSecrets } from '$lib/api/types/platform';
	import { to } from '$lib/paths';
	import { allowedSecretsNote, nodeName, secretsSummary } from './utils';

	let { missing }: { missing: MissingProviderSecrets[] } = $props();
	const problems = $derived(missing.filter((p) => !p.allowed));
	const allowed = $derived(missing.filter((p) => p.allowed));
</script>

{#if problems.length}
	<details class="alert" data-testid="location-missing-secrets">
		<summary>
			<TriangleAlertIcon size={14} />
			<span class="summary-text"><b>Provider secrets missing</b> · {secretsSummary(problems)}</span>
			<span class="more">details</span>
		</summary>
		<div class="alert-body">
			{#each problems as p (p.provider)}
				<div class="provider">
					<p>
						<a href={to(`/providers/${encodeURIComponent(p.provider)}`)}><b>{p.provider}</b></a>
						<span class="faint">{p.path}</span>
					</p>
					<ul>
						{#each p.settings as s (s.variable)}
							<li><code>{s.variable}</code> <span class="faint">for {s.field}{s.line ? `, line ${s.line}` : ''}</span></li>
						{/each}
					</ul>
					{#if p.nodes.length}
						<p>
							Can't run:
							{#each p.nodes as key, i (key)}{i ? ', ' : ''}<a href={to(`/catalog/${key}`)} class="mono">{nodeName(key)}</a>{/each}, and the nodes that read {p.nodes.length === 1 ? 'it' : 'them'}
						</p>
					{/if}
				</div>
			{/each}
			<p>Set each variable in the location's env file, then restart the location (in sqb dev, press r), or in the environment the location (or sqb dev) was started with, then restart it. Until then, runs that use these providers stop before the warehouse.</p>
		</div>
	</details>
{/if}
{#each allowed as p (p.provider)}
	<div class="quiet" data-testid="location-allowed-secrets">
		<InfoIcon size={13} />
		<span>{allowedSecretsNote(p)}</span>
	</div>
{/each}

<style>
	.alert {
		border: 1px solid color-mix(in srgb, var(--sb-warning) 40%, var(--border));
		border-radius: 2px;
		background: color-mix(in srgb, var(--sb-warning) 8%, var(--background));
	}
	.alert summary {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 7px 10px;
		cursor: pointer;
		list-style: none;
	}
	.alert summary::-webkit-details-marker {
		display: none;
	}
	.alert summary > :global(svg) {
		flex: 0 0 auto;
		color: var(--sb-warning);
	}
	.summary-text {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.summary-text b {
		font-weight: 600;
	}
	.more {
		flex: 0 0 auto;
		margin-left: auto;
		color: var(--muted-foreground);
		text-decoration: underline;
	}
	.alert[open] .more {
		visibility: hidden;
	}
	.alert-body {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 0 10px 10px 32px;
		color: var(--muted-foreground);
	}
	.provider p,
	.provider ul {
		margin: 0;
	}
	.provider ul {
		padding-left: 16px;
	}
	.provider a {
		color: var(--foreground);
	}
	code,
	.mono {
		font-family: var(--font-mono);
		font-size: 11.5px;
	}
	.faint {
		color: var(--muted-foreground);
	}
	.quiet {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 2px 10px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
</style>
