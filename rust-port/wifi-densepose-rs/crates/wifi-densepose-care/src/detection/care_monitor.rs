//! CareMonitor — top-level orchestrator that wires all detectors together.
//!
//! Accepts a single `SensingFrame` per tick (from the WiFi-DensePose pipeline)
//! and routes it through:
//!   - Fall detection
//!   - Vital sign monitoring
//!   - Activity / posture tracking
//!   - Emergency motion detection
//!   - Sleep monitoring (when posture is Lying + stationary)
//!
//! Produces `CareAlert` events to be dispatched by `CareAlertDispatcher`.

use chrono::Utc;

use crate::{
    alerting::CareAlertDispatcher,
    detection::{
        activity::{ActivityMonitor, ActivityMonitorConfig},
        emergency::{EmergencyDetector, EmergencyDetectorConfig},
        fall::{FallDetector, FallDetectorConfig, MotionFrame},
        sleep::{SleepMonitor, SleepMonitorConfig},
        vitals::{VitalsMonitor, VitalsMonitorConfig},
    },
    domain::{ActivityLevel, CareAlert, PostureKind, SleepStage, VitalSnapshot},
};

/// A single sensing frame from the WiFi-DensePose pipeline.
#[derive(Debug, Clone)]
pub struct SensingFrame {
    /// Resident identifier (configured by caregiver).
    pub resident_id: String,
    /// Room the resident is in.
    pub room: Option<String>,
    /// Normalised motion score [0.0, 1.0].
    pub motion_score: f32,
    /// Estimated normalised body height [0.0 = floor, 1.0 = standing].
    pub height_norm: f32,
    /// Inferred posture.
    pub posture: PostureKind,
    /// Inferred activity level.
    pub activity: ActivityLevel,
    /// Vital sign snapshot.
    pub vitals: VitalSnapshot,
    /// Detection confidence [0.0, 1.0].
    pub confidence: f32,
}

/// Full configuration for the care monitor.
#[derive(Debug, Clone, Default)]
pub struct CareMonitorConfig {
    pub fall: FallDetectorConfig,
    pub vitals: VitalsMonitorConfig,
    pub activity: ActivityMonitorConfig,
    pub emergency: EmergencyDetectorConfig,
    pub sleep: SleepMonitorConfig,
    /// Sensor frames per second (used by sleep monitor).
    pub fps: f32,
}

/// Output produced per sensing frame.
#[derive(Debug)]
pub struct CareMonitorOutput {
    /// Any care alerts generated this frame.
    pub alerts: Vec<CareAlert>,
    /// Current sleep stage (only meaningful during sleeping posture).
    pub sleep_stage: SleepStage,
}

/// The top-level care monitor — one instance per resident.
pub struct CareMonitor {
    fall_detector: FallDetector,
    vitals_monitor: VitalsMonitor,
    activity_monitor: ActivityMonitor,
    emergency_detector: EmergencyDetector,
    sleep_monitor: SleepMonitor,
    dispatcher: CareAlertDispatcher,
}

impl CareMonitor {
    /// Create a new care monitor with custom configuration.
    pub fn new(config: CareMonitorConfig, dispatcher: CareAlertDispatcher) -> Self {
        let fps = if config.fps > 0.0 { config.fps } else { 10.0 };
        Self {
            fall_detector: FallDetector::new(config.fall),
            vitals_monitor: VitalsMonitor::new(config.vitals),
            activity_monitor: ActivityMonitor::new(config.activity),
            emergency_detector: EmergencyDetector::new(config.emergency),
            sleep_monitor: SleepMonitor::new(config.sleep, fps),
            dispatcher,
        }
    }

    /// Create with defaults.
    pub fn with_defaults() -> Self {
        Self::new(
            CareMonitorConfig::default(),
            CareAlertDispatcher::in_memory(),
        )
    }

