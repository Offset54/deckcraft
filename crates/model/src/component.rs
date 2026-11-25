//! SlideCraft components (Phase 4.3). Filled in by P4.3.

use serde::{Deserialize, Serialize};

/// A main component in the deck's library.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Component {
    pub id: u32,
    pub name: String,
}

pub(crate) fn sanitize(c: &mut Vec<Component>) {
    c.truncate(4096);
}

pub(crate) fn max_shape_id(_c: &Component) -> u32 {
    0
}
