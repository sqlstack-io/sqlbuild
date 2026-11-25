import { describe, expect, it } from 'vitest';
import { canSelectByPath, dirOfFolder, folderOfDir } from './project-layout';

describe('the folders a project directory groups under', () => {
	it.each([
		['a model folder below sql/models', 'sql/models/marts/core', 'marts/core'],
		['the top of sql/models', 'sql/models', ''],
		['a seed folder below sql', 'sql/seeds/geo', 'seeds/geo'],
		['the seeds folder', 'sql/seeds', 'seeds'],
		['the functions folder', 'sql/functions', 'functions'],
		['a sources folder', 'sql/sources/raw', 'sources/raw'],
		['a scoped declaration folder, in its owner', 'sql/models/staging/_sqlbuild/functions', 'staging/_sqlbuild/functions'],
		['a Python node folder below python/nodes', 'python/nodes/exports/csv', 'python/exports/csv'],
		['the top of python/nodes', 'python/nodes', 'python'],
		['a trailing slash', 'sql/models/staging/', 'staging'],
		['a directory outside the roots', 'legacy/models', 'legacy/models']
	])('%s', (_, dir, folder) => {
		expect(folderOfDir(dir)).toBe(folder);
	});

	it.each([
		['a model folder', 'marts/core', 'sql/models/marts/core'],
		['the top of sql/models', '', 'sql/models'],
		['a seed folder', 'seeds/geo', 'sql/seeds/geo'],
		['a source file', 'sources/raw', 'sql/sources/raw'],
		['a Python folder', 'python/exports', 'python/nodes/exports'],
		['the Python folder', 'python', 'python/nodes'],
		['a scoped declaration folder', 'staging/_sqlbuild/functions', 'sql/models/staging/_sqlbuild/functions']
	])('maps %s back to its directory', (_, folder, dir) => {
		expect(dirOfFolder(folder)).toBe(dir);
		expect(folderOfDir(dir)).toBe(folder);
	});

	it('lets a path selector name folders below sql/models and python/nodes only', () => {
		expect(['sql/models/marts', 'python/nodes/exports', 'sql/models', 'sql/seeds/geo', 'models/marts', 'python/exports'].map(canSelectByPath)).toEqual([true, true, false, false, false, false]);
	});
});
