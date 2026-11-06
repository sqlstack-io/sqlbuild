//! Choose the route of a parsed command, or the user error that stops it.

use crate::dispatch::_helpers::subcommands::{
    dbt_dispatch, failure, format_failure, scenario_dispatch,
};
use crate::dispatch::constants::{
    AUDIT_ROUTE, BUILD_ROUTE, CHECK_ROUTE, CLONE_ROUTE, COMPILE_ROUTE, CONTRACT_ROUTE, COST_ROUTE,
    DAG_ROUTE, DEBUG_ROUTE, DIFF_ROUTE, FORMAT_ROUTE, FRESHNESS_ROUTE, INIT_ROUTE, JANITOR_ROUTE,
    LINEAGE_ROUTE, LOAD_ROUTE, LOCATION_ROUTE, PLAN_ROUTE, PLAYGROUND_ROUTE, QUERY_ROUTE,
    RECALL_COMMAND_NAME, RECALL_ROUTE, REFACTOR_ROUTE, REVIEW_ROUTE, RULES_ROUTE, SCOPE_ROUTE,
    SEED_ROUTE, SKILLS_ROUTE, TEST_ROUTE,
};
use crate::dispatch::models::{CommandFacts, Dispatch, DispatchFailure, Route};

/// The route `facts` select, checked in the order the Python dispatcher checked them.
#[must_use]
pub fn command_dispatch(facts: &CommandFacts) -> Dispatch {
    let command: &str = facts.command.as_deref().unwrap_or_default();
    let subcommand: &str = facts.subcommand.as_deref().unwrap_or_default();
    let run = |route: &'static Route| Dispatch::Run(route, None);
    let checked =
        |route: &'static Route, failure: Option<DispatchFailure>| Dispatch::Run(route, failure);
    match command {
        "rename" | "mv" => run(&REFACTOR_ROUTE),
        "contract" => checked(
            &CONTRACT_ROUTE,
            (!facts.contract_ready).then(|| {
                failure(
                    "contract requires an action and --from target",
                    "C474",
                    None,
                )
            }),
        ),
        "compile" => run(&COMPILE_ROUTE),
        "scope" => run(&SCOPE_ROUTE),
        "dag" => run(&DAG_ROUTE),
        "plan" => run(&PLAN_ROUTE),
        "dbt" => dbt_dispatch(subcommand),
        "build" => run(&BUILD_ROUTE),
        "freshness" => run(&FRESHNESS_ROUTE),
        "test" => run(&TEST_ROUTE),
        "check" => run(&CHECK_ROUTE),
        "audit" => run(&AUDIT_ROUTE),
        "load" => run(&LOAD_ROUTE),
        "seed" => run(&SEED_ROUTE),
        "lineage" => run(&LINEAGE_ROUTE),
        "clone" => checked(
            &CLONE_ROUTE,
            (!facts.from_target).then(|| failure("clone requires --from", "C406", None)),
        ),
        "diff" => run(&DIFF_ROUTE),
        "review" => run(&REVIEW_ROUTE),
        "query" => run(&QUERY_ROUTE),
        RECALL_COMMAND_NAME => run(&RECALL_ROUTE),
        "cost" => run(&COST_ROUTE),
        "debug" => run(&DEBUG_ROUTE),
        "location" => run(&LOCATION_ROUTE),
        "janitor" => run(&JANITOR_ROUTE),
        "init" => run(&INIT_ROUTE),
        "playground" => run(&PLAYGROUND_ROUTE),
        "skills" => run(&SKILLS_ROUTE),
        "scenario" => scenario_dispatch(facts, subcommand),
        "format" => checked(&FORMAT_ROUTE, format_failure(facts)),
        "rules" => run(&RULES_ROUTE),
        _ => Dispatch::Nothing,
    }
}
