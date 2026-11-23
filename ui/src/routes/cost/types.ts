import type { RunStatus } from '$lib/api';

/**
 * The Cost page's read, as the mockups assume it. Fields marked "new" are not in the cost API
 * today (`CostSummary`); see the design notes in the merge request.
 */

/** Where the price per credit came from: the project's `usd_per_credit`, Snowflake's rate sheet, or the public list price. */
export type PriceSource = 'config' | 'rate_sheet' | 'list_price';

export type CreditPrice = {
	usdPerCredit: number;
	source: PriceSource;
	/** Edition and region of a list price, or the rate sheet's contract and date. */
	detail: string;
};

/** One group of the breakdown: a job, folder, warehouse size or model. */
export type CostRow = {
	key: string;
	credits: number;
	/** Credits in the period before, for the change column (new). */
	previous: number;
	seconds: number;
	runs: number;
	/** Models in the group that ran in the period. */
	nodes: number;
};

export type CostModel = CostRow & { folder: string; job: string; queries: number; bytesScanned: number };

/** One expensive run (new: the summary has no run list). */
export type CostRun = {
	id: string;
	/** Null for runs started by hand or from the CLI. */
	job: string | null;
	startedAt: string;
	status: RunStatus;
	credits: number;
	seconds: number;
	queries: number;
	nodes: number;
	size: string;
};

/** One job's cost on one model over the period (new: the API has jobs and models, not both together). */
export type CostFact = {
	job: string;
	model: string;
	/** The model's folder path, such as `staging/partners/web`. */
	folder: string;
	/** The model's share of each of the job's runs. */
	share: number;
	credits: number;
	previous: number;
	seconds: number;
	runs: number;
	queries: number;
	bytesScanned: number;
};

/** One day of the period, split by job and by warehouse size (new). */
export type CostDay = { date: string; credits: number; byJob: Record<string, number>; bySize: Record<string, number> };

export type CostPage = {
	days: number;
	since: string;
	price: CreditPrice;
	credits: number;
	/** Credits in the period before (new). */
	previous: number;
	seconds: number;
	runCount: number;
	incompleteRuns: number;
	jobs: CostRow[];
	folders: CostRow[];
	sizes: CostRow[];
	models: CostModel[];
	runs: CostRun[];
	daily: CostDay[];
	/** Every job-and-model pair with cost (new). */
	facts: CostFact[];
	/** Each job's cost per day, in the order of `daily` (new). */
	jobDaily: Record<string, number[]>;
	jobSize: Record<string, string>;
};

export type Variant = 'a' | 'b' | 'c' | 'd' | 'e' | 'f';
export type Period = 7 | 30;
