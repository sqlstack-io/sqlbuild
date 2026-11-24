use crate::dag_artifact::errors::DagArtifactError;
use crate::dag_artifact::models::{DagProjectInput, DagPythonGraphInput};
use crate::dag_artifact::tests::helpers::{
    built, dag_edges, key, loader, python_node, source, team_meta,
};
use crate::dag_artifact::tests::test_types::{BuildDagJsonTestCase, DagEdgeTestCase};
use crate::dag_artifact::types::DagValue;

const EMPTY_JSON: &str = "{
  \"version\": 1,
  \"project_name\": \"orders_project\",
  \"nodes\": [],
  \"edges\": [],
  \"checks\": []
}";

const LOADED_JSON: &str = "{
  \"version\": 1,
  \"project_name\": \"orders_project\",
  \"nodes\": [
    {
      \"id\": \"source:orders\",
      \"kind\": \"source\",
      \"name\": \"orders\",
      \"asset_key\": [
        \"raw\",
        \"orders\"
      ],
      \"target\": {
        \"schema\": \"raw\",
        \"name\": \"orders\",
        \"qualified_name\": \"raw.orders\"
      },
      \"path\": \"sql/sources/raw.yml\",
      \"meta\": {
        \"team\": \"ops\"
      },
      \"columns\": [
        {
          \"name\": \"id\",
          \"type\": \"INTEGER\"
        }
      ]
    },
    {
      \"id\": \"loader:load_customers\",
      \"kind\": \"loader\",
      \"name\": \"load_customers\",
      \"asset_key\": [
        \"load_customers\"
      ],
      \"target\": {
        \"schema\": \"main\",
        \"name\": \"__loader__load_customers\",
        \"qualified_name\": \"main.__loader__load_customers\"
      },
      \"path\": \"python/nodes/loaders.py\",
      \"meta\": {
        \"sqlbuild_loader_node\": true
      },
      \"loader\": \"load_customers\"
    },
    {
      \"id\": \"check:orders_check\",
      \"kind\": \"check\",
      \"name\": \"orders_check\",
      \"asset_key\": [
        \"check\",
        \"orders_check\"
      ],
      \"path\": \"python/nodes/nodes.py\"
    }
  ],
  \"edges\": [
    {
      \"from_id\": \"source:orders\",
      \"to_id\": \"check:orders_check\"
    },
    {
      \"from_id\": \"source:orders\",
      \"to_id\": \"loader:load_customers\"
    }
  ],
  \"checks\": [
    {
      \"id\": \"check:orders_check\",
      \"kind\": \"python_check\",
      \"name\": \"orders_check\",
      \"checked_asset_ids\": [
        \"source:orders\"
      ],
      \"path\": \"python/nodes/nodes.py\",
      \"severity\": \"error\"
    }
  ]
}";

#[test]
fn given_project_when_building_dag_json_then_python_artifact_agrees() {
    let test_cases = [
        BuildDagJsonTestCase {
            description: "an empty project keeps its required collections",
            project: DagProjectInput::default(),
            python: None,
            upstream: Vec::new(),
            expected_json: Ok(EMPTY_JSON),
        },
        BuildDagJsonTestCase {
            description: "declared loaders become sources and None and empty fields are dropped",
            project: DagProjectInput {
                sources: vec![source(
                    "orders",
                    Some("load_orders"),
                    team_meta(DagValue::String("ops".to_owned())),
                )],
                loaders: vec![
                    loader("load_orders", None, &[]),
                    loader("load_customers", None, &["load_orders"]),
                ],
                target_schema: Some("main".to_owned()),
                ..DagProjectInput::default()
            },
            python: Some(DagPythonGraphInput {
                nodes: vec![python_node(
                    "check",
                    "orders_check",
                    vec![key("source", "orders")],
                )],
                dependency_edges: Vec::new(),
                kinds_by_name: vec![("orders_check".to_owned(), "check".to_owned())],
            }),
            upstream: Vec::new(),
            expected_json: Ok(LOADED_JSON),
        },
        BuildDagJsonTestCase {
            description: "a loader destination with an empty part is refused",
            project: DagProjectInput {
                loaders: vec![loader("load_orders", Some("raw..orders"), &[])],
                ..DagProjectInput::default()
            },
            python: None,
            upstream: Vec::new(),
            expected_json: Err(DagArtifactError::LoaderDestination(
                "Invalid loader destination 'raw..orders': expected 1 to 3 non-empty \
                 dot-separated parts (table, schema.table, or database.schema.table)"
                    .to_owned(),
            )),
        },
        BuildDagJsonTestCase {
            description: "a meta value json cannot encode raises its TypeError",
            project: DagProjectInput {
                sources: vec![source(
                    "orders",
                    None,
                    team_meta(DagValue::Unencodable(
                        "Object of type set is not JSON serializable".to_owned(),
                    )),
                )],
                ..DagProjectInput::default()
            },
            python: None,
            upstream: Vec::new(),
            expected_json: Err(DagArtifactError::Unencodable(
                "Object of type set is not JSON serializable".to_owned(),
            )),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            built(&test_case),
            test_case.expected_json.map(str::to_owned),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_loader_depending_on_an_asset_when_building_dag_json_then_the_edge_reaches_its_source() {
    let test_cases = [DagEdgeTestCase {
        description: "an asset upstream of a declared loader points at the loader's source",
        project: DagProjectInput {
            sources: vec![source(
                "partner_scores",
                Some("partner_scores"),
                DagValue::Object(Vec::new()),
            )],
            loaders: vec![loader("partner_scores", None, &[])],
            ..DagProjectInput::default()
        },
        python: DagPythonGraphInput {
            nodes: vec![
                python_node("asset", "score_orders", vec![key("model", "orders")]),
                python_node("loader", "partner_scores", Vec::new()),
            ],
            dependency_edges: vec![("score_orders".to_owned(), "partner_scores".to_owned())],
            kinds_by_name: vec![
                ("score_orders".to_owned(), "asset".to_owned()),
                ("partner_scores".to_owned(), "loader".to_owned()),
            ],
        },
        expected_edges: &[
            ("asset:score_orders", "source:partner_scores"),
            ("model:orders", "asset:score_orders"),
        ],
    }];
    for test_case in test_cases {
        assert_eq!(
            dag_edges(&test_case),
            test_case
                .expected_edges
                .iter()
                .map(|(from, to)| ((*from).to_owned(), (*to).to_owned()))
                .collect::<Vec<(String, String)>>(),
            "{}",
            test_case.description
        );
    }
}
