import type { RunNode, RunNodeStatus } from '$lib/api';

export type BottomTab = 'statements' | 'events' | 'output';

/** The node panel's groups, in display order. */
export type NodeGroupKey = 'running' | 'failed' | 'skipped' | 'succeeded' | 'not_run';
export type NodeGroup = { key: NodeGroupKey; label: string; tone: string; statuses: RunNodeStatus[]; nodes: RunNode[] };

/** Event categories for the Events tab filter. */
export type EventCategory = 'run' | 'nodes' | 'queries' | 'checks';

export type BannerTone = 'info' | 'warn' | 'error';
export type BannerPart = { text: string; href?: string };
export type Banner = { key: string; tone: BannerTone; title: string; body: BannerPart[]; detail?: string };

export type RerunChoice = { mode: 'all' | 'from_failure' | 'selected'; label: string; help: string; disabledReason: string | null };
