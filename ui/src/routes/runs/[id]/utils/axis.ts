// Time axis for the node timeline: adaptive steps from milliseconds to hours.

const STEPS = [0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200];

/** The smallest "nice" step that keeps at most `maxTicks` ticks over `totalSeconds`. */
export function tickStep(totalSeconds: number, maxTicks: number): number {
	const raw = totalSeconds / Math.max(2, maxTicks);
	return STEPS.find((s) => s >= raw) ?? STEPS[STEPS.length - 1];
}

export function axisTicks(totalSeconds: number, step: number): number[] {
	const out: number[] = [];
	for (let i = 0; i * step <= totalSeconds + 1e-9 && out.length < 500; i++) out.push(Number((i * step).toFixed(3)));
	return out;
}

/** Tick label in the step's unit: `250ms`, `1.5s`, `45s`, `2m`, `1m 30s`, `1h 30m`. */
export function formatTick(seconds: number, step: number): string {
	if (seconds === 0) return '0';
	if (step < 1) return seconds < 1 ? `${Math.round(seconds * 1000)}ms` : `${Number(seconds.toFixed(3))}s`;
	if (step < 60) return seconds < 60 ? `${seconds}s` : `${Math.floor(seconds / 60)}m${seconds % 60 ? ` ${seconds % 60}s` : ''}`;
	if (step < 3600) return seconds < 3600 ? `${seconds / 60}m` : `${Math.floor(seconds / 3600)}h${seconds % 3600 ? ` ${(seconds % 3600) / 60}m` : ''}`;
	return `${seconds / 3600}h`;
}

/** A node's duration: `340ms`, `4.2s`, `3m 05s`. */
export function formatSpan(seconds: number): string {
	if (seconds < 1) return `${Math.max(0, Math.round(seconds * 1000))}ms`;
	if (seconds < 10) return `${seconds.toFixed(1)}s`;
	if (seconds < 60) return `${Math.round(seconds)}s`;
	const s = Math.round(seconds);
	if (s < 3600) return `${Math.floor(s / 60)}m ${String(s % 60).padStart(2, '0')}s`;
	return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
}
