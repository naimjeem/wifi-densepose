//! Detection engines for disability care monitoring.

pub mod activity;
pub mod care_monitor;
pub mod emergency;
pub mod fall;
pub mod sleep;
pub mod vitals;

pub use care_monitor::{CareMonitor, CareMonitorConfig, SensingFrame};
