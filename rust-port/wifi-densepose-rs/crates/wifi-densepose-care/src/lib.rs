//! WiFi-DensePose Disability Care Monitor
//!
//! Provides passive, camera-free monitoring for physically disabled people
//! living independently. All sensing happens via existing WiFi signals —
//! no wearables, no cameras, no privacy invasion.
//!
//! # Features
//! - Fall detection with <2s alert latency
//! - Room presence & inactivity monitoring
//! - Vital signs (breathing + heart rate) through walls
//! - Activity & posture tracking (lying/sitting/standing/walking)
//! - Emergency / distress motion detection
//! - Sleep stage monitoring (wake/light/deep/REM)
//! - Caregiver alert dispatch (webhook, MQTT, in-memory)

pub mod alerting;
pub mod api;
pub mod detection;
pub mod domain;

pub use domain::*;
