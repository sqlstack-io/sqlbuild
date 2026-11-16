//! Public constants of planner entries.

use crate::planner_entries::_helpers::entries::getters::read_model_plan_context;
use crate::planner_entries::_helpers::entries::getters::read_plan_entry_build_inputs;
use crate::planner_entries::_helpers::entries::getters::read_planner_relations_context;
use crate::planner_entries::models::ModelPlanContext;
use crate::planner_entries::models::PlanEntryBuildInputs;
use crate::planner_entries::models::PlannerRelationsContext;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<ModelPlanContext>,
        read: read_model_plan_context,
    },
    GetterEntry {
        class: class_of::<PlanEntryBuildInputs>,
        read: read_plan_entry_build_inputs,
    },
    GetterEntry {
        class: class_of::<PlannerRelationsContext>,
        read: read_planner_relations_context,
    },
];
