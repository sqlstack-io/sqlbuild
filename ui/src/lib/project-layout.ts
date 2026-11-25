// Where a project's files live, and the folders the UI groups nodes by: SQL models by their folder
// below `sql/models/`, other SQL resources by their path below `sql/` (`seeds/…`, `sources/raw`), and
// Python nodes as `python/<folder below python/nodes/>`. The one place that knows the layout's roots. Pure.

/** The project folder of everything the warehouse runs. */
export const SQL_ROOT = 'sql';
/** The project folder of SQL models; a `path:` selector may start here. */
export const MODELS_ROOT = 'sql/models';
/** The project folder of Python nodes; a `path:` selector may start here. */
export const PYTHON_NODES_ROOT = 'python/nodes';
/** The folder Python nodes group under. */
export const PYTHON_FOLDER = 'python';
/** The folders `sql/` holds besides `models/`, which group by their own name. */
export const SQL_KIND_FOLDERS = ['seeds', 'sources', 'macros', 'functions', 'enums', 'constants', 'audits', 'hooks', 'schemas'] as const;
/** The roots a `path:` selector accepts; anything else is refused (S012). */
export const PATH_SELECTOR_ROOTS = [MODELS_ROOT, PYTHON_NODES_ROOT] as const;

const below = (dir: string, root: string): string | null => (dir === root ? '' : dir.startsWith(`${root}/`) ? dir.slice(root.length + 1) : null);

/**
 * A project directory as the folder the UI groups by: `sql/models/marts/core` is `marts/core` (and
 * `sql/models` itself is empty), `sql/seeds/geo` is `seeds/geo`, `python/nodes/exports` is
 * `python/exports`. A directory outside the roots stays as it is.
 */
export function folderOfDir(dir: string): string {
	const trimmed = dir.replace(/\/+$/, '');
	const model = below(trimmed, MODELS_ROOT);
	if (model !== null) return model;
	const python = below(trimmed, PYTHON_NODES_ROOT);
	if (python !== null) return python ? `${PYTHON_FOLDER}/${python}` : PYTHON_FOLDER;
	const sql = below(trimmed, SQL_ROOT);
	if (sql) return sql;
	return trimmed;
}

/** The project directory of a folder `folderOfDir` gives: `marts` is `sql/models/marts`, `seeds/geo` is `sql/seeds/geo`, `python/exports` is `python/nodes/exports`. */
export function dirOfFolder(folder: string): string {
	const trimmed = folder.replace(/\/+$/, '');
	const python = below(trimmed, PYTHON_FOLDER);
	if (python !== null) return python ? `${PYTHON_NODES_ROOT}/${python}` : PYTHON_NODES_ROOT;
	const top = trimmed.split('/')[0];
	if ((SQL_KIND_FOLDERS as readonly string[]).includes(top)) return `${SQL_ROOT}/${trimmed}`;
	return trimmed ? `${MODELS_ROOT}/${trimmed}` : MODELS_ROOT;
}

/** Whether a `path:` selector can name the directory: a folder below one of its roots. */
export const canSelectByPath = (dir: string) => PATH_SELECTOR_ROOTS.some((root) => dir.startsWith(`${root}/`));
