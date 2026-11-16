//! Public constants of compiled views.

use crate::compiled_views::_helpers::project::getters::read_native_audit_input;
use crate::compiled_views::_helpers::project::getters::read_native_compiled_audit;
use crate::compiled_views::_helpers::project::getters::read_native_compiled_function;
use crate::compiled_views::_helpers::project::getters::read_native_compiled_model;
use crate::compiled_views::_helpers::project::getters::read_native_compiled_seed;
use crate::compiled_views::_helpers::project::getters::read_native_compiled_source;
use crate::compiled_views::_helpers::project::getters::read_native_function_input;
use crate::compiled_views::_helpers::project::getters::read_native_inputs_view;
use crate::compiled_views::_helpers::project::getters::read_native_model_input;
use crate::compiled_views::_helpers::project::getters::read_native_project_view;
use crate::compiled_views::_helpers::project::getters::read_native_seed_input;
use crate::compiled_views::_helpers::project::getters::read_native_source_input;
use crate::compiled_views::models::NativeCompiledModel;
use crate::compiled_views::models::NativeInputsView;
use crate::compiled_views::models::NativeModelInput;
use crate::compiled_views::models::NativeProjectView;
use crate::resource_views::models::NativeAuditInput;
use crate::resource_views::models::NativeCompiledAudit;
use crate::resource_views::models::NativeCompiledFunction;
use crate::resource_views::models::NativeCompiledSeed;
use crate::resource_views::models::NativeCompiledSource;
use crate::resource_views::models::NativeFunctionInput;
use crate::resource_views::models::NativeSeedInput;
use crate::resource_views::models::NativeSourceInput;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<NativeAuditInput>,
        read: read_native_audit_input,
    },
    GetterEntry {
        class: class_of::<NativeCompiledAudit>,
        read: read_native_compiled_audit,
    },
    GetterEntry {
        class: class_of::<NativeCompiledModel>,
        read: read_native_compiled_model,
    },
    GetterEntry {
        class: class_of::<NativeCompiledFunction>,
        read: read_native_compiled_function,
    },
    GetterEntry {
        class: class_of::<NativeFunctionInput>,
        read: read_native_function_input,
    },
    GetterEntry {
        class: class_of::<NativeInputsView>,
        read: read_native_inputs_view,
    },
    GetterEntry {
        class: class_of::<NativeModelInput>,
        read: read_native_model_input,
    },
    GetterEntry {
        class: class_of::<NativeProjectView>,
        read: read_native_project_view,
    },
    GetterEntry {
        class: class_of::<NativeCompiledSeed>,
        read: read_native_compiled_seed,
    },
    GetterEntry {
        class: class_of::<NativeSeedInput>,
        read: read_native_seed_input,
    },
    GetterEntry {
        class: class_of::<NativeCompiledSource>,
        read: read_native_compiled_source,
    },
    GetterEntry {
        class: class_of::<NativeSourceInput>,
        read: read_native_source_input,
    },
];
