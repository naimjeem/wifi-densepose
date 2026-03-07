//! Sleep stage classification and sleep-related alerts.
//!
//! Stage classification is based on:
//! - Breathing rate: Deep sleep → slower (10–14), REM → slightly irregular
//! - Motion score: Deep sleep → near-zero; REM → micro-movements; Awake → high
//! - Heart rate: Deep sleep → lower; REM → variable; Awake → higher

use crate::domain::{CareAlert, CareAlertKind, CareAlertPriority, SleepStage, VitalSnapshot};

/// Configuration for sleep monitoring.
#[derive(Debug, Clone)]
pub struct SleepMonitorConfig {
    /// Motion score below this = "asleep" condition.
    pub sleep_motion_threshold: f32,
    /// Seconds without breathing signal to trigger sleep apnea alert.
    pub apnea_duration_threshold_secs: u32,
    /// Rolling restlessness score above this triggers alert.
    pub restlessness_alert_threshold: f32,
    /// Number of consecutive frames to confirm stage change.
    pub stage_confirm_frames: usize,
}

impl Default for SleepMonitorConfig {
    fn default() -> Self {
        Self {
            sleep_motion_threshold: 0.15,
            apnea_duration_threshold_secs: 15,
            restlessness_alert_threshold: 0.65,
            stage_confirm_frames: 10,
        }
    }
}

/// Sleep monitor — classifies sleep stage and raises alerts.
pub struct SleepMonitor {
    config: SleepMonitorConfig,
    /// Rolling motion scores.
    motion_window: Vec<f32>,
    /// How many frames breathing has been absent.
    apnea_frames: u32,
    /// Frames per second of the sensor stream.
    fps: f32,
    apnea_alerted: bool,
    restlessness_alerted: bool,
}

