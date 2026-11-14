use crate::orchestration_modules::main::orchestration_module_declarations::orchestration_module_declarations;
use crate::orchestration_modules::models::{
    LiteralArgument, ModuleDeclarations, PythonDeclarationKind, SourcePosition,
};

fn at(line: usize, column: usize) -> SourcePosition {
    SourcePosition { line, column }
}

fn text(value: &str, line: usize, column: usize) -> LiteralArgument {
    LiteralArgument {
        value: Some(value.to_owned()),
        position: at(line, column),
    }
}

fn scan(contents: &str) -> ModuleDeclarations {
    orchestration_module_declarations(contents, (3, 12)).expect("module parses")
}

#[test]
fn given_decorated_calendar_when_scanning_then_reads_its_literal_name() {
    let module = scan(
        "from sqlbuild.orchestration import calendar\n\n@calendar(name=\"trading_days_calendar\", description=\"d\", horizon=\"7d\")\ndef trading_days(window):\n    return []\n",
    );
    assert_eq!(module.declarations.len(), 1);
    let calendar = &module.declarations[0];
    assert_eq!(calendar.kind, PythonDeclarationKind::Calendar);
    assert_eq!(calendar.name, Some(text("trading_days_calendar", 3, 16)));
    assert_eq!(calendar.position, at(3, 2));
    assert!(calendar.job_references.is_empty());
}

#[test]
fn given_sensor_when_scanning_then_reads_run_and_watched_job_in_order() {
    let module = scan(
        "from sqlbuild.orchestration import RunEventKind, run_event as watched, sensor\n\nstock = sensor(\n    \"stock_sensor\",\n    description=\"d\",\n    when=watched(job=\"orders_job\", kinds=[RunEventKind.SUCCESS]),\n    run=\"inventory_job\",\n)\n",
    );
    let sensor = &module.declarations[0];
    assert_eq!(sensor.kind, PythonDeclarationKind::Sensor);
    assert_eq!(sensor.name, Some(text("stock_sensor", 4, 5)));
    assert_eq!(
        sensor.job_references,
        vec![
            ("run", text("inventory_job", 7, 9)),
            ("run_event", text("orders_job", 6, 22)),
        ]
    );
}

#[test]
fn given_non_literal_references_when_scanning_then_keeps_their_positions_without_values() {
    let module = scan(
        "import sqlbuild.orchestration as orchestration\n\nNAME = \"x_sensor\"\nJOB = \"x_job\"\nx = orchestration.sensor(name=NAME, description=\"d\", run=JOB)\n",
    );
    let sensor = &module.declarations[0];
    assert_eq!(
        sensor.name,
        Some(LiteralArgument {
            value: None,
            position: at(5, 31),
        })
    );
    assert_eq!(sensor.job_references[0].1.value, None);
    assert_eq!(sensor.job_references[0].1.position, at(5, 58));
}

#[test]
fn given_unrelated_calls_when_scanning_then_ignores_them() {
    let module = scan(
        "import calendar\nfrom other import sensor\n\ndays = calendar.monthrange(2026, 2)\nsensor(\"not_ours\")\n",
    );
    assert!(module.declarations.is_empty());
    assert!(module.removed_imports.is_empty());
}

#[test]
fn given_removed_authoring_imports_when_scanning_then_reports_each() {
    let module = scan(
        "from sqlbuild.orchestration import (\n    job,\n    job_group as group,\n    sensor,\n)\n",
    );
    assert_eq!(
        module.removed_imports,
        vec![
            ("job".to_owned(), at(2, 5)),
            ("job_group".to_owned(), at(3, 5))
        ]
    );
}

#[test]
fn given_syntax_error_when_scanning_then_returns_none() {
    assert_eq!(
        orchestration_module_declarations("def f(:\n", (3, 12)),
        None
    );
}

#[test]
fn given_other_objects_calendar_and_sensor_methods_when_scanning_then_only_orchestration_calls_declare() {
    let module = scan(
        "import calendar\nfrom sqlbuild.orchestration import calendar as declare\n\n@declare(name=\"days_calendar\", description=\"d\", horizon=\"7d\")\ndef days(window, exchange):\n    calendar.calendar(2026)\n    return exchange.calendar(window.start)\n",
    );
    assert_eq!(module.declarations.len(), 1);
    assert_eq!(module.declarations[0].name, Some(text("days_calendar", 4, 15)));
}

#[test]
fn given_orchestration_module_attributes_when_scanning_then_each_import_form_declares() {
    let module = scan(
        "import sqlbuild.orchestration\nfrom sqlbuild import orchestration as authoring\n\na = sqlbuild.orchestration.sensor(\"a_sensor\", description=\"d\", run=\"a_job\")\nb = authoring.sensor(\"b_sensor\", description=\"d\", run=\"b_job\")\n",
    );
    let names: Vec<Option<String>> = module
        .declarations
        .iter()
        .map(|declaration| declaration.name.as_ref().and_then(|name| name.value.clone()))
        .collect();
    assert_eq!(
        names,
        vec![Some("a_sensor".to_owned()), Some("b_sensor".to_owned())]
    );
}
