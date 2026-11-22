use crate::orchestration_modules::main::orchestration_module_declarations::orchestration_module_declarations;
use crate::orchestration_modules::models::{
    ConditionSource, ModuleDeclarations, PythonDeclaration, PythonDeclarationKind, SensorForm,
};
use crate::orchestration_modules::tests::helpers::{at, declaration, non_literal, sensor, text};
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
            description: "a decorated sensor is named after its function and reads its watched and requested jobs",
            contents: "from sqlbuild.orchestration import RunEventKind, RunRequest, run_event as watched, sensor\n\n@sensor(\n    description=\"d\",\n    when=watched(job=\"orders_job\", kinds=[RunEventKind.SUCCESS]),\n)\ndef stock_sensor(ctx):\n    return RunRequest(job=\"inventory_job\")\n",
            expected_module: Some(ModuleDeclarations {
                declarations: vec![sensor(
                    text("stock_sensor", 7, 5),
                    at(3, 2),
                    "stock_sensor",
                    vec![
                        ("run_event", text("orders_job", 5, 22)),
                        ("RunRequest", text("inventory_job", 8, 27)),
                    ],
                )],
                removed_imports: Vec::new(),
            }),
        },
        ScanTestCase {
            description: "name= overrides the function name, and only literal RunRequest jobs are read",
            contents: "from sqlbuild.orchestration import RunRequest, sensor\n\nJOB = \"x_job\"\n\n@sensor(name=\"x_sensor\", description=\"d\")\ndef poll(ctx):\n    return [RunRequest(job=JOB, run_key=\"k\"), RunRequest(job=\"y_job\", run_key=\"j\")]\n",
            expected_module: Some(ModuleDeclarations {
                declarations: vec![sensor(
                    text("x_sensor", 5, 14),
                    at(5, 2),
                    "poll",
                    vec![("RunRequest", text("y_job", 7, 62))],
                )],
                removed_imports: Vec::new(),
            }),
        },
        ScanTestCase {
            description: "each selecting condition keeps its selection, literal or not, under any import name",
            contents: "from sqlbuild.orchestration import check_event, node_event, sensor, source_new_data as fresh\nSELECTION = \"source:raw_orders\"\n\n@sensor(description=\"d\", when=node_event(select=\"fct_orders fct_refunds\", kinds=[]))\ndef a_sensor(ctx):\n    return None\n\n@sensor(description=\"d\", when=fresh(select=SELECTION))\ndef b_sensor(ctx):\n    return None\n\n@sensor(description=\"d\", when=check_event(select=\"fct_orders\", kinds=[]))\ndef c_sensor(ctx):\n    return None\n",
            expected_module: Some(ModuleDeclarations {
                declarations: vec![
                    PythonDeclaration {
                        condition: Some(ConditionSource::Selection(text(
                            "fct_orders fct_refunds",
                            4,
                            49,
                        ))),
                        ..sensor(text("a_sensor", 5, 5), at(4, 2), "a_sensor", Vec::new())
                    },
                    PythonDeclaration {
                        condition: Some(ConditionSource::Selection(non_literal(8, 44))),
                        ..sensor(text("b_sensor", 9, 5), at(8, 2), "b_sensor", Vec::new())
                    },
                    PythonDeclaration {
                        condition: Some(ConditionSource::Selection(text("fct_orders", 12, 50))),
                        ..sensor(text("c_sensor", 13, 5), at(12, 2), "c_sensor", Vec::new())
                    },
                ],
                removed_imports: Vec::new(),
            }),
        },
        ScanTestCase {
            description: "attributes of the orchestration module declare under each import form",
            contents: "import sqlbuild.orchestration\nfrom sqlbuild import orchestration as authoring\n\n@sqlbuild.orchestration.sensor(description=\"d\")\ndef a_sensor(ctx):\n    return None\n\n@authoring.sensor(description=\"d\")\ndef b_sensor(ctx):\n    return None\n",
            expected_module: Some(ModuleDeclarations {
                declarations: vec![
                    sensor(text("a_sensor", 5, 5), at(4, 2), "a_sensor", Vec::new()),
                    sensor(text("b_sensor", 9, 5), at(8, 2), "b_sensor", Vec::new()),
                ],
                removed_imports: Vec::new(),
            }),
        },
        ScanTestCase {
            description: "calls of other calendar and sensor objects declare nothing",
            contents: "import calendar\nfrom other import sensor\n\ndays = calendar.monthrange(2026, 2)\nsensor(\"not_ours\")\n",
            expected_module: Some(ModuleDeclarations::default()),
        },
        ScanTestCase {
            description: "a sensor call that decorates nothing, a bare decorator and a positional name are told apart",
            contents: "from sqlbuild.orchestration import sensor\n\norders = sensor(description=\"d\")\n\n@sensor\ndef bare_sensor(ctx):\n    return None\n\n@sensor(\"named_sensor\", description=\"d\")\ndef positional_sensor(ctx):\n    return None\n",
            expected_module: Some(ModuleDeclarations {
                declarations: vec![
                    PythonDeclaration {
                        name: None,
                        sensor_form: Some(SensorForm::Call),
                        ..sensor(text("x", 1, 1), at(3, 10), "x", Vec::new())
                    },
                    PythonDeclaration {
                        sensor_form: Some(SensorForm::BareDecorator),
                        ..sensor(
                            text("bare_sensor", 6, 5),
                            at(5, 2),
                            "bare_sensor",
                            Vec::new(),
                        )
                    },
                    PythonDeclaration {
                        positional: Some(at(9, 9)),
                        ..sensor(
                            text("positional_sensor", 10, 5),
                            at(9, 2),
                            "positional_sensor",
                            Vec::new(),
                        )
                    },
                ],
                removed_imports: Vec::new(),
            }),
        },
        ScanTestCase {
            description: "a platform event, a function call, an imported name and a rebinding after the sensor",
            contents: "from orchestration.sensors._conditions import LATE as LATE_ORDERS\nfrom sqlbuild.orchestration import node_event, platform_event, sensor\nTICK = platform_event(kinds=[\"tick\"])\nC = node_event(select=\"a b\", kinds=[])\n\n@sensor(description=\"d\", when=TICK)\ndef p_sensor(ctx):\n    return None\n\n@sensor(description=\"d\", when=make_condition(\"x\"))\ndef f_sensor(ctx):\n    return None\n\n@sensor(description=\"d\", when=LATE_ORDERS)\ndef i_sensor(ctx):\n    return None\n\n@sensor(description=\"d\", when=C)\ndef r_sensor(ctx):\n    return None\n\nC = platform_event(kinds=[])\n",
            expected_module: Some(ModuleDeclarations {
                declarations: vec![
                    sensor(text("p_sensor", 7, 5), at(6, 2), "p_sensor", Vec::new()),
                    PythonDeclaration {
                        condition: Some(ConditionSource::Unresolvable {
                            what: "`make_condition(...)` is a function call".to_owned(),
                            position: at(10, 31),
                        }),
                        ..sensor(text("f_sensor", 11, 5), at(10, 2), "f_sensor", Vec::new())
                    },
                    PythonDeclaration {
                        condition: Some(ConditionSource::Imported {
                            module: "orchestration.sensors._conditions".to_owned(),
                            level: 0,
                            name: "LATE".to_owned(),
                            position: at(14, 31),
                        }),
                        ..sensor(text("i_sensor", 15, 5), at(14, 2), "i_sensor", Vec::new())
                    },
                    PythonDeclaration {
                        condition: Some(ConditionSource::Selection(text("a b", 4, 23))),
                        ..sensor(text("r_sensor", 19, 5), at(18, 2), "r_sensor", Vec::new())
                    },
                ],
                removed_imports: Vec::new(),
            }),
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
