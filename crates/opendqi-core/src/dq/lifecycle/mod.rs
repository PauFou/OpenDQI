//! EMIR lifecycle (cross-batch) checks. Each impl receives both the
//! current batch and a slice of `prior` records loaded from the
//! history store for the UTIs present in the current batch.

mod duplicate_newt_for_uti;
mod etrm_without_newt;
mod modi_without_newt;
mod valuation_after_termination;
mod valuation_regression;

pub use duplicate_newt_for_uti::DuplicateNewtForUti;
pub use etrm_without_newt::EtrmWithoutNewt;
pub use modi_without_newt::ModiWithoutNewt;
pub use valuation_after_termination::LifecycleValuationAfterTermination;
pub use valuation_regression::ValuationRegression;
