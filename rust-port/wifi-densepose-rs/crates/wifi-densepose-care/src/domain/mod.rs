//! Domain types for disability care monitoring.

pub mod care_event;
pub mod person_state;
pub mod room;

pub use care_event::{CareAlert, CareAlertKind, CareAlertPriority};
pub use person_state::{
    ActivityLevel, PostureKind, ResidentState, SleepStage, VitalSnapshot,
};
pub use room::RoomPresence;
