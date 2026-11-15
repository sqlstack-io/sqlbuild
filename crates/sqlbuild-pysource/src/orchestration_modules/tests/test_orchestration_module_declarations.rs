use crate::orchestration_modules::main::orchestration_module_declarations::orchestration_module_declarations;
use crate::orchestration_modules::models::{
    ConditionSource, ModuleDeclarations, PythonDeclaration, PythonDeclarationKind,
};
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
            description: "each selecting condition keeps its selection, literal or not, under any import name",
            contents: "from sqlbuild.orchestration import node_event, sensor, source_new_data as fresh\nSELECTION = \"source:raw_orders\"\na = sensor(\"a_sensor\", description=\"d\", when=node_event(select=\"fct_orders fct_refunds\", kinds=[]), run=\"a_job\")\nb = sensor(\"b_sensor\", description=\"d\", when=fresh(select=SELECTION), run=\"b_job\")\n",
            expected_module: Some(ModuleDeclarations {
                declarations: vec![
                    PythonDeclaration {
                        condition: Some(ConditionSource::Selection(text(
                            "fct_orders fct_refunds",
                            3,
                            64,
                        ))),
                        ..declaration(
                            PythonDeclarationKind::Sensor,
                            text("a_sensor", 3, 12),
                            at(3, 5),
                            vec![("run", text("a_job", 3, 105))],
                        )
                    },
                    PythonDeclaration {
                        condition: Some(ConditionSource::Selection(non_literal(4, 59))),
                        ..declaration(
                            PythonDeclarationKind::Sensor,
                            text("b_sensor", 4, 12),
                            at(4, 5),
                            vec![("run", text("b_job", 4, 75))],
                        )
                    },
                ],
                removed_imports: Vec::new(),
            }),
        },
        ScanTestCase {
            description: "a condition bound to a module-level name is followed, and one whose select is unpacked cannot be counted",
            contents: "from sqlbuild.orchestration import node_event, sensor\nFAILED = node_event(select=\"a b c d\", kinds=[\"failure\"])\nOPTIONS = {\"select\": \"a\"}\nc = sensor(\"c_sensor\", description=\"d\", when=FAILED, run=\"c_job\")\nd = sensor(\"d_sensor\", description=\"d\", when=node_event(**OPTIONS), run=\"d_job\")\n",
            expected_module: Some(ModuleDeclarations {
                declarations: vec![
                    PythonDeclaration {
                        condition: Some(ConditionSource::Selection(text("a b c d", 2, 28))),
                        ..declaration(
                            PythonDeclarationKind::Sensor,
                            text("c_sensor", 4, 12),
                            at(4, 5),
                            vec![("run", text("c_job", 4, 58))],
                        )
                    },
                    PythonDeclaration {
                        condition: Some(ConditionSource::Selection(non_literal(5, 46))),
                        ..declaration(
                            PythonDeclarationKind::Sensor,
                            text("d_sensor", 5, 12),
                            at(5, 5),
                            vec![("run", text("d_job", 5, 73))],
                        )
                    },
                ],
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
        ScanTestCase {
            description: "a platform event, a function call, an imported name and a rebinding after the sensor",
            contents: "from orchestration.sensors._conditions import LATE as LATE_ORDERS\nfrom sqlbuild.orchestration import node_event, platform_event, sensor\nTICK = platform_event(kinds=[\"tick\"])\nC = node_event(select=\"a b\", kinds=[])\np = sensor(\"p_sensor\", description=\"d\", when=TICK, run=\"p_job\")\nf = sensor(\"f_sensor\", description=\"d\", when=make_condition(\"x\"), run=\"f_job\")\ni = sensor(\"i_sensor\", description=\"d\", when=LATE_ORDERS, run=\"i_job\")\nr = sensor(\"r_sensor\", description=\"d\", when=C, run=\"r_job\")\nC = platform_event(kinds=[])\n",
            expected_module: Some(ModuleDeclarations {
                declarations: vec![
                    declaration(
                        PythonDeclarationKind::Sensor,
                        text("p_sensor", 5, 12),
                        at(5, 5),
                        vec![("run", text("p_job", 5, 56))],
                    ),
                    PythonDeclaration {
                        condition: Some(ConditionSource::Unresolvable {
                            what: "`make_condition(...)` is a function call".to_owned(),
                            position: at(6, 46),
                        }),
                        ..declaration(
                            PythonDeclarationKind::Sensor,
                            text("f_sensor", 6, 12),
                            at(6, 5),
                            vec![("run", text("f_job", 6, 71))],
                        )
                    },
                    PythonDeclaration {
                        condition: Some(ConditionSource::Imported {
                            module: "orchestration.sensors._conditions".to_owned(),
                            level: 0,
                            name: "LATE".to_owned(),
                            position: at(7, 46),
                        }),
                        ..declaration(
                            PythonDeclarationKind::Sensor,
                            text("i_sensor", 7, 12),
                            at(7, 5),
                            vec![("run", text("i_job", 7, 63))],
                        )
                    },
                    PythonDeclaration {
                        condition: Some(ConditionSource::Selection(text("a b", 4, 23))),
                        ..declaration(
                            PythonDeclarationKind::Sensor,
                            text("r_sensor", 8, 12),
                            at(8, 5),
                            vec![("run", text("r_job", 8, 53))],
                        )
                    },
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
