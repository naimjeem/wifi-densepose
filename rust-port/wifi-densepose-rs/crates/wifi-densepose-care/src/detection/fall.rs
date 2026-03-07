//! Fall detection from CSI motion features.
//!
//! Algorithm:
//! 1. Detect a rapid spike in motion score (impact phase).
//! 2. Confirm a sudden drop to a low-altitude posture (post-fall stillness).
//! 3. Require both conditions within a short time window to avoid false positives.
//! 4. Track how long the person remains on the floor for extended-fall alerts.

use chrono::{DateTime, Utc};

use crate::domain::{CareAlert, CareAlertKind, CareAlertPriority};

/// Configuration for fall detection.
#[derive(Debug, Clone)]
pub struct FallDetectorConfig {
    /// Motion spike threshold to detect impact [0.0, 1.0].
    pub impact_motion_threshold: f32,
    /// Maximum motion score after fall (stillness confirmation).
    pub post_fall_motion_threshold: f32,
    /// Seconds after impact to look for post-fall stillness.
    pub confirmation_window_secs: u64,
    /// Minutes on floor before issuing an extended-fall alert.
    pub extended_fall_alert_minutes: u64,
    /// Minimum confidence to act on.
    pub min_confidence: f32,
}

impl Default for FallDetectorConfig {
    fn default() -> Self {
        Self {
            impact_motion_threshold: 0.75,
            post_fall_motion_threshold: 0.15,
            confirmation_window_secs: 5,
            extended_fall_alert_minutes: 2,
            min_confidence: 0.6,
        }
    }
}

/// Internal fall state machine.
#[derive(Debug, Clone, PartialEq)]
enum FallState {
    Normal,
    /// Impact detected — waiting for post-fall stillness confirmation.
    PossibleFall {
        detected_at: DateTime<Utc>,
    },
    /// Fall confirmed — tracking time on floor.
    FallConfirmed {
        confirmed_at: DateTime<Utc>,
    },
}

/// A motion frame fed to the fall detector.
#[derive(Debug, Clone)]
pub struct MotionFrame {
    /// Normalised motion score [0.0, 1.0].
    pub motion_score: f32,
    /// Estimated body height above floor [0.0 = floor, 1.0 = standing].
    /// Derived from keypoint vertical position if pose is available.
    pub height_norm: f32,
    /// Detection confidence [0.0, 1.0].
    pub confidence: f32,
    /// Frame timestamp.
    pub timestamp: DateTime<Utc>,
}

impl MotionFrame {
    pub fn new(motion_score: f32, height_norm: f32, confidence: f32) -> Self {
        Self {
            motion_score: motion_score.clamp(0.0, 1.0),
            height_norm: height_norm.clamp(0.0, 1.0),
            confidence: confidence.clamp(0.0, 1.0),
            timestamp: Utc::now(),
        }
    }
}

/// Fall detector with two-phase confirmation and floor-time tracking.
pub struct FallDetector {
    config: FallDetectorConfig,
    state: FallState,
    extended_alert_issued: bool,
}

