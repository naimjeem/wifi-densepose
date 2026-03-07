//! Emergency motion detection — distress, seizure, sudden cessation.

use chrono::{DateTime, Utc};

use crate::domain::{CareAlert, CareAlertKind, CareAlertPriority};

/// Configuration for emergency motion detection.
#[derive(Debug, Clone)]
pub struct EmergencyDetectorConfig {
    /// Motion score threshold for "agitated" movement.
    pub agitation_threshold: f32,
    /// Number of consecutive agitated frames to classify as distress.
    pub distress_frame_count: usize,
    /// Motion variance threshold for seizure detection.
    /// Seizures produce oscillatory high-frequency motion.
    pub seizure_variance_threshold: f32,
    /// Motion score below this = "ceased" after agitation.
    pub cessation_threshold: f32,
    /// Seconds of agitation before checking for cessation.
    pub cessation_lookback_secs: u64,
}

impl Default for EmergencyDetectorConfig {
    fn default() -> Self {
        Self {
            agitation_threshold: 0.70,
            distress_frame_count: 8,
            seizure_variance_threshold: 0.25,
            cessation_threshold: 0.10,
            cessation_lookback_secs: 30,
        }
    }
}

/// Emergency motion detector.
pub struct EmergencyDetector {
    config: EmergencyDetectorConfig,
    /// Recent motion scores (rolling window).
    motion_history: Vec<f32>,
    /// Timestamp of when agitation started.
    agitation_start: Option<DateTime<Utc>>,
    /// Whether a distress alert has been issued this episode.
    distress_alerted: bool,
    /// Whether a seizure alert has been issued this episode.
    seizure_alerted: bool,
}

impl EmergencyDetector {
    pub fn new(config: EmergencyDetectorConfig) -> Self {
        Self {
            config,
            motion_history: Vec::with_capacity(64),
            agitation_start: None,
            distress_alerted: false,
            seizure_alerted: false,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(EmergencyDetectorConfig::default())
    }

    /// Feed a motion score. Returns any emergency care alerts.
    pub fn process(
        &mut self,
        motion_score: f32,
        timestamp: DateTime<Utc>,
        resident_id: &str,
        room: Option<String>,
    ) -> Vec<CareAlert> {
        let mut alerts = Vec::new();
        let motion = motion_score.clamp(0.0, 1.0);

        // Maintain rolling window (last 64 frames)
        self.motion_history.push(motion);
        if self.motion_history.len() > 64 {
            self.motion_history.remove(0);
        }

        let is_agitated = motion >= self.config.agitation_threshold;

        // Track agitation start
        if is_agitated {
            if self.agitation_start.is_none() {
                self.agitation_start = Some(timestamp);
            }
        } else {
            // Check for sudden cessation after prolonged agitation
            if let Some(start) = self.agitation_start {
                let agitation_secs = (timestamp - start).num_seconds() as u64;
                if agitation_secs >= self.config.cessation_lookback_secs
                    && motion <= self.config.cessation_threshold
                {
                    alerts.push(CareAlert::new(
                        resident_id,
                        CareAlertKind::SuddenMotionCessation,
                        CareAlertPriority::High,
                        room.clone(),
                    ));
                }
            }
            self.agitation_start = None;
            self.distress_alerted = false;
            self.seizure_alerted = false;
        }

        // Count consecutive high-motion frames
        let consecutive_agitated = self
            .motion_history
            .iter()
            .rev()
            .take_while(|&&s| s >= self.config.agitation_threshold)
            .count();

        // ── Distress detection ───────────────────────────────────────────
        if consecutive_agitated >= self.config.distress_frame_count && !self.distress_alerted {
            self.distress_alerted = true;
            alerts.push(CareAlert::new(
                resident_id,
                CareAlertKind::DistressMotionDetected,
                CareAlertPriority::High,
                room.clone(),
            ));
        }

        // ── Seizure detection ────────────────────────────────────────────
        // Seizures = high variance in motion within window (oscillatory)
        if self.motion_history.len() >= 16 && !self.seizure_alerted {
            let window = &self.motion_history[self.motion_history.len() - 16..];
            let variance = Self::variance(window);
            if variance >= self.config.seizure_variance_threshold && consecutive_agitated >= 4 {
                self.seizure_alerted = true;
                alerts.push(CareAlert::new(
                    resident_id,
                    CareAlertKind::PossibleSeizure,
                    CareAlertPriority::Critical,
                    room.clone(),
                ));
            }
        }

        alerts
    }

    fn variance(data: &[f32]) -> f32 {
        if data.is_empty() {
            return 0.0;
        }
        let mean = data.iter().sum::<f32>() / data.len() as f32;
        let sq_diff_sum: f32 = data.iter().map(|&x| (x - mean) * (x - mean)).sum();
        sq_diff_sum / data.len() as f32
    }

    pub fn reset(&mut self) {
        self.motion_history.clear();
        self.agitation_start = None;
        self.distress_alerted = false;
        self.seizure_alerted = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_alert_on_calm_motion() {
        let mut det = EmergencyDetector::with_defaults();
        for _ in 0..20 {
            let alerts = det.process(0.3, Utc::now(), "r1", Some("living-room".into()));
            assert!(alerts.is_empty());
        }
    }

    #[test]
    fn distress_alert_after_sustained_high_motion() {
        let mut det = EmergencyDetector::new(EmergencyDetectorConfig {
            distress_frame_count: 5,
            ..Default::default()
        });

        let mut all_alerts = Vec::new();
        for _ in 0..10 {
            let mut a = det.process(0.85, Utc::now(), "r1", None);
            all_alerts.append(&mut a);
        }

        assert!(all_alerts
            .iter()
            .any(|a| a.kind == CareAlertKind::DistressMotionDetected));
    }

    #[test]
    fn distress_alert_not_repeated() {
        let mut det = EmergencyDetector::new(EmergencyDetectorConfig {
            distress_frame_count: 3,
            ..Default::default()
        });
        let mut distress_count = 0;
        for _ in 0..20 {
            for a in det.process(0.9, Utc::now(), "r1", None) {
                if a.kind == CareAlertKind::DistressMotionDetected {
                    distress_count += 1;
                }
            }
        }
        assert_eq!(distress_count, 1); // only once per episode
    }

    #[test]
    fn variance_calculation() {
        let data = vec![0.0_f32, 1.0, 0.0, 1.0]; // max oscillation
        let v = EmergencyDetector::variance(&data);
        assert!(v > 0.2);

        let flat = vec![0.5_f32; 4]; // constant
        let v2 = EmergencyDetector::variance(&flat);
        assert!(v2 < 0.001);
    }
}
