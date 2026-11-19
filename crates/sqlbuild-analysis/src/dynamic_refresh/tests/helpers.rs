use crate::dynamic_refresh::models::{DynamicRead, DynamicSettings, LiveDynamicTable, RefreshLag};

pub(super) fn settings(lag: RefreshLag, warehouse: Option<&str>) -> DynamicSettings {
    DynamicSettings {
        lag,
        warehouse: warehouse.map(str::to_owned),
    }
}

pub(super) fn live(target_lag: &str, warehouse: &str) -> LiveDynamicTable {
    LiveDynamicTable {
        target_lag: target_lag.to_owned(),
        warehouse: warehouse.to_owned(),
    }
}

pub(super) fn read(name: &str, dynamic: bool, replaced: bool, reads: &[&str]) -> DynamicRead {
    DynamicRead {
        name: name.to_owned(),
        dynamic,
        replaced,
        reads: reads.iter().map(|read| (*read).to_owned()).collect(),
    }
}