impl FallDetector {
    pub fn new(config: FallDetectorConfig) -> Self {
        Self {
            config,
            state: FallState::Normal,
            extended_alert_issued: false,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(FallDetectorConfig::default())
    }

    /// Feed a motion frame. Returns any care alerts generated.
    pub fn process(
        &mut self,
        frame: &MotionFrame,
        resident_id: &str,
        room: Option<String>,
    ) -> Vec<CareAlert> {
        let mut alerts = Vec::new();

        if frame.confidence < self.config.min_confidence {
            return alerts;
        }

        match &self.state.clone() {
            FallState::Normal => {
                // Phase 1: look for impact spike + low height
                if frame.motion_score >= self.config.impact_motion_threshold
                    && frame.height_norm < 0.3
                {
                    self.state = FallState::PossibleFall {
                        detected_at: frame.timestamp,
                    };
                }
            }

            FallState::PossibleFall { detected_at } => {
                let elapsed = (frame.timestamp - *detected_at).num_seconds() as u64;

                if elapsed > self.config.confirmation_window_secs {
                    // Window expired — reset without confirming
                    self.state = FallState::Normal;
                    return alerts;
                }

                // Phase 2: confirm via post-fall stillness + low height
                if frame.motion_score <= self.config.post_fall_motion_threshold
                    && frame.height_norm < 0.25
                {
                    self.state = FallState::FallConfirmed {
                        confirmed_at: frame.timestamp,
                    };
                    self.extended_alert_issued = false;

                    tracing::warn!(
                        resident = resident_id,
                        room = ?room,
                        "Fall confirmed"
                    );

                    alerts.push(CareAlert::new(
                        resident_id,
                        CareAlertKind::FallDetected,
                        CareAlertPriority::Critical,
                        room.clone(),
                    ));
                }
            }

            FallState::FallConfirmed { confirmed_at } => {
                let seconds_on_floor = (frame.timestamp - *confirmed_at).num_seconds() as u64;

                // If person gets up — return to normal
                if frame.height_norm > 0.5 && frame.motion_score > 0.2 {
                    tracing::info!(
                        resident = resident_id,
                        seconds_on_floor,
                        "Resident recovered from fall"
                    );
                    self.state = FallState::Normal;
                    self.extended_alert_issued = false;
                    return alerts;
                }

                // Extended floor time alert
                let threshold_secs = self.config.extended_fall_alert_minutes * 60;
                if seconds_on_floor >= threshold_secs && !self.extended_alert_issued {
                    self.extended_alert_issued = true;
                    alerts.push(CareAlert::new(
                        resident_id,
                        CareAlertKind::FallOnFloorExtended { seconds_on_floor },
                        CareAlertPriority::Critical,
                        room.clone(),
                    ));
                }
            }
        }

        alerts
    }

    /// True if a fall is currently confirmed and the person is still on the floor.
    pub fn fall_active(&self) -> bool {
        matches!(self.state, FallState::FallConfirmed { .. })
    }

    /// Reset the detector state (e.g., when sensor loses signal).
    pub fn reset(&mut self) {
        self.state = FallState::Normal;
        self.extended_alert_issued = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(motion: f32, height: f32) -> MotionFrame {
        MotionFrame::new(motion, height, 0.9)
    }

    #[test]
    fn no_alert_on_normal_motion() {
        let mut det = FallDetector::with_defaults();
        let alerts = det.process(&frame(0.3, 0.9), "r1", Some("living-room".into()));
        assert!(alerts.is_empty());
        assert!(!det.fall_active());
    }

    #[test]
    fn fall_detected_two_phase() {
        let mut det = FallDetector::with_defaults();

        // Phase 1: impact
        let alerts = det.process(&frame(0.9, 0.2), "r1", Some("bedroom".into()));
        assert!(alerts.is_empty()); // Not confirmed yet

        // Phase 2: stillness on floor
        let alerts = det.process(&frame(0.05, 0.1), "r1", Some("bedroom".into()));
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].kind, CareAlertKind::FallDetected);
        assert_eq!(alerts[0].priority, CareAlertPriority::Critical);
        assert!(det.fall_active());
    }

    #[test]
    fn high_motion_alone_does_not_trigger() {
        let mut det = FallDetector::with_defaults();
        // High motion but still standing
        let alerts = det.process(&frame(0.9, 0.9), "r1", None);
        assert!(alerts.is_empty());
    }

    #[test]
    fn recovery_resets_fall_state() {
        let mut det = FallDetector::with_defaults();
        det.process(&frame(0.9, 0.2), "r1", None);
        det.process(&frame(0.05, 0.1), "r1", None); // confirm fall
        assert!(det.fall_active());

        // Person gets up
        det.process(&frame(0.4, 0.85), "r1", None);
        assert!(!det.fall_active());
    }

    #[test]
    fn low_confidence_ignored() {
        let mut det = FallDetector::with_defaults();
        let f = MotionFrame::new(0.9, 0.1, 0.3); // below min_confidence
        let alerts = det.process(&f, "r1", None);
        assert!(alerts.is_empty());
    }
}
