//! Resident state — posture, activity, vitals, sleep.

use chrono::{DateTime, Utc};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Body posture inferred from 17-keypoint skeleton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PostureKind {
    /// Person is lying flat — bed/floor.
    Lying,
    /// Person is seated — chair, wheelchair, floor.
    Sitting,
    /// Person is upright and still.
    Standing,
    /// Person is moving through the space.
    Walking,
    /// Cannot be determined (low confidence or no signal).
    Unknown,
}

impl std::fmt::Display for PostureKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PostureKind::Lying => write!(f, "lying"),
            PostureKind::Sitting => write!(f, "sitting"),
            PostureKind::Standing => write!(f, "standing"),
            PostureKind::Walking => write!(f, "walking"),
            PostureKind::Unknown => write!(f, "unknown"),
        }
    }
}

/// Coarse activity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ActivityLevel {
    /// No detectable motion — possibly asleep or very still.
    Stationary,
    /// Micro-movements only (breathing, tremor).
    Minimal,
    /// Normal low-level activity (shifting, reaching).
    Low,
    /// Active movement around the space.
    Active,
    /// Rapid, erratic, or high-energy movement.
    Agitated,
}

/// Sleep stage classification from breathing + movement patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SleepStage {
    /// Awake and moving.
    Awake,
    /// Light sleep — NREM1/2.
    Light,
    /// Deep slow-wave sleep — NREM3.
    Deep,
    /// Rapid eye movement — REM.
    Rem,
    /// Not enough data yet.
    Unknown,
}

impl std::fmt::Display for SleepStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SleepStage::Awake => write!(f, "awake"),
            SleepStage::Light => write!(f, "light"),
            SleepStage::Deep => write!(f, "deep"),
            SleepStage::Rem => write!(f, "REM"),
            SleepStage::Unknown => write!(f, "unknown"),
        }
    }
}

/// Vital sign snapshot at one moment.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct VitalSnapshot {
    /// Respiratory rate in breaths-per-minute. None = not detected.
    pub breathing_bpm: Option<f32>,
    /// Heart rate in beats-per-minute. None = not detected.
    pub heart_rate_bpm: Option<f32>,
    /// Signal quality [0.0, 1.0].
    pub signal_quality: f32,
    /// Timestamp.
    pub timestamp: DateTime<Utc>,
}

impl VitalSnapshot {
    /// Create a new snapshot with current timestamp.
    pub fn new(
        breathing_bpm: Option<f32>,
        heart_rate_bpm: Option<f32>,
        signal_quality: f32,
    ) -> Self {
        Self {
            breathing_bpm,
            heart_rate_bpm,
            signal_quality: signal_quality.clamp(0.0, 1.0),
            timestamp: Utc::now(),
        }
    }

    /// True if breathing is in normal adult range (12–20 BPM).
    pub fn breathing_normal(&self) -> bool {
        self.breathing_bpm
            .map(|r| r >= 12.0 && r <= 20.0)
            .unwrap_or(false)
    }

    /// True if heart rate is in normal resting range (50–100 BPM).
    pub fn heart_rate_normal(&self) -> bool {
        self.heart_rate_bpm
            .map(|r| r >= 50.0 && r <= 100.0)
            .unwrap_or(false)
    }

    /// True if breathing suggests apnea (< 4 BPM or absent).
    pub fn possible_apnea(&self) -> bool {
        match self.breathing_bpm {
            None => true,
            Some(r) => r < 4.0,
        }
    }
}

/// Full snapshot of a resident's state at one moment in time.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ResidentState {
    /// Unique resident identifier (configured by caregiver).
    pub resident_id: String,
    /// Room the resident is currently in.
    pub room: String,
    /// Current body posture.
    pub posture: PostureKind,
    /// Current activity level.
    pub activity: ActivityLevel,
    /// Vital signs.
    pub vitals: VitalSnapshot,
    /// Sleep stage (relevant when posture is Lying and activity is Stationary/Minimal).
    pub sleep_stage: SleepStage,
    /// Confidence in this overall reading [0.0, 1.0].
    pub confidence: f32,
    /// When this state was observed.
    pub timestamp: DateTime<Utc>,
}

impl ResidentState {
    /// Create a new resident state.
    pub fn new(
        resident_id: impl Into<String>,
        room: impl Into<String>,
        posture: PostureKind,
        activity: ActivityLevel,
        vitals: VitalSnapshot,
        sleep_stage: SleepStage,
        confidence: f32,
    ) -> Self {
        Self {
            resident_id: resident_id.into(),
            room: room.into(),
            posture,
            activity,
            vitals,
            sleep_stage,
            confidence: confidence.clamp(0.0, 1.0),
            timestamp: Utc::now(),
        }
    }

    /// True when the resident appears to be asleep.
    pub fn is_sleeping(&self) -> bool {
        matches!(self.posture, PostureKind::Lying)
            && matches!(
                self.activity,
                ActivityLevel::Stationary | ActivityLevel::Minimal
            )
            && matches!(
                self.sleep_stage,
                SleepStage::Light | SleepStage::Deep | SleepStage::Rem
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vital_snapshot_breathing_normal() {
        let v = VitalSnapshot::new(Some(16.0), Some(72.0), 0.9);
        assert!(v.breathing_normal());
        assert!(v.heart_rate_normal());
        assert!(!v.possible_apnea());
    }

    #[test]
    fn vital_snapshot_apnea() {
        let v = VitalSnapshot::new(Some(2.0), Some(72.0), 0.7);
        assert!(!v.breathing_normal());
        assert!(v.possible_apnea());
    }

    #[test]
    fn vital_snapshot_no_signal() {
        let v = VitalSnapshot::new(None, None, 0.1);
        assert!(!v.breathing_normal());
        assert!(v.possible_apnea()); // absent = possible apnea
    }

    #[test]
    fn resident_state_sleeping() {
        let vitals = VitalSnapshot::new(Some(14.0), Some(58.0), 0.85);
        let state = ResidentState::new(
            "resident-1",
            "bedroom",
            PostureKind::Lying,
            ActivityLevel::Stationary,
            vitals,
            SleepStage::Deep,
            0.9,
        );
        assert!(state.is_sleeping());
    }

    #[test]
    fn posture_display() {
        assert_eq!(PostureKind::Walking.to_string(), "walking");
        assert_eq!(PostureKind::Lying.to_string(), "lying");
    }
}
