use crate::models::FailureKind;
use crate::project_validation::_helpers::csv_header::csv_header;
use crate::project_validation::main::validate_project_inputs::validate_project_inputs;
use crate::project_validation::models::{
    CsvSettings, DependencyFact, ModelFact, NamedPath, NodeFact, NodeKind, OrchestrationNameFact,
    ProjectFacts, SchemaFileFact, SeedEntryFact, SeedFileFact, ValidationFailure,
};
use crate::project_validation::tests::test_types::failure;

/// The header the reader returns, or its error's message.
pub(super) fn read_header(
    text: &str,
    settings: &CsvSettings,
    field_limit: usize,
) -> Result<Vec<String>, String> {
    csv_header(text.as_bytes(), settings, field_limit).map_err(|error| error.to_string())
}

/// Validation's outcome, the failure shown as `Kind: message`.
pub(super) fn validation_outcome(facts: &ProjectFacts) -> Result<(), String> {
    validate_project_inputs(facts).map_err(|failure: ValidationFailure| failure.to_string())
}

/// Validation's outcome for one seed CSV holding `csv_text` declared with `columns`.
pub(super) fn seed_header_outcome(csv_text: &str, columns: &[&str]) -> Result<(), String> {
    let directory = tempfile::tempdir().expect("a temporary directory is writable");
    let csv_path = directory.path().join("countries.csv");
    std::fs::write(&csv_path, csv_text).expect("the seed CSV is writable");
    let facts = ProjectFacts {
        seeds: vec![SeedFileFact {
            file: named("countries", "seeds/countries.csv"),
            absolute_path: csv_path.to_string_lossy().into_owned(),
        }],
        schemas: vec![SchemaFileFact {
            path: "seeds/countries.yml".to_owned(),
            model_names: Vec::new(),
            seeds: vec![SeedEntryFact {
                name: "countries".to_owned(),
                columns: columns.iter().map(|column| (*column).to_owned()).collect(),
                ..SeedEntryFact::default()
            }],
        }],
        ..ProjectFacts::default()
    };
    validation_outcome(&facts)
}

pub(super) fn named(name: &str, path: &str) -> NamedPath {
    NamedPath {
        name: name.to_owned(),
        path: path.to_owned(),
    }
}

/// A model file at `path` named by its stem.
pub(super) fn model_at(name: &str, path: &str) -> ModelFact {
    ModelFact {
        file: named(name, path),
        local_declarations: Vec::new(),
    }
}

pub(super) fn model(name: &str) -> ModelFact {
    model_at(name, &format!("models/{name}.sql"))
}

pub(super) fn node(kind: NodeKind, name: &str, depends_on: Vec<DependencyFact>) -> NodeFact {
    NodeFact {
        kind,
        name: name.to_owned(),
        path: format!("python/{name}.py"),
        decorator_config: false,
        depends_on,
    }
}

/// A dependency on the decorated function of node `owner`, named `name`.
pub(super) fn on(owner: usize, name: &str) -> DependencyFact {
    DependencyFact::Object {
        owner: Some(owner),
        decorated_name: Some(name.to_owned()),
    }
}

/// Facts for models at `paths` with the path-default `keys`.
pub(super) fn path_default_facts(paths: &[&str], keys: &[&str]) -> ProjectFacts {
    ProjectFacts {
        models: paths.iter().map(|path| model_at("orders", path)).collect(),
        path_default_keys: keys.iter().map(|key| (*key).to_owned()).collect(),
        ..ProjectFacts::default()
    }
}

/// An orchestration declaration named `name`, of `kind`, in one module.
pub(super) fn declared(name: &str, kind: &'static str) -> OrchestrationNameFact {
    OrchestrationNameFact {
        name: name.to_owned(),
        kind,
        path: "orchestration/daily.py".to_owned(),
    }
}

/// A global unique-name conflict that `message` begins.
pub(super) fn collision(message: &str) -> Result<(), String> {
    failure(
        FailureKind::Conflict,
        &format!(
            "{message}; model, source, seed, function, loader, task, asset, check, provider, \
             hook, unit test, scenario, job, job group, sensor, and calendar schedule names must \
             be globally unique"
        ),
    )
}