impl SleepMonitor {
    pub fn new(config: SleepMonitorConfig, fps: f32) -> Self {
        Self {
            config,
            motion_window: Vec::with_capacity(60),
            apnea_frames: 0,
            fps,
            apnea_alerted: false,
            restlessness_alerted: false,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(SleepMonitorConfig::default(), 10.0)
    }

    /// Feed a sensing frame during sleep. Returns care alerts and current sleep stage.
    pub fn process(
        &mut self,
        motion_score: f32,
        vitals: &VitalSnapshot,
        resident_id: &str,
        room: Option<String>,
    ) -> (SleepStage, Vec<CareAlert>) {
        let mut alerts = Vec::new();
        let motion = motion_score.clamp(0.0, 1.0);

        // Update motion window (30-frame rolling window)
        self.motion_window.push(motion);
        if self.motion_window.len() > 30 {
            self.motion_window.remove(0);
        }

        // ── Apnea tracking ────────────────────────────────────────────────
        if vitals.possible_apnea() {
            self.apnea_frames += 1;
        } else {
            self.apnea_frames = 0;
            self.apnea_alerted = false;
        }

        let apnea_secs = (self.apnea_frames as f32 / self.fps) as u32;
        if apnea_secs >= self.config.apnea_duration_threshold_secs && !self.apnea_alerted {
            self.apnea_alerted = true;
            alerts.push(CareAlert::new(
                resident_id,
                CareAlertKind::SleepApneaEpisode {
                    duration_seconds: apnea_secs,
                },
                CareAlertPriority::Critical,
                room.clone(),
            ));
        }

        // ── Restlessness ──────────────────────────────────────────────────
        let restlessness = self.restlessness_score();
        if restlessness >= self.config.restlessness_alert_threshold && !self.restlessness_alerted {
            self.restlessness_alerted = true;
            alerts.push(CareAlert::new(
                resident_id,
                CareAlertKind::RestlessSleep {
                    restlessness_score: restlessness,
                },
                CareAlertPriority::Low,
                room.clone(),
            ));
        } else if restlessness < self.config.restlessness_alert_threshold * 0.5 {
            self.restlessness_alerted = false;
        }

        let stage = self.classify_stage(motion, vitals);
        (stage, alerts)
    }

    /// Classify sleep stage from current motion and vitals.
    fn classify_stage(&self, motion: f32, vitals: &VitalSnapshot) -> SleepStage {
        if motion > self.config.sleep_motion_threshold * 3.0 {
            return SleepStage::Awake;
        }

        let rr = vitals.breathing_bpm.unwrap_or(0.0);

        if motion < self.config.sleep_motion_threshold {
            if rr > 0.0 && rr < 14.0 {
                SleepStage::Deep
            } else if rr >= 14.0 {
                SleepStage::Light
            } else {
                SleepStage::Unknown
            }
        } else {
            // Some micro-movement — likely light or REM
            let restlessness = self.restlessness_score();
            if restlessness > 0.3 {
                SleepStage::Rem // REM produces micro-movements + irregular breathing
            } else {
                SleepStage::Light
            }
        }
    }

    /// Compute a restlessness score [0.0, 1.0] from motion variance.
    pub fn restlessness_score(&self) -> f32 {
        if self.motion_window.is_empty() {
            return 0.0;
        }
        let mean = self.motion_window.iter().sum::<f32>() / self.motion_window.len() as f32;
        let variance = self
            .motion_window
            .iter()
            .map(|&x| (x - mean) * (x - mean))
            .sum::<f32>()
            / self.motion_window.len() as f32;
        // Normalise: variance of 0.25 (max oscillation 0↔1) maps to 1.0
        (variance / 0.25).min(1.0)
    }

    pub fn reset(&mut self) {
        self.motion_window.clear();
        self.apnea_frames = 0;
        self.apnea_alerted = false;
        self.restlessness_alerted = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet_vitals() -> VitalSnapshot {
        VitalSnapshot::new(Some(13.0), Some(58.0), 0.9)
    }

    fn no_breathing_vitals() -> VitalSnapshot {
        VitalSnapshot::new(Some(2.0), Some(55.0), 0.8)
    }

    #[test]
    fn deep_sleep_classification() {
        let mut mon = SleepMonitor::with_defaults();
        let (stage, _) = mon.process(0.02, &quiet_vitals(), "r1", None);
        assert_eq!(stage, SleepStage::Deep);
    }

    #[test]
    fn awake_when_high_motion() {
        let mut mon = SleepMonitor::with_defaults();
        let vitals = VitalSnapshot::new(Some(18.0), Some(80.0), 0.9);
        let (stage, _) = mon.process(0.8, &vitals, "r1", None);
        assert_eq!(stage, SleepStage::Awake);
    }

    #[test]
    fn sleep_apnea_alert_after_threshold() {
        let mut mon = SleepMonitor::new(
            SleepMonitorConfig {
                apnea_duration_threshold_secs: 2,
                ..Default::default()
            },
            10.0, // 10 fps → 2 seconds = 20 frames
        );

        let apnea_vitals = no_breathing_vitals();
        let mut apnea_alerts = Vec::new();
        for _ in 0..25 {
            let (_, mut a) = mon.process(0.01, &apnea_vitals, "r1", Some("bedroom".into()));
            apnea_alerts.append(&mut a);
        }
        assert!(apnea_alerts
            .iter()
            .any(|a| matches!(a.kind, CareAlertKind::SleepApneaEpisode { .. })));
    }

    #[test]
    fn apnea_alert_not_repeated() {
        let mut mon = SleepMonitor::new(
            SleepMonitorConfig {
                apnea_duration_threshold_secs: 1,
                ..Default::default()
            },
            10.0,
        );
        let apnea_vitals = no_breathing_vitals();
        let mut count = 0;
        for _ in 0..50 {
            let (_, a) = mon.process(0.01, &apnea_vitals, "r1", None);
            count += a
                .iter()
                .filter(|x| matches!(x.kind, CareAlertKind::SleepApneaEpisode { .. }))
                .count();
        }
        assert_eq!(count, 1);
    }
}
