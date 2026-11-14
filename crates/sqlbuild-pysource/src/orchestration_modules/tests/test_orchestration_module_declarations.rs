use crate::orchestration_modules::main::orchestration_module_declarations::orchestration_module_declarations;
use crate::orchestration_modules::models::{ModuleDeclarations, PythonDeclarationKind};
use crate::orchestration_modules::tests::helpers::{at, declaration, non_literal, text};
use crate::orchestration_modules::tests::test_types::ScanTestCase;

#[test]
fn given_orchestration_modules_when_scanning_then_reads_declarations_and_removed_imports() {
    let test_cases = [
        ScanTestCase {
            description: "a decorated calendar has its literal name and decorator position",
            contents: "from sqlbuild.orchestration import calendar\n\n@calendar(name=\"trading_days_calendar\", description=\"d\", horizon=\"7d\")\ndef trading_days(window):\n    return []\n",
            expected_module: Some(ModuleDeclarations {
                declarations: vec![declaration(
                    PythonDeclarationKind::Calendar,
                    text("trading_days_calendar", 3, 16),
                    at(3, 2),
                    Vec::new(),
                )],
                removed_imports: Vec::new(),
            }),
        },
        ScanTestCase {
            description: "a sensor reads its run job, then each watched job",
            contents: "from sqlbuild.orchestration import RunEventKind, run_event as watched, sensor\n\nstock = sensor(\n    \"stock_sensor\",\n    description=\"d\",\n    when=watched(job=\"orders_job\", kinds=[RunEventKind.SUCCESS]),\n    run=\"inventory_job\",\n)\n",
            expected_module: Some(ModuleDeclarations {
                declarations: vec![declaration(
                    PythonDeclarationKind::Sensor,
                    text("stock_sensor", 4, 5),
                    at(3, 9),
                    vec![
                        ("run", text("inventory_job", 7, 9)),
                        ("run_event", text("orders_job", 6, 22)),
                    ],
                )],
                removed_imports: Vec::new(),
            }),
        },
        ScanTestCase {
            description: "non-literal names and jobs keep their positions without values",
            contents: "import sqlbuild.orchestration as orchestration\n\nNAME = \"x_sensor\"\nJOB = \"x_job\"\nx = orchestration.sensor(name=NAME, description=\"d\", run=JOB)\n",
            expected_module: Some(ModuleDeclarations {
                declarations: vec![declaration(
                    PythonDeclarationKind::Sensor,
                    non_literal(5, 31),
                    at(5, 5),
                    vec![("run", non_literal(5, 58))],
                )],
                removed_imports: Vec::new(),
            }),
        },
        ScanTestCase {
            description: "calls of other calendar and sensor objects declare nothing",
            contents: "import calendar\nfrom other import sensor\n\ndays = calendar.monthrange(2026, 2)\nsensor(\"not_ours\")\n",
            expected_module: Some(ModuleDeclarations::default()),
        },
        ScanTestCase {
            description: "each removed authoring import is reported at its name",
            contents: "from sqlbuild.orchestration import (\n    job,\n    job_group as group,\n    sensor,\n)\n",
            expected_module: Some(ModuleDeclarations {
                declarations: Vec::new(),
                removed_imports: vec![
                    ("job".to_owned(), at(2, 5)),
                    ("job_group".to_owned(), at(3, 5)),
                ],
            }),
        },
        ScanTestCase {
            description: "a syntax error reads as no module",
            contents: "def f(:\n",
            expected_module: None,
        },
        ScanTestCase {
            description: "calendar methods of other objects do not declare beside an aliased import",
            contents: "import calendar\nfrom sqlbuild.orchestration import calendar as declare\n\n@declare(name=\"days_calendar\", description=\"d\", horizon=\"7d\")\ndef days(window, exchange):\n    calendar.calendar(2026)\n    return exchange.calendar(window.start)\n",
            expected_module: Some(ModuleDeclarations {
                declarations: vec![declaration(
                    PythonDeclarationKind::Calendar,
                    text("days_calendar", 4, 15),
                    at(4, 2),
                    Vec::new(),
                )],
                removed_imports: Vec::new(),
            }),
        },
        ScanTestCase {
            description: "attributes of the orchestration module declare under each import form",
            contents: "import sqlbuild.orchestration\nfrom sqlbuild import orchestration as authoring\n\na = sqlbuild.orchestration.sensor(\"a_sensor\", description=\"d\", run=\"a_job\")\nb = authoring.sensor(\"b_sensor\", description=\"d\", run=\"b_job\")\n",
            expected_module: Some(ModuleDeclarations {
                declarations: vec![
                    declaration(
                        PythonDeclarationKind::Sensor,
                        text("a_sensor", 4, 35),
                        at(4, 5),
                        vec![("run", text("a_job", 4, 68))],
                    ),
                    declaration(
                        PythonDeclarationKind::Sensor,
                        text("b_sensor", 5, 22),
                        at(5, 5),
                        vec![("run", text("b_job", 5, 55))],
                    ),
                ],
                removed_imports: Vec::new(),
            }),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            orchestration_module_declarations(test_case.contents, (3, 12)),
            test_case.expected_module,
            "{}",
            test_case.description
        );
    }
}
