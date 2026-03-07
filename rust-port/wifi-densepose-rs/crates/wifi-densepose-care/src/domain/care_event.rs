//! Care alerts — what gets sent to caregivers.

use chrono::{DateTime, Utc};
use uuid::Uuid;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Priority level for a care alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CareAlertPriority {
    /// Informational — no action needed right now.
    Info,
    /// Caregiver should check in when convenient.
    Low,
    /// Caregiver should check in soon.
    Medium,
    /// Urgent — check in immediately.
    High,
    /// Emergency — call for help / dispatch assistance.
    Critical,
}

impl std::fmt::Display for CareAlertPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CareAlertPriority::Info => write!(f, "INFO"),
            CareAlertPriority::Low => write!(f, "LOW"),
            CareAlertPriority::Medium => write!(f, "MEDIUM"),
            CareAlertPriority::High => write!(f, "HIGH"),
            CareAlertPriority::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// The specific type of care alert.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CareAlertKind {
    // ── Fall detection ──────────────────────────────────────────────────────
    /// Person fell. Immediate action required.
    FallDetected,
    /// Person has been on the floor for longer than the configured threshold.
    FallOnFloorExtended { seconds_on_floor: u64 },

    // ── Presence / inactivity ────────────────────────────────────────────────
    /// No presence detected in the home for longer than threshold.
    InactivityAlert { room: String, minutes_inactive: u64 },
    /// Resident has not been seen in expected room at expected time.
    MissedRoutine {
        expected_room: String,
        expected_time: String,
    },

    // ── Vital signs ─────────────────────────────────────────────────────────
    /// Breathing stopped or below 4 BPM — possible apnea.
    PossibleApnea { breathing_bpm: f32 },
    /// Breathing rate is abnormally slow (4–8 BPM).
    BradypneaAlert { breathing_bpm: f32 },
    /// Breathing rate is abnormally fast (> 30 BPM).
    TachypneaAlert { breathing_bpm: f32 },
    /// Heart rate too fast (> 120 BPM).
    TachycardiaAlert { heart_rate_bpm: f32 },
    /// Heart rate too slow (< 45 BPM).
    BradycardiaAlert { heart_rate_bpm: f32 },

    // ── Activity / posture ───────────────────────────────────────────────────
    /// Unusual posture change detected.
    UnusualPosture { description: String },
    /// Person has been in the same posture for an unusually long time.
    ProlongedInactivePosture { posture: String, minutes: u64 },

    // ── Emergency motion ─────────────────────────────────────────────────────
    /// Rapid, erratic motion suggesting distress.
    DistressMotionDetected,
    /// Motion pattern consistent with seizure activity.
    PossibleSeizure,
    /// Motion stopped suddenly after agitated movement.
    SuddenMotionCessation,

    // ── Sleep ────────────────────────────────────────────────────────────────
    /// Possible sleep apnea episode during monitored sleep.
    SleepApneaEpisode { duration_seconds: u32 },
    /// Very restless sleep — may need attention.
    RestlessSleep { restlessness_score: f32 },
    /// No movement or breathing detected during expected sleep — check required.
    SleepMonitoringLost,
}

