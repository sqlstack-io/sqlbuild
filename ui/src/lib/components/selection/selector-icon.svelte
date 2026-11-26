<script lang="ts">
	import TagIcon from '@lucide/svelte/icons/tag';
	import FolderIcon from '@lucide/svelte/icons/folder';
	import BriefcaseIcon from '@lucide/svelte/icons/briefcase';
	import ShapesIcon from '@lucide/svelte/icons/shapes';
	import DatabaseIcon from '@lucide/svelte/icons/database';
	import SproutIcon from '@lucide/svelte/icons/sprout';
	import FlaskIcon from '@lucide/svelte/icons/flask-conical';
	import BoxIcon from '@lucide/svelte/icons/box';
	import HistoryIcon from '@lucide/svelte/icons/history';
	import CirclePlayIcon from '@lucide/svelte/icons/circle-play';
	import CircleXIcon from '@lucide/svelte/icons/circle-x';
	import SkipForwardIcon from '@lucide/svelte/icons/skip-forward';
	import ShieldCheckIcon from '@lucide/svelte/icons/shield-check';
	import type { Component } from 'svelte';

	// The icon of a selector type; a bare name is a node, or a folder when it holds `/`.
	let { type, folder = false, recent = false }: { type: string | null; folder?: boolean; recent?: boolean } = $props();

	const ICONS: Record<string, Component<{ size?: number }>> = { tag: TagIcon, path: FolderIcon, job: BriefcaseIcon, kind: ShapesIcon, source: DatabaseIcon, seed: SproutIcon, test: FlaskIcon, audit: ShieldCheckIcon, run: CirclePlayIcon, failed: CircleXIcon, skipped: SkipForwardIcon };
	const Icon = $derived(recent ? HistoryIcon : type ? (ICONS[type] ?? BoxIcon) : folder ? FolderIcon : BoxIcon);
</script>

<span class="selector-icon"><Icon size={13} /></span>

<style>
	.selector-icon {
		display: inline-flex;
		flex: none;
		color: var(--sb-text-faint);
	}
</style>
