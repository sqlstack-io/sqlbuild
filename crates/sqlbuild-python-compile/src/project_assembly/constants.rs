//! Public constants of project assembly.

use crate::project_assembly::_helpers::getters::read_native_model_facts;
use crate::project_assembly::_helpers::getters::read_native_project_facts;
use crate::project_assembly::_helpers::getters::read_native_project_resources;
use crate::project_assembly::_helpers::getters::read_native_resource_failure;
use crate::project_assembly::models::NativeModelFacts;
use crate::project_assembly::models::NativeProjectFacts;
use crate::project_assembly::models::NativeProjectResources;
use crate::project_assembly::models::NativeResourceFailure;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<NativeModelFacts>,
        read: read_native_model_facts,
    },
    GetterEntry {
        class: class_of::<NativeProjectFacts>,
        read: read_native_project_facts,
    },
    GetterEntry {
        class: class_of::<NativeProjectResources>,
        read: read_native_project_resources,
    },
    GetterEntry {
        class: class_of::<NativeResourceFailure>,
        read: read_native_resource_failure,
    },
];