impl std::fmt::Display for CareAlertKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CareAlertKind::FallDetected => write!(f, "Fall detected"),
            CareAlertKind::FallOnFloorExtended { seconds_on_floor } => {
                write!(f, "On floor for {seconds_on_floor}s")
            }
            CareAlertKind::InactivityAlert {
                room,
                minutes_inactive,
            } => write!(f, "No activity in {room} for {minutes_inactive} min"),
            CareAlertKind::MissedRoutine {
                expected_room,
                expected_time,
            } => write!(f, "Not seen in {expected_room} at {expected_time}"),
            CareAlertKind::PossibleApnea { breathing_bpm } => {
                write!(f, "Possible apnea: {breathing_bpm:.1} BPM")
            }
            CareAlertKind::BradypneaAlert { breathing_bpm } => {
                write!(f, "Slow breathing: {breathing_bpm:.1} BPM")
            }
            CareAlertKind::TachypneaAlert { breathing_bpm } => {
                write!(f, "Fast breathing: {breathing_bpm:.1} BPM")
            }
            CareAlertKind::TachycardiaAlert { heart_rate_bpm } => {
                write!(f, "High heart rate: {heart_rate_bpm:.0} BPM")
            }
            CareAlertKind::BradycardiaAlert { heart_rate_bpm } => {
                write!(f, "Low heart rate: {heart_rate_bpm:.0} BPM")
            }
            CareAlertKind::UnusualPosture { description } => {
                write!(f, "Unusual posture: {description}")
            }
            CareAlertKind::ProlongedInactivePosture { posture, minutes } => {
                write!(f, "{posture} for {minutes} min")
            }
            CareAlertKind::DistressMotionDetected => write!(f, "Distress motion detected"),
            CareAlertKind::PossibleSeizure => write!(f, "Possible seizure activity"),
            CareAlertKind::SuddenMotionCessation => {
                write!(f, "Sudden stop after agitated movement")
            }
            CareAlertKind::SleepApneaEpisode { duration_seconds } => {
                write!(f, "Sleep apnea episode: {duration_seconds}s")
            }
            CareAlertKind::RestlessSleep { restlessness_score } => {
                write!(f, "Restless sleep (score {restlessness_score:.2})")
            }
            CareAlertKind::SleepMonitoringLost => write!(f, "Sleep monitoring signal lost"),
        }
    }
}

/// A care alert to be dispatched to caregivers.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CareAlert {
    /// Unique alert identifier.
    pub id: String,
    /// Resident this alert concerns.
    pub resident_id: String,
    /// Specific kind of alert.
    pub kind: CareAlertKind,
    /// Priority level.
    pub priority: CareAlertPriority,
    /// Human-readable message for caregiver.
    pub message: String,
    /// Room where the event was detected.
    pub room: Option<String>,
    /// When the alert was generated.
    pub timestamp: DateTime<Utc>,
    /// Whether a caregiver has acknowledged this alert.
    pub acknowledged: bool,
}

impl CareAlert {
    /// Create a new unacknowledged alert.
    pub fn new(
        resident_id: impl Into<String>,
        kind: CareAlertKind,
        priority: CareAlertPriority,
        room: Option<String>,
    ) -> Self {
        let message = kind.to_string();
        Self {
            id: Uuid::new_v4().to_string(),
            resident_id: resident_id.into(),
            kind,
            priority,
            message,
            room,
            timestamp: Utc::now(),
            acknowledged: false,
        }
    }

    /// Mark this alert as acknowledged.
    pub fn acknowledge(&mut self) {
        self.acknowledged = true;
    }

    /// Returns true if this is a critical or high priority alert.
    pub fn is_urgent(&self) -> bool {
        matches!(
            self.priority,
            CareAlertPriority::Critical | CareAlertPriority::High
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_creation() {
        let alert = CareAlert::new(
            "resident-1",
            CareAlertKind::FallDetected,
            CareAlertPriority::Critical,
            Some("living-room".into()),
        );
        assert_eq!(alert.resident_id, "resident-1");
        assert!(!alert.acknowledged);
        assert!(alert.is_urgent());
        assert!(!alert.id.is_empty());
    }

    #[test]
    fn alert_acknowledge() {
        let mut alert = CareAlert::new(
            "resident-1",
            CareAlertKind::InactivityAlert {
                room: "bedroom".into(),
                minutes_inactive: 90,
            },
            CareAlertPriority::Medium,
            Some("bedroom".into()),
        );
        assert!(!alert.acknowledged);
        alert.acknowledge();
        assert!(alert.acknowledged);
    }

    #[test]
    fn alert_priority_ordering() {
        assert!(CareAlertPriority::Critical > CareAlertPriority::High);
        assert!(CareAlertPriority::High > CareAlertPriority::Medium);
        assert!(CareAlertPriority::Medium > CareAlertPriority::Low);
        assert!(CareAlertPriority::Low > CareAlertPriority::Info);
    }

    #[test]
    fn apnea_alert_display() {
        let kind = CareAlertKind::PossibleApnea { breathing_bpm: 2.1 };
        assert!(kind.to_string().contains("2.1"));
    }

    #[test]
    fn info_alert_not_urgent() {
        let alert = CareAlert::new(
            "r1",
            CareAlertKind::RestlessSleep {
                restlessness_score: 0.45,
            },
            CareAlertPriority::Info,
            None,
        );
        assert!(!alert.is_urgent());
    }
}