    /// Process a single sensing frame. Dispatches all generated alerts.
    pub fn process(&mut self, frame: &SensingFrame) -> CareMonitorOutput {
        let now = Utc::now();
        let mut all_alerts: Vec<CareAlert> = Vec::new();

        let motion_frame =
            MotionFrame::new(frame.motion_score, frame.height_norm, frame.confidence);

        // ── Fall detection ────────────────────────────────────────────────
        let mut fall_alerts =
            self.fall_detector
                .process(&motion_frame, &frame.resident_id, frame.room.clone());
        all_alerts.append(&mut fall_alerts);

        // ── Vital signs ───────────────────────────────────────────────────
        let mut vital_alerts =
            self.vitals_monitor
                .check(&frame.vitals, &frame.resident_id, frame.room.clone());
        all_alerts.append(&mut vital_alerts);

        // ── Activity / posture ────────────────────────────────────────────
        let mut activity_alerts = self.activity_monitor.update(
            frame.posture,
            frame.activity.clone(),
            &frame.resident_id,
            frame.room.clone(),
            now,
        );
        all_alerts.append(&mut activity_alerts);

        // ── Emergency motion ──────────────────────────────────────────────
        let mut emerg_alerts = self.emergency_detector.process(
            frame.motion_score,
            now,
            &frame.resident_id,
            frame.room.clone(),
        );
        all_alerts.append(&mut emerg_alerts);

        // ── Sleep monitoring (when lying still) ───────────────────────────
        let sleep_stage = if frame.posture == PostureKind::Lying
            && matches!(
                frame.activity,
                ActivityLevel::Stationary | ActivityLevel::Minimal
            ) {
            let (stage, mut sleep_alerts) = self.sleep_monitor.process(
                frame.motion_score,
                &frame.vitals,
                &frame.resident_id,
                frame.room.clone(),
            );
            all_alerts.append(&mut sleep_alerts);
            stage
        } else {
            SleepStage::Awake
        };

        // Dispatch all alerts
        for alert in &all_alerts {
            self.dispatcher.dispatch(alert);
        }

        CareMonitorOutput {
            alerts: all_alerts,
            sleep_stage,
        }
    }

    /// Access the dispatcher to query recent alerts.
    pub fn dispatcher(&self) -> &CareAlertDispatcher {
        &self.dispatcher
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::VitalSnapshot;

    fn normal_frame(resident_id: &str) -> SensingFrame {
        SensingFrame {
            resident_id: resident_id.into(),
            room: Some("living-room".into()),
            motion_score: 0.3,
            height_norm: 0.9,
            posture: PostureKind::Standing,
            activity: ActivityLevel::Low,
            vitals: VitalSnapshot::new(Some(16.0), Some(72.0), 0.9),
            confidence: 0.9,
        }
    }

    #[test]
    fn normal_frame_no_alerts() {
        let mut mon = CareMonitor::with_defaults();
        let output = mon.process(&normal_frame("r1"));
        assert!(output.alerts.is_empty());
        assert_eq!(output.sleep_stage, SleepStage::Awake);
    }

    #[test]
    fn fall_frame_generates_alert() {
        let mut mon = CareMonitor::with_defaults();

        // Impact frame
        mon.process(&SensingFrame {
            resident_id: "r1".into(),
            room: Some("bedroom".into()),
            motion_score: 0.9,
            height_norm: 0.2,
            posture: PostureKind::Lying,
            activity: ActivityLevel::Agitated,
            vitals: VitalSnapshot::new(Some(20.0), Some(95.0), 0.85),
            confidence: 0.9,
        });

        // Stillness on floor
        let output = mon.process(&SensingFrame {
            resident_id: "r1".into(),
            room: Some("bedroom".into()),
            motion_score: 0.05,
            height_norm: 0.1,
            posture: PostureKind::Lying,
            activity: ActivityLevel::Stationary,
            vitals: VitalSnapshot::new(Some(14.0), Some(65.0), 0.8),
            confidence: 0.9,
        });

        let has_fall = output
            .alerts
            .iter()
            .any(|a| matches!(a.kind, crate::domain::CareAlertKind::FallDetected));
        assert!(has_fall, "Expected fall alert");
    }

    #[test]
    fn apnea_generates_critical_alert() {
        let mut mon = CareMonitor::with_defaults();
        let output = mon.process(&SensingFrame {
            resident_id: "r1".into(),
            room: Some("bedroom".into()),
            motion_score: 0.02,
            height_norm: 0.05,
            posture: PostureKind::Lying,
            activity: ActivityLevel::Stationary,
            vitals: VitalSnapshot::new(Some(2.0), Some(55.0), 0.85),
            confidence: 0.9,
        });

        let has_apnea = output
            .alerts
            .iter()
            .any(|a| matches!(a.kind, crate::domain::CareAlertKind::PossibleApnea { .. }));
        assert!(has_apnea);
    }
}
