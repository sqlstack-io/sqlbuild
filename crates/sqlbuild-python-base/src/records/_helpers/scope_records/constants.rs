//! Layouts of the declaration scope value records hosted natively.

use crate::records::models::{FieldDefault, FieldSpec, RecordSpec};
use crate::records::types::RecordId;

const R_RESOURCE_IDENTITY: RecordSpec = RecordSpec {
    name: "ResourceIdentity",
    doc: "Stable kind-qualified identity of one authored resource.",
    fields: &[FieldSpec::required("kind"), FieldSpec::required("name")],
};
const R_DECLARATION_IDENTITY: RecordSpec = RecordSpec {
    name: "DeclarationIdentity",
    doc: "Stable declaration identity, including a private declaration owner.",
    fields: &[
        FieldSpec::required("kind"),
        FieldSpec::required("name"),
        FieldSpec::new("owner", FieldDefault::None),
    ],
};
const R_OWNERSHIP_ROOT: RecordSpec = RecordSpec {
    name: "OwnershipRoot",
    doc: "Normalized project-relative directory that owns resource scope.",
    fields: &[
        FieldSpec::required("path"),
        FieldSpec::new(
            "kind",
            FieldDefault::NativeMember(RecordId::OwnershipRootKind, "RESOURCE"),
        ),
        FieldSpec::new("resource_kind", FieldDefault::None),
    ],
};
const R_RESOURCE_RECORD: RecordSpec = RecordSpec {
    name: "ResourceRecord",
    doc: "One resource participating in lexical declaration scope.",
    fields: &[
        FieldSpec::required("identity"),
        FieldSpec::required("path"),
        FieldSpec::required("ownership_root"),
    ],
};
const R_MACRO_METADATA: RecordSpec = RecordSpec {
    name: "MacroMetadata",
    doc: "Safe macro metadata required by scope and dependency inspection.",
    fields: &[
        FieldSpec::new("parameters", FieldDefault::EmptyTuple),
        FieldSpec::new("dependencies", FieldDefault::EmptyTuple),
        FieldSpec::new("source_digest", FieldDefault::Text("")),
    ],
};
const R_ENUM_MEMBER_METADATA: RecordSpec = RecordSpec {
    name: "EnumMemberMetadata",
    doc: "One value-free authored enum member identity.",
    fields: &[FieldSpec::required("name")],
};
const R_ENUM_METADATA: RecordSpec = RecordSpec {
    name: "EnumMetadata",
    doc: "Safe enum metadata containing member names and their shared scalar type.",
    fields: &[
        FieldSpec::required("members"),
        FieldSpec::required("scalar_type"),
    ],
};
const R_CONSTANT_METADATA: RecordSpec = RecordSpec {
    name: "ConstantMetadata",
    doc: "Value-free normalized metadata for one typed constant.",
    fields: &[
        FieldSpec::required("logical_type"),
        FieldSpec::new("collection_kind", FieldDefault::None),
        FieldSpec::new("item_count", FieldDefault::None),
        FieldSpec::new("nullable", FieldDefault::False),
        FieldSpec::new("render_as", FieldDefault::None),
    ],
};
const R_DECLARATION_RECORD: RecordSpec = RecordSpec {
    name: "DeclarationRecord",
    doc: "One declaration definition and its compiler-owned scope metadata.",
    fields: &[
        FieldSpec::required("identity"),
        FieldSpec::required("path"),
        FieldSpec::required("line"),
        FieldSpec::required("column"),
        FieldSpec::required("scope"),
        FieldSpec::required("ownership_root"),
        FieldSpec::new("owning_path", FieldDefault::None),
        FieldSpec::new("macro", FieldDefault::None),
        FieldSpec::new("enum", FieldDefault::None),
        FieldSpec::new("constant", FieldDefault::None),
    ],
};
const R_USAGE_RECORD: RecordSpec = RecordSpec {
    name: "UsageRecord",
    doc: "A runtime or declaration dependency edge.",
    fields: &[
        FieldSpec::required("consumer"),
        FieldSpec::required("declaration"),
        FieldSpec::new(
            "kind",
            FieldDefault::NativeMember(RecordId::UsageKind, "RUNTIME"),
        ),
        FieldSpec::new("through", FieldDefault::None),
        FieldSpec::new("enum_member", FieldDefault::None),
    ],
};
const R_GRANT_RECORD: RecordSpec = RecordSpec {
    name: "GrantRecord",
    doc: "A compiler relationship granting a public declaration to a resource.",
    fields: &[
        FieldSpec::required("resource"),
        FieldSpec::required("declaration"),
        FieldSpec::required("through"),
        FieldSpec::new(
            "kind",
            FieldDefault::NativeMember(RecordId::GrantKind, "EXPECTED_MODEL"),
        ),
    ],
};
const R_VISIBILITY_RECORD: RecordSpec = RecordSpec {
    name: "VisibilityRecord",
    doc: "A resolved positive visibility fact with stable provenance.",
    fields: &[
        FieldSpec::required("resource"),
        FieldSpec::required("declaration"),
        FieldSpec::required("reason"),
        FieldSpec::new("through", FieldDefault::None),
    ],
};
const R_INACCESSIBLE_RECORD: RecordSpec = RecordSpec {
    name: "InaccessibleRecord",
    doc: "A resolved negative visibility fact with stable provenance.",
    fields: &[
        FieldSpec::required("resource"),
        FieldSpec::required("declaration"),
        FieldSpec::required("reason"),
    ],
};
const R_SCOPE_DIAGNOSTIC: RecordSpec = RecordSpec {
    name: "ScopeDiagnostic",
    doc: "Stable diagnostic retained even when an index is partial.",
    fields: &[
        FieldSpec::required("code"),
        FieldSpec::required("message"),
        FieldSpec::new(
            "severity",
            FieldDefault::NativeMember(RecordId::ScopeDiagnosticSeverity, "ERROR"),
        ),
        FieldSpec::new("path", FieldDefault::None),
        FieldSpec::new("line", FieldDefault::None),
        FieldSpec::new("column", FieldDefault::None),
        FieldSpec::new("declaration", FieldDefault::None),
        FieldSpec::new("resource", FieldDefault::None),
    ],
};
const R_SCOPE_COMPLETENESS: RecordSpec = RecordSpec {
    name: "ScopeCompleteness",
    doc: "Independent completeness facts for partially invalid projects.",
    fields: &[
        FieldSpec::new("discovery", FieldDefault::True),
        FieldSpec::new("static_visibility", FieldDefault::True),
        FieldSpec::new("runtime_usage", FieldDefault::True),
        FieldSpec::new("relationships", FieldDefault::True),
        FieldSpec::new("placement", FieldDefault::True),
        FieldSpec::new("promotion_impact", FieldDefault::True),
    ],
};
const R_SCOPE_INDEX: RecordSpec = RecordSpec {
    name: "ScopeIndex",
    doc: "Canonical deterministic facts used by every future scope consumer.",
    fields: &[
        FieldSpec::new("ownership_roots", FieldDefault::EmptyTuple),
        FieldSpec::new("resources", FieldDefault::EmptyTuple),
        FieldSpec::new("declarations", FieldDefault::EmptyTuple),
        FieldSpec::new("usages", FieldDefault::EmptyTuple),
        FieldSpec::new("grants", FieldDefault::EmptyTuple),
        FieldSpec::new("visibility", FieldDefault::EmptyTuple),
        FieldSpec::new("inaccessible", FieldDefault::EmptyTuple),
        FieldSpec::new("diagnostics", FieldDefault::EmptyTuple),
        FieldSpec::new(
            "completeness",
            FieldDefault::NativeCall(RecordId::ScopeCompleteness),
        ),
    ],
};
const R_DECLARATION_VISIBILITY_INDEX: RecordSpec = RecordSpec {
    name: "DeclarationVisibilityIndex",
    doc: "Declaration positions grouped by the scope facts that make them visible.",
    fields: &[
        FieldSpec::required("identities"),
        FieldSpec::required("global_positions"),
        FieldSpec::required("private_positions"),
        FieldSpec::required("local_positions"),
        FieldSpec::required("inherited_positions"),
    ],
};
const R_SCOPE_LOOKUP: RecordSpec = RecordSpec {
    name: "ScopeLookup",
    doc: "Immutable indexes over one canonical ``ScopeIndex``.",
    fields: &[
        FieldSpec::required("index"),
        FieldSpec::required("resources"),
        FieldSpec::required("resources_by_path"),
        FieldSpec::required("declarations"),
        FieldSpec::required("usages_by_consumer"),
        FieldSpec::required("usages_by_declaration"),
        FieldSpec::required("grants_by_resource"),
        FieldSpec::required("visibility_index"),
        FieldSpec {
            name: "native",
            default: FieldDefault::None,
            compared: false,
            shown: false,
        },
    ],
};
const R_SCOPE_TARGET_QUERY: RecordSpec = RecordSpec {
    name: "ScopeTargetQuery",
    doc: "Non-throwing qualified-identity or path lookup result.",
    fields: &[
        FieldSpec::required("value"),
        FieldSpec::new("matches", FieldDefault::EmptyTuple),
        FieldSpec::new("declaration_matches", FieldDefault::EmptyTuple),
    ],
};
const R_VISIBILITY_RESOLUTION: RecordSpec = RecordSpec {
    name: "VisibilityResolution",
    doc: "Complete static visibility classification for one queried target.",
    fields: &[
        FieldSpec::required("target"),
        FieldSpec::new("visible", FieldDefault::EmptyTuple),
        FieldSpec::new("inaccessible", FieldDefault::EmptyTuple),
    ],
};
const R_DECLARATION_VISIBILITY: RecordSpec = RecordSpec {
    name: "DeclarationVisibility",
    doc: "Visible facts and inaccessible declaration identities for one queried target.",
    fields: &[
        FieldSpec::required("target"),
        FieldSpec::new("visible", FieldDefault::EmptyTuple),
        FieldSpec::new("inaccessible", FieldDefault::EmptyTuple),
    ],
};
const R_SCOPE_REPORT_FILTERS: RecordSpec = RecordSpec {
    name: "ScopeReportFilters",
    doc: "Orthogonal declaration filters applied in their documented fixed order.",
    fields: &[
        FieldSpec::new(
            "sections",
            FieldDefault::TextTuple(&["available", "used", "relationship_scope"]),
        ),
        FieldSpec::new("include_nearby", FieldDefault::False),
        FieldSpec::new("defined_under", FieldDefault::None),
        FieldSpec::new("kinds", FieldDefault::EmptyTuple),
        FieldSpec::new("glob", FieldDefault::None),
        FieldSpec::new("used_only", FieldDefault::False),
        FieldSpec::new("dependency_depth", FieldDefault::Int(0)),
        FieldSpec::new("sort", FieldDefault::Text("identity")),
        FieldSpec::new("cursor", FieldDefault::None),
        FieldSpec::new("page_size", FieldDefault::Int(100)),
        FieldSpec::new("nearby_depth", FieldDefault::Int(1)),
        FieldSpec::new("globals", FieldDefault::Text("summary")),
    ],
};
const R_SCOPE_SOURCE_LOCATION: RecordSpec = RecordSpec {
    name: "ScopeSourceLocation",
    doc: "Safe project-relative source location.",
    fields: &[
        FieldSpec::required("path"),
        FieldSpec::required("line"),
        FieldSpec::required("column"),
    ],
};
const R_VISIBILITY_PROVENANCE: RecordSpec = RecordSpec {
    name: "VisibilityProvenance",
    doc: "Why a declaration is visible or inaccessible.",
    fields: &[
        FieldSpec::required("reason"),
        FieldSpec::new("through", FieldDefault::None),
    ],
};
const R_DECLARATION_REPORT: RecordSpec = RecordSpec {
    name: "DeclarationReport",
    doc: "Value-free declaration projection used by every report section.",
    fields: &[
        FieldSpec::required("identity"),
        FieldSpec::required("kind"),
        FieldSpec::required("name"),
        FieldSpec::required("owner"),
        FieldSpec::required("definition"),
        FieldSpec::required("scope"),
        FieldSpec::required("owning_path"),
        FieldSpec::new("visibility", FieldDefault::None),
        FieldSpec::new("inaccessible_reason", FieldDefault::None),
        FieldSpec::new("metadata", FieldDefault::EmptyTuple),
        FieldSpec::new("consumers", FieldDefault::EmptyTuple),
        FieldSpec::new("dependencies", FieldDefault::EmptyTuple),
        FieldSpec::new("grants", FieldDefault::EmptyTuple),
        FieldSpec::new("required_scope", FieldDefault::None),
        FieldSpec::new("required_path", FieldDefault::None),
        FieldSpec::new("promotion_impact", FieldDefault::EmptyTuple),
    ],
};
const R_SCOPE_CHAIN_ENTRY: RecordSpec = RecordSpec {
    name: "ScopeChainEntry",
    doc: "One exact or inherited lexical scope in deterministic path order.",
    fields: &[
        FieldSpec::required("kind"),
        FieldSpec::required("path"),
        FieldSpec::required("declaration_count"),
    ],
};
const R_SCOPE_SECTION: RecordSpec = RecordSpec {
    name: "ScopeSection",
    doc: "Pagination and completeness metadata for one report section.",
    fields: &[
        FieldSpec::required("name"),
        FieldSpec::required("total"),
        FieldSpec::required("returned"),
        FieldSpec::new("collapsed", FieldDefault::False),
        FieldSpec::new("collapsed_count", FieldDefault::Int(0)),
        FieldSpec::new("truncated", FieldDefault::False),
        FieldSpec::new("complete", FieldDefault::True),
        FieldSpec::new("next_cursor", FieldDefault::None),
        FieldSpec::new("cursor", FieldDefault::None),
        FieldSpec::new("page_size", FieldDefault::Int(100)),
    ],
};
const R_SCOPE_RESOURCE_REPORT: RecordSpec = RecordSpec {
    name: "ScopeResourceReport",
    doc: "Resolved existing resource or prospective authored path.",
    fields: &[
        FieldSpec::required("target"),
        FieldSpec::required("identity"),
        FieldSpec::required("path"),
        FieldSpec::new("prospective", FieldDefault::False),
        FieldSpec::new("directory", FieldDefault::False),
        FieldSpec::new("duplicate_count", FieldDefault::Int(0)),
    ],
};
const R_DECLARATION_EXPLANATION: RecordSpec = RecordSpec {
    name: "DeclarationExplanation",
    doc: "Detailed explanation for one qualified declaration.",
    fields: &[
        FieldSpec::required("declaration"),
        FieldSpec::required("complete"),
    ],
};
const R_MOVE_PREVIEW: RecordSpec = RecordSpec {
    name: "MovePreview",
    doc: "Pure visibility delta for moving one existing resource.",
    fields: &[
        FieldSpec::required("resource"),
        FieldSpec::required("destination"),
        FieldSpec::required("new_ownership_root"),
        FieldSpec::new("retained", FieldDefault::EmptyTuple),
        FieldSpec::new("gained", FieldDefault::EmptyTuple),
        FieldSpec::new("lost", FieldDefault::EmptyTuple),
        FieldSpec::new("invalidated_usages", FieldDefault::EmptyTuple),
        FieldSpec::new("private_retained", FieldDefault::EmptyTuple),
        FieldSpec::new("relationship_retained", FieldDefault::EmptyTuple),
        FieldSpec::new("complete", FieldDefault::True),
    ],
};
const R_SCOPE_FOLDER: RecordSpec = RecordSpec {
    name: "ScopeFolder",
    doc: "One declaration-definition folder with exact recursive counts.",
    fields: &[
        FieldSpec::required("path"),
        FieldSpec::required("name"),
        FieldSpec::required("descendant_count"),
        FieldSpec::required("child_count"),
        FieldSpec::required("used_count"),
        FieldSpec::required("kind_counts"),
    ],
};
const R_SCOPE_BROWSE_RESULT: RecordSpec = RecordSpec {
    name: "ScopeBrowseResult",
    doc: "Folder-first browse result.",
    fields: &[
        FieldSpec::required("folder"),
        FieldSpec::required("folders"),
        FieldSpec::new("diagnostics", FieldDefault::EmptyTuple),
        FieldSpec::new("complete", FieldDefault::True),
    ],
};
const R_SCOPE_LIST_RESULT: RecordSpec = RecordSpec {
    name: "ScopeListResult",
    doc: "Recursive declaration list beneath one definition folder.",
    fields: &[
        FieldSpec::required("folder"),
        FieldSpec::required("declarations"),
        FieldSpec::required("section"),
        FieldSpec::new("diagnostics", FieldDefault::EmptyTuple),
    ],
};
const R_SCOPE_REPORT: RecordSpec = RecordSpec {
    name: "ScopeReport",
    doc: "Canonical payload shared by all non-rendering scope consumers.",
    fields: &[
        FieldSpec::required("resource"),
        FieldSpec::new("scope_chain", FieldDefault::EmptyTuple),
        FieldSpec::new("available", FieldDefault::EmptyTuple),
        FieldSpec::new("used", FieldDefault::EmptyTuple),
        FieldSpec::new("relationship_scope", FieldDefault::EmptyTuple),
        FieldSpec::new("nearby_unavailable", FieldDefault::EmptyTuple),
        FieldSpec::new(
            "filters",
            FieldDefault::NativeCall(RecordId::ScopeReportFilters),
        ),
        FieldSpec::new("sections", FieldDefault::EmptyTuple),
        FieldSpec::new("explanation", FieldDefault::None),
        FieldSpec::new("move_preview", FieldDefault::None),
        FieldSpec::new("diagnostics", FieldDefault::EmptyTuple),
        FieldSpec::new("complete", FieldDefault::True),
    ],
};

