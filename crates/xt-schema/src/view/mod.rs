//! Read-only resolved B-rep view over generated typed documents.
//!
//! Pointers are resolved through an index of file indices. Geometry is
//! returned as plain values for analytic surfaces and curves and as the
//! generated node for everything else. An unset tolerance is `None` and means
//! exact.

mod common;

pub use common::*;

#[cfg(test)]
mod tests;

/// The view for Onshape/ABCD `SCH_30100` documents.
pub mod sch_30100 {
    use crate::schemas::sch_30100 as schema;
    include!("model.rs");
}

/// The view for `SCH_37102` documents.
pub mod sch_37102 {
    use crate::schemas::sch_37102 as schema;
    include!("model.rs");
}
