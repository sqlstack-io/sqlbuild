// Mockup data (discovered-node variants): two discovered sets, each feeding a `for_each` asset whose
// items become one node per step, as `asset:<asset>[<step>]`, plus a node completing the set.
// One set is fresh; the other's newest fetch failed, so its nodes keep the last good items. Pure.

import type { SnapshotNode } from '../types';
import type { DiscoveredSet } from '../types/build';

const MIN = 60_000;
const HOUR = 60 * MIN;

type Step = [step: string, upstream: string[], args?: Record<string, string>];

/** The EU forecast plan: 36 steps, as a model release lists them. */
const EU: Step[] = [
	['id_mappings', []],
	['seasonal_data', ['id_mappings']],
	['regional_seasonal_data', ['id_mappings']],
	['regional_scores', ['regional_seasonal_data']],
	['regional_inputs', ['regional_scores', 'seasonal_data']],
	['cleaned_sales', ['id_mappings']],
	['hierarchy_data', ['id_mappings']],
	['hierarchy_category', ['hierarchy_data']],
	['hierarchy_supplier', ['hierarchy_data']],
	['hierarchy_product', ['hierarchy_category', 'hierarchy_supplier']],
	['popularity_scores', ['seasonal_data', 'cleaned_sales']],
	['gbm_inputs', ['regional_inputs', 'popularity_scores']],
	['gbm_chain_d1', ['gbm_inputs'], { depth: '1' }],
	['gbm_chain_d3', ['gbm_inputs'], { depth: '3' }],
	['gbm_gru', ['gbm_chain_d1', 'gbm_chain_d3']],
	['itemgraph', ['hierarchy_product', 'regional_inputs']],
	['itemloop', ['itemgraph']],
	['itemcluster', ['itemgraph', 'popularity_scores']],
	['transformer_inputs', ['regional_inputs', 'hierarchy_product']],
	['transformer_predictions', ['transformer_inputs']],
	['topk_predict_base', ['gbm_gru', 'itemloop'], { k: '5' }],
	['itemrnnboost', ['itemcluster', 'cleaned_sales']],
	['itemcorr', ['itemloop', 'itemcluster']],
	['model_meta_features', ['gbm_gru', 'itemrnnboost', 'itemcorr', 'transformer_predictions']],
	['topk_predictions', ['topk_predict_base', 'model_meta_features'], { k: '5' }],
	['blender_inputs', ['model_meta_features']],
	['blender_training', ['blender_inputs', 'topk_predictions'], { gpu: 'true' }],
	['blender_predictions', ['blender_training']],
	['store_calibration', ['blender_predictions']],
	['channel_blend', ['store_calibration']],
	['confidence_bands', ['store_calibration']],
	['forecast_export', ['channel_blend', 'confidence_bands']],
	['forecast_qa', ['forecast_export']],
	['feature_importance', ['blender_training']],
	['drift_report', ['model_meta_features']],
	['forecast_publish', ['forecast_qa']]
];

/** The US plan: a few steps, whose newest fetch failed. */
const US: Step[] = [
	['id_mappings', []],
	['seasonal_data', ['id_mappings']],
	['gbm_chain_d1', ['seasonal_data'], { depth: '1' }],
	['itemloop', ['seasonal_data']],
	['blender_training', ['gbm_chain_d1', 'itemloop'], { gpu: 'true' }],
	['forecast_publish', ['blender_training']]
];

type Plan = { set: string; asset: string; job: string; region: string; steps: Step[] };

export const DISCOVERED_PLANS: Plan[] = [
	{ set: 'forecast_eu_plan', asset: 'forecast_eu', job: 'forecast_v2_eu_job', region: 'EU', steps: EU },
	{ set: 'forecast_us_plan', asset: 'forecast_us', job: 'forecast_v2_us_job', region: 'US', steps: US }
];

const stepKey = (asset: string, step: string) => `asset:${asset}[${step}]`;
export const completeKey = (asset: string) => `asset:${asset}_complete`;
const pathOf = (asset: string) => `src/forecast_pipelines/definitions/models/regions/${asset.split('_')[1]}/discovered_model_steps.py`;

/** Every step node and each set's completing node, in the snapshot's shape. */
export const DISCOVERED_NODES: SnapshotNode[] = DISCOVERED_PLANS.flatMap((p) => {
	const group = `forecast_${p.region.toLowerCase()}`;
	const steps = p.steps.map(([step, upstream]): SnapshotNode => ({ key: stepKey(p.asset, step), kind: 'asset', group, tags: ['forecast', 'discovered'], path: pathOf(p.asset), cursor: null, upstream: upstream.map((u) => stepKey(p.asset, u)) }));
	const leaves = p.steps.filter(([step]) => !p.steps.some(([, up]) => up.includes(step))).map(([step]) => stepKey(p.asset, step));
	return [...steps, { key: completeKey(p.asset), kind: 'asset', group, tags: ['forecast'], path: pathOf(p.asset), cursor: null, upstream: leaves }];
});

/** The sets as `readDiscovered` would report them, with the nodes each item became. */
export function discoveredSets(now: number): DiscoveredSet[] {
	const at = (ms: number) => new Date(now - ms).toISOString();
	const [eu, us] = DISCOVERED_PLANS;
	const nodes = (p: Plan) => p.steps.map(([step, , args]) => ({ key: stepKey(p.asset, step), asset: p.asset, step, args: { step, region: p.region, ...args } }));
	return [
		{
			name: eu.set,
			status: 'ok',
			items: eu.steps.length,
			fetchedAt: at(2 * HOUR + 14 * MIN),
			lastGoodAt: at(2 * HOUR + 14 * MIN),
			digest: 'sha256:3be1c0a94f27d8e6b1a05c3f9e72d4a816c0b59e2f37a1d8c4e6b09f5a2d7c13',
			error: null,
			checkEverySeconds: 900,
			tags: { release: 'f59ed676a9f8c41b2e07d3a5c96b8e14f2a7d0c3', plan_digest: '3be1c0a94f27', max_parallel: '4', region: 'EU' },
			assets: [eu.asset],
			nodes: nodes(eu)
		},
		{
			name: us.set,
			status: 'failed',
			items: us.steps.length,
			fetchedAt: at(11 * MIN),
			lastGoodAt: at(2 * 24 * HOUR + 3 * HOUR),
			digest: 'sha256:9d41e7b2c0a85f3e6d17b94c2a0e58f1d3b6c79a4e2f08d5b1c7a93e6f40d82b',
			error: 'GET https://releases.example.com/us/latest/plan.json: 503 Service Unavailable',
			checkEverySeconds: 900,
			tags: { release: '7c2e19b04d6a3f85e1b97c0d2a4f6e38b5c1d907', plan_digest: '9d41e7b2c0a8', max_parallel: '2', region: 'US' },
			assets: [us.asset],
			nodes: nodes(us)
		}
	];
}
