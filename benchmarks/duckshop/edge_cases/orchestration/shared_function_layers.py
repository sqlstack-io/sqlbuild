"""Layer jobs selected by tag share the function their models call, and still order by lineage.

A tag selection brings in the functions its models call, so both jobs build the shared function. A
job that builds a node itself does not wait for that node's other builders, so the group runs the
marts layer after the staging layer instead of reporting a cycle.
"""

from sqlbuild.orchestration import job, job_group

staging = job("staging_layer", description="Build the staging layer.", select="tag:staging")
marts = job("marts_layer", description="Build the marts layer.", select="tag:marts")
layers = job_group(
    "layered_orders", description="Build staging, then marts.", jobs=[marts, staging]
)
