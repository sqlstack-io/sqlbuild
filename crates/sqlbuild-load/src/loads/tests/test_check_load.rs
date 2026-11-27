use crate::loads::tests::helpers::checked;
use crate::loads::tests::test_types::CheckLoadTestCase;

#[test]
fn given_load_settings_when_checking_them_then_sqlload_reads_them_with_the_listed_sources_columns()
{
    let test_cases = [
        CheckLoadTestCase {
            description: "a listed source's on_schema_change goes to its table",
            settings: "kind = \"postgres\"\nconnection = \"shop_db\"\ninclude = [\"public.orders\"]\n",
            sources: &[("shop__orders", &[])],
            on_schema_change: Some("fail"),
            expected_result: Ok("{ \"public.orders\" = { on_schema_change = \"fail\" } }"),
        },
        CheckLoadTestCase {
            description: "the load's table can't set on_schema_change its source sets",
            settings: "kind = \"postgres\"\nconnection = \"shop_db\"\ninclude = [\"public.orders\"]\n\n[tables.\"public.orders\"]\non_schema_change = \"ignore\"\n",
            sources: &[("shop__orders", &[])],
            on_schema_change: Some("fail"),
            expected_result: Err((
                "source 'shop__orders' sets on_schema_change, so the load's table can't; keep it on the source",
                &["tables", "public.orders", "on_schema_change"],
            )),
        },
        CheckLoadTestCase {
            description: "a literal include hands a listed source's column types to its table",
            settings: "kind = \"postgres\"\nconnection = \"shop_db\"\ninclude = [\"public.orders\"]\n",
            sources: &[("shop__orders", &[("id", "INTEGER")])],
            on_schema_change: None,
            expected_result: Ok("{ \"public.orders\" = { columns = { id = \"INTEGER\" } } }"),
        },
        CheckLoadTestCase {
            description: "a pattern leaves the listed sources to the plan",
            settings: "kind = \"postgres\"\nconnection = \"shop_db\"\ninclude = [\"public.*\"]\n",
            sources: &[("shop__anything", &[("id", "INTEGER")])],
            on_schema_change: None,
            expected_result: Ok(""),
        },
        CheckLoadTestCase {
            description: "a listed source no literal include loads into",
            settings: "kind = \"postgres\"\nconnection = \"shop_db\"\ninclude = [\"public.orders\"]\n",
            sources: &[("shop__carts", &[])],
            on_schema_change: None,
            expected_result: Err((
                "source 'shop__carts' matches no table the load 'shop' includes; its tables load \
                 into shop__orders",
                &["include"],
            )),
        },
        CheckLoadTestCase {
            description: "a one-table files load hands its source's column types to its table",
            settings: "kind = \"files\"\nconnection = \"landing\"\npath = \"orders/\"\nformat = \"csv\"\n\
                       table_name = \"shop__orders\"\n",
            sources: &[("shop__orders", &[("id", "BIGINT"), ("big", "UBIGINT")])],
            on_schema_change: None,
            expected_result: Ok(
                "{ shop = { columns = { big = 'native(\"UBIGINT\")', id = \"BIGINT\" } } }",
            ),
        },
        CheckLoadTestCase {
            description: "a one-table files load leaves a source it doesn't fill to the plan",
            settings: "kind = \"files\"\nconnection = \"landing\"\npath = \"orders/\"\nformat = \"csv\"\n\
                       table_name = \"shop__orders\"\n",
            sources: &[("shop__refunds", &[("id", "BIGINT")])],
            on_schema_change: None,
            expected_result: Ok(""),
        },
        CheckLoadTestCase {
            description: "a load can't read a warehouse connection",
            settings: "kind = \"postgres\"\nconnection = \"warehouse\"\ninclude = [\"public.orders\"]\n",
            sources: &[],
            on_schema_change: None,
            expected_result: Err((
                "connection 'warehouse' sets no kind; loads and loaders read source connections",
                &["connection"],
            )),
        },
        CheckLoadTestCase {
            description: "column types belong to the sources",
            settings: "kind = \"postgres\"\nconnection = \"shop_db\"\ninclude = [\"public.orders\"]\n\
                       [tables.\"public.orders\"]\ncolumns = { id = \"INTEGER\" }\n",
            sources: &[],
            on_schema_change: None,
            expected_result: Err((
                "a load's tables can't set columns; declare each column's type on the source \
                 the table loads into, under `sources:`",
                &["tables", "public.orders", "columns"],
            )),
        },
        CheckLoadTestCase {
            description: "an unknown kind gets SQLLoad's message, its path under load",
            settings: "kind = \"postgress\"\nconnection = \"shop_db\"\n",
            sources: &[],
            on_schema_change: None,
            expected_result: Err((
                "unknown variant `postgress`, expected one of `postgres`, `postgres_cdc`, \
                 `files`, `snowflake`, `odbc`, `bigquery`, `databricks`, `duckdb`, `motherduck`, \
                 `python`, `google_sheets`\nin `load.kind`",
                &["kind"],
            )),
        },
        CheckLoadTestCase {
            description: "an unknown setting gets SQLLoad's message, named under load",
            settings: "kind = \"postgres\"\nconnection = \"shop_db\"\ninclude = [\"public.orders\"]\n\
                       [tables.\"public.orders\"]\nwrite_strategi = \"merge\"\n",
            sources: &[],
            on_schema_change: None,
            expected_result: Err((
                "unknown key \"write_strategi\" in [load.tables.\"public.orders\"]; did you mean \
                 \"write_strategy\"?",
                &["tables", "public.orders", "write_strategi"],
            )),
        },
    ];
    for test_case in test_cases {
        let result = checked(
            test_case.settings,
            (test_case.sources, test_case.on_schema_change),
        );
        assert_eq!(
            result
                .as_ref()
                .map(String::as_str)
                .map_err(|(message, path)| (message.as_str(), path.clone())),
            test_case.expected_result.map_err(|(message, path)| (
                message,
                path.iter().map(|segment| (*segment).to_owned()).collect()
            )),
            "{}",
            test_case.description
        );
    }
}
