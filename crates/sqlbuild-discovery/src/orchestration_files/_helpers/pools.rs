//! What is wrong with a job's or job group's pools: an undeclared pool, or an amount over a limit.

use std::collections::BTreeMap;

use sqlbuild_config::project::models::PoolLimits;

use crate::orchestration_files::models::RunFile;

/// What is wrong with the run's pools, and how to fix it.
pub(crate) fn pool_problem(
    (kind, run): (&str, &RunFile),
    limits: &PoolLimits,
) -> Option<(String, String)> {
    let name = &run.name.value;
    for (pool, amount) in &run.pools {
        if !limits.declared.contains_key(pool) {
            return Some((
                format!(
                    "{kind} '{name}' uses pool '{pool}', which [pools] in sqlbuild_project.toml \
                     does not declare"
                ),
                declare_help(pool, limits),
            ));
        }
        for (target, limit) in run_limits(run, pool, limits) {
            if amount > &limit {
                let (on, table) = target.map_or_else(
                    || (String::new(), "[pools]".to_owned()),
                    |target| {
                        (
                            format!(" on target '{target}'"),
                            format!("[targets.{target}.pools]"),
                        )
                    },
                );
                return Some((
                    format!(
                        "{kind} '{name}' takes {amount} slots of pool '{pool}', more than its \
                         limit of {limit}{on}"
                    ),
                    format!("take at most {limit}, or raise the limit in {table}"),
                ));
            }
        }
    }
    None
}

/// The pool's limit on each target the run may run in; `None` names the project's own limit.
fn run_limits<'a>(
    run: &'a RunFile,
    pool: &str,
    limits: &'a PoolLimits,
) -> Vec<(Option<&'a str>, u32)> {
    let limit = |pools: &BTreeMap<String, u32>| pools.get(pool).copied().unwrap_or(u32::MAX);
    if run.targets.is_empty() {
        let mut every: Vec<(Option<&str>, u32)> = vec![(None, limit(&limits.declared))];
        for (target, pools) in &limits.targets {
            every.push((Some(target.as_str()), limit(pools)));
        }
        return every;
    }
    let mut chosen: Vec<(Option<&str>, u32)> = Vec::new();
    for target in &run.targets {
        chosen.push((Some(target.as_str()), limit(limits.on(target))));
    }
    chosen
}

/// How to declare `pool`, naming the pools the project declares.
fn declare_help(pool: &str, limits: &PoolLimits) -> String {
    let declare = format!("declare it with its limit, such as {pool} = 1");
    if limits.declared.is_empty() {
        return format!("the project declares no pools; {declare}");
    }
    let names: Vec<&str> = limits.declared.keys().map(String::as_str).collect();
    format!("declared pools: {}; or {declare}", names.join(", "))
}