/// The declaration scope value records.
pub(crate) const SCOPE_RECORDS: &[RecordSpec] = &[
    R_RESOURCE_IDENTITY,
    R_DECLARATION_IDENTITY,
    R_OWNERSHIP_ROOT,
    R_RESOURCE_RECORD,
    R_MACRO_METADATA,
    R_ENUM_MEMBER_METADATA,
    R_ENUM_METADATA,
    R_CONSTANT_METADATA,
    R_DECLARATION_RECORD,
    R_USAGE_RECORD,
    R_GRANT_RECORD,
    R_VISIBILITY_RECORD,
    R_INACCESSIBLE_RECORD,
    R_SCOPE_DIAGNOSTIC,
    R_SCOPE_COMPLETENESS,
    R_SCOPE_INDEX,
    R_DECLARATION_VISIBILITY_INDEX,
    R_SCOPE_LOOKUP,
    R_SCOPE_TARGET_QUERY,
    R_VISIBILITY_RESOLUTION,
    R_DECLARATION_VISIBILITY,
    R_SCOPE_REPORT_FILTERS,
    R_SCOPE_SOURCE_LOCATION,
    R_VISIBILITY_PROVENANCE,
    R_DECLARATION_REPORT,
    R_SCOPE_CHAIN_ENTRY,
    R_SCOPE_SECTION,
    R_SCOPE_RESOURCE_REPORT,
    R_DECLARATION_EXPLANATION,
    R_MOVE_PREVIEW,
    R_SCOPE_FOLDER,
    R_SCOPE_BROWSE_RESULT,
    R_SCOPE_LIST_RESULT,
    R_SCOPE_REPORT,
];
