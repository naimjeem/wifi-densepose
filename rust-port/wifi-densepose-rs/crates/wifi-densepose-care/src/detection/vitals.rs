//! Vital sign monitoring — breathing and heart rate alerts for caregivers.

use crate::domain::{CareAlert, CareAlertKind, CareAlertPriority, VitalSnapshot};

/// Thresholds for vital sign alerts.
#[derive(Debug, Clone)]
pub struct VitalsMonitorConfig {
    /// Breathing below this = bradypnea alert (BPM).
    pub bradypnea_threshold: f32,
    /// Breathing below this = apnea alert (BPM).
    pub apnea_threshold: f32,
    /// Breathing above this = tachypnea alert (BPM).
    pub tachypnea_threshold: f32,
    /// Heart rate above this = tachycardia alert (BPM).
    pub tachycardia_threshold: f32,
    /// Heart rate below this = bradycardia alert (BPM).
    pub bradycardia_threshold: f32,
    /// Minimum signal quality to act on [0.0, 1.0].
    pub min_signal_quality: f32,
}

impl Default for VitalsMonitorConfig {
    fn default() -> Self {
        Self {
            bradypnea_threshold: 8.0,
            apnea_threshold: 4.0,
            tachypnea_threshold: 30.0,
            tachycardia_threshold: 120.0,
            bradycardia_threshold: 45.0,
            min_signal_quality: 0.4,
        }
    }
}

/// Vital sign monitor — converts a `VitalSnapshot` into care alerts.
pub struct VitalsMonitor {
    config: VitalsMonitorConfig,
}

impl VitalsMonitor {
    pub fn new(config: VitalsMonitorConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(VitalsMonitorConfig::default())
    }

    /// Check a vital snapshot and return any alerts for caregivers.
    pub fn check(
        &self,
        snapshot: &VitalSnapshot,
        resident_id: &str,
        room: Option<String>,
    ) -> Vec<CareAlert> {
        let mut alerts = Vec::new();

        if snapshot.signal_quality < self.config.min_signal_quality {
            return alerts;
        }

        // ── Respiratory rate ──────────────────────────────────────────────
        match snapshot.breathing_bpm {
            None => {
                // Signal present but no breathing detected
                alerts.push(CareAlert::new(
                    resident_id,
                    CareAlertKind::PossibleApnea { breathing_bpm: 0.0 },
                    CareAlertPriority::Critical,
                    room.clone(),
                ));
            }
            Some(rr) => {
                if rr < self.config.apnea_threshold {
                    alerts.push(CareAlert::new(
                        resident_id,
                        CareAlertKind::PossibleApnea { breathing_bpm: rr },
                        CareAlertPriority::Critical,
                        room.clone(),
                    ));
                } else if rr < self.config.bradypnea_threshold {
                    alerts.push(CareAlert::new(
                        resident_id,
                        CareAlertKind::BradypneaAlert { breathing_bpm: rr },
                        CareAlertPriority::High,
                        room.clone(),
                    ));
                } else if rr > self.config.tachypnea_threshold {
                    alerts.push(CareAlert::new(
                        resident_id,
                        CareAlertKind::TachypneaAlert { breathing_bpm: rr },
                        CareAlertPriority::High,
                        room.clone(),
                    ));
                }
            }
        }

        // ── Heart rate ───────────────────────────────────────────────────
        if let Some(hr) = snapshot.heart_rate_bpm {
            if hr > self.config.tachycardia_threshold {
                alerts.push(CareAlert::new(
                    resident_id,
                    CareAlertKind::TachycardiaAlert { heart_rate_bpm: hr },
                    CareAlertPriority::High,
                    room.clone(),
                ));
            } else if hr < self.config.bradycardia_threshold {
                alerts.push(CareAlert::new(
                    resident_id,
                    CareAlertKind::BradycardiaAlert { heart_rate_bpm: hr },
                    CareAlertPriority::High,
                    room.clone(),
                ));
            }
        }

        alerts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(rr: Option<f32>, hr: Option<f32>) -> VitalSnapshot {
        VitalSnapshot::new(rr, hr, 0.9)
    }

    #[test]
    fn normal_vitals_no_alerts() {
        let mon = VitalsMonitor::with_defaults();
        let alerts = mon.check(&snapshot(Some(16.0), Some(72.0)), "r1", None);
        assert!(alerts.is_empty());
    }

    #[test]
    fn apnea_alert() {
        let mon = VitalsMonitor::with_defaults();
        let alerts = mon.check(&snapshot(Some(2.0), Some(72.0)), "r1", None);
        assert_eq!(alerts.len(), 1);
        assert!(matches!(
            alerts[0].kind,
            CareAlertKind::PossibleApnea { .. }
        ));
        assert_eq!(alerts[0].priority, CareAlertPriority::Critical);
    }

    #[test]
    fn missing_breathing_is_apnea() {
        let mon = VitalsMonitor::with_defaults();
        let alerts = mon.check(&snapshot(None, Some(72.0)), "r1", None);
        assert!(matches!(
            alerts[0].kind,
            CareAlertKind::PossibleApnea { .. }
        ));
    }

    #[test]
    fn tachycardia_alert() {
        let mon = VitalsMonitor::with_defaults();
        let alerts = mon.check(&snapshot(Some(16.0), Some(135.0)), "r1", None);
        assert!(matches!(
            alerts[0].kind,
            CareAlertKind::TachycardiaAlert { .. }
        ));
    }

    #[test]
    fn bradycardia_alert() {
        let mon = VitalsMonitor::with_defaults();
        let alerts = mon.check(&snapshot(Some(16.0), Some(38.0)), "r1", None);
        assert!(matches!(
            alerts[0].kind,
            CareAlertKind::BradycardiaAlert { .. }
        ));
    }

    #[test]
    fn low_signal_quality_ignored() {
        let mon = VitalsMonitor::with_defaults();
        let s = VitalSnapshot::new(Some(2.0), Some(150.0), 0.1); // quality too low
        let alerts = mon.check(&s, "r1", None);
        assert!(alerts.is_empty());
    }
}
