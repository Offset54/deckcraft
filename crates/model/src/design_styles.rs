//! SlideCraft paint, effect and grid styles (Phase 4.2). Filled in by P4.2.

use serde::{Deserialize, Serialize};

/// A named style in the deck's design system.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DesignStyle {
    pub id: u32,
    pub name: String,
}

pub(crate) fn sanitize(styles: &mut Vec<DesignStyle>) {
    styles.truncate(4096);
}
