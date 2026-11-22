//! A location runtime's live discovery for the publication it is asked for.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{PyAnyMethods, PyResult};
use sqlbuild_orchestration::discovered_sets::models::DiscoveryPublishRequest;
use sqlbuild_python_base::boundary::models::KeywordArguments;

use crate::discovered_sets::main::set_discovery_source::set_discovery_source;
use crate::discovered_sets::models::{DiscoverySource, LiveDiscovery, Rediscover, SeedSource};

/// Discover live for the publication `request` asks for: its functions called again, the rest reused.
pub fn discover_for_publication(
    arguments: &KeywordArguments<'_>,
    request: DiscoveryPublishRequest,
) -> PyResult<()> {
    let target: String = arguments.value("selected_target")?.extract()?;
    let location_id = arguments
        .value("location_id")?
        .extract::<String>()?
        .parse()
        .map_err(|_| PyValueError::new_err("location ID must be a UUID"))?;
    let seed: Option<SeedSource> = if request.seed {
        Some(SeedSource {
            server_url: arguments.value("server_url")?.extract()?,
            location_id,
            target: target.clone(),
        })
    } else {
        None
    };
    set_discovery_source(DiscoverySource::Live(LiveDiscovery {
        target: Some(target),
        previous: request.previous,
        rediscover: request.rediscover.map_or(Rediscover::All, Rediscover::Only),
        keep_last_good: true,
        seed,
    }));
    Ok(())
}
