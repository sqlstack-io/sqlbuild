<script lang="ts">
	import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
	import HourglassIcon from '@lucide/svelte/icons/hourglass';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import type { Banner } from './types';

	let { banners }: { banners: Banner[] } = $props();
</script>

{#each banners as b (b.key)}
	<div class="banner {b.tone}" role="status">
		<span class="icon">
			{#if b.tone === 'error'}<CircleAlertIcon size={15} />{:else if b.tone === 'warn'}<TriangleAlertIcon size={15} />{:else}<HourglassIcon size={15} />{/if}
		</span>
		<div class="text">
			<strong>{b.title}</strong>
			{#each b.body as part, i (i)}{#if part.href}<a href={part.href}>{part.text}</a>{:else}{part.text}{/if}{/each}
		</div>
		{#if b.detail}
			<details>
				<summary>Details</summary>
				<pre>{b.detail}</pre>
			</details>
		{/if}
	</div>
{/each}

<style>
	.banner {
		--tone: var(--sb-running);
		display: flex;
		align-items: flex-start;
		gap: 10px;
		padding: 9px 24px;
		border-bottom: 1px solid color-mix(in srgb, var(--tone) 30%, var(--border));
		background: color-mix(in srgb, var(--tone) 7%, var(--background));
		font-size: 13px;
	}
	.banner.warn {
		--tone: var(--sb-warning);
	}
	.banner.error {
		--tone: var(--sb-error);
	}
	.icon {
		display: grid;
		height: 20px;
		place-items: center;
		color: var(--tone);
	}
	.text {
		min-width: 0;
		flex: 1;
		line-height: 20px;
	}
	strong {
		margin-right: 6px;
		font-weight: 600;
	}
	a {
		color: var(--primary);
	}
	a:hover {
		text-decoration: underline;
	}
	details {
		max-width: 50%;
		color: var(--muted-foreground);
		font-size: 12px;
	}
	summary {
		cursor: pointer;
		line-height: 20px;
		text-align: right;
	}
	pre {
		margin-top: 4px;
		padding: 6px 8px;
		border-radius: 2px;
		background: var(--sb-inset);
		font-family: var(--font-mono);
		font-size: 11.5px;
		white-space: pre-wrap;
	}
</style>
