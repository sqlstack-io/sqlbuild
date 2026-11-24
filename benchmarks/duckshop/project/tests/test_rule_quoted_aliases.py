from rules.quoted_aliases import mixed_case_names_stay_in_exports

from sqlbuild.rules import RuleCase, RuleFile, evaluate_rule

_HEADER = 'MODEL (description "Listings");\n'
_MIXED_CASE = (
    _HEADER
    + "SELECT\n"
    + '  l.listing_id AS "listingId",\n'
    + '  l.note AS "note",\n'
    + "  'isLive' AS status_label\n"
    + 'FROM __ref("dim_listings") AS l\n'
)
_LISTINGS = RuleFile(
    path="sql/models/marts/catalogue/dim_listings.sql",
    source=_HEADER + "SELECT 1 AS listing_id, 'new' AS note\n",
)


def test_given_quoted_mixed_case_alias_outside_an_export_when_evaluating_then_reports_it() -> None:
    result = evaluate_rule(
        rule=mixed_case_names_stay_in_exports,
        test_case=RuleCase(
            description="a mart names a column in a partner's case",
            source=_MIXED_CASE,
            path="sql/models/marts/catalogue/rpt_listings.sql",
            files=(_LISTINGS,),
            expected_finding_count=1,
        ),
    )

    assert result.finding_count == 1
    assert (result.findings[0].line, result.findings[0].column) == (3, 3)
    assert result.findings[0].message == (
        "Quoted mixed-case name 'listingId' in `l.listing_id AS \"listingId\"`"
    )


def test_given_quoted_mixed_case_alias_in_an_export_when_evaluating_then_passes() -> None:
    result = evaluate_rule(
        rule=mixed_case_names_stay_in_exports,
        test_case=RuleCase(
            description="a partner export writes the partner's own names",
            source=_MIXED_CASE,
            path="sql/models/marts/edge_cases/rpt_edge_partner_listing_export.sql",
            files=(_LISTINGS,),
            expected_finding_count=0,
        ),
    )

    assert result.finding_count == 0
