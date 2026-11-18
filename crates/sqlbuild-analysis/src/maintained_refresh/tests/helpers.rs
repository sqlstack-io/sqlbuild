use crate::maintained_refresh::models::{
    LiveDynamicTable, MaintainedRead, MaintainedSettings, RefreshLag,
};

pub(super) fn settings(lag: RefreshLag, warehouse: Option<&str>) -> MaintainedSettings {
    MaintainedSettings {
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

pub(super) fn read(name: &str, maintained: bool, replaced: bool, reads: &[&str]) -> MaintainedRead {
    MaintainedRead {
        name: name.to_owned(),
        maintained,
        replaced,
        reads: reads.iter().map(|read| (*read).to_owned()).collect(),
    }
}
