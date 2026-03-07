//! Activity and posture tracking with inactivity alerts.

use chrono::{DateTime, Utc};

use crate::domain::{ActivityLevel, CareAlert, CareAlertKind, CareAlertPriority, PostureKind};

/// Configuration for activity monitoring.
#[derive(Debug, Clone)]
pub struct ActivityMonitorConfig {
    /// Minutes of same posture before issuing an alert.
    pub prolonged_posture_minutes: u64,
    /// Minutes with no activity in the whole home before alert.
    pub home_inactivity_minutes: u64,
}

impl Default for ActivityMonitorConfig {
    fn default() -> Self {
        Self {
            prolonged_posture_minutes: 120, // 2 hours in same posture
            home_inactivity_minutes: 60,    // 1 hour no movement
        }
    }
}

/// Tracks posture over time and raises alerts for concerning patterns.
pub struct ActivityMonitor {
    config: ActivityMonitorConfig,
    current_posture: PostureKind,
    posture_since: DateTime<Utc>,
    last_activity: DateTime<Utc>,
    prolonged_alert_issued: bool,
    inactivity_alert_issued: bool,
}

impl ActivityMonitor {
    pub fn new(config: ActivityMonitorConfig) -> Self {
        let now = Utc::now();
        Self {
            config,
            current_posture: PostureKind::Unknown,
            posture_since: now,
            last_activity: now,
            prolonged_alert_issued: false,
            inactivity_alert_issued: false,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(ActivityMonitorConfig::default())
    }

    /// Update with current posture and activity observations.
    /// Returns any care alerts generated.
    pub fn update(
        &mut self,
        posture: PostureKind,
        activity: ActivityLevel,
        resident_id: &str,
        room: Option<String>,
        now: DateTime<Utc>,
    ) -> Vec<CareAlert> {
        let mut alerts = Vec::new();

        // Track activity time
        if !matches!(activity, ActivityLevel::Stationary) {
            self.last_activity = now;
            self.inactivity_alert_issued = false;
        }

        // Posture change resets the prolonged timer
        if posture != self.current_posture {
            self.current_posture = posture;
            self.posture_since = now;
            self.prolonged_alert_issued = false;
        }

        // ── Prolonged same posture ────────────────────────────────────────
        let posture_mins = (now - self.posture_since).num_minutes() as u64;
        if posture_mins >= self.config.prolonged_posture_minutes
            && !self.prolonged_alert_issued
            && !matches!(self.current_posture, PostureKind::Unknown)
        {
            // Don't alert for normal prolonged sitting/sleeping in bedroom at night
            let is_sleeping_posture = matches!(self.current_posture, PostureKind::Lying)
                && matches!(activity, ActivityLevel::Stationary | ActivityLevel::Minimal);

            if !is_sleeping_posture {
                self.prolonged_alert_issued = true;
                alerts.push(CareAlert::new(
                    resident_id,
                    CareAlertKind::ProlongedInactivePosture {
                        posture: self.current_posture.to_string(),
                        minutes: posture_mins,
                    },
                    CareAlertPriority::Medium,
                    room.clone(),
                ));
            }
        }

        // ── Home inactivity ───────────────────────────────────────────────
        let inactive_mins = (now - self.last_activity).num_minutes() as u64;
        if inactive_mins >= self.config.home_inactivity_minutes && !self.inactivity_alert_issued {
            self.inactivity_alert_issued = true;
            alerts.push(CareAlert::new(
                resident_id,
                CareAlertKind::InactivityAlert {
                    room: room.clone().unwrap_or_else(|| "home".into()),
                    minutes_inactive: inactive_mins,
                },
                CareAlertPriority::Medium,
                room.clone(),
            ));
        }

        alerts
    }

    /// Minutes the resident has been in the current posture.
    pub fn posture_duration_minutes(&self) -> i64 {
        (Utc::now() - self.posture_since).num_minutes()
    }

    /// Minutes since any activity was detected.
    pub fn minutes_since_activity(&self) -> i64 {
        (Utc::now() - self.last_activity).num_minutes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn no_alert_on_normal_activity() {
        let mut mon = ActivityMonitor::with_defaults();
        let now = Utc::now();
        let alerts = mon.update(
            PostureKind::Sitting,
            ActivityLevel::Low,
            "r1",
            Some("living-room".into()),
            now,
        );
        assert!(alerts.is_empty());
    }

    #[test]
    fn prolonged_sitting_triggers_alert() {
        let mut mon = ActivityMonitor::new(ActivityMonitorConfig {
            prolonged_posture_minutes: 10,
            home_inactivity_minutes: 60,
        });

        let start = Utc::now() - Duration::minutes(15);
        mon.posture_since = start;
        mon.current_posture = PostureKind::Sitting;

        let alerts = mon.update(
            PostureKind::Sitting,
            ActivityLevel::Stationary,
            "r1",
            Some("living-room".into()),
            Utc::now(),
        );
        assert_eq!(alerts.len(), 1);
        assert!(matches!(
            alerts[0].kind,
            CareAlertKind::ProlongedInactivePosture { .. }
        ));
    }

    #[test]
    fn inactivity_alert_after_threshold() {
        let mut mon = ActivityMonitor::new(ActivityMonitorConfig {
            prolonged_posture_minutes: 120,
            home_inactivity_minutes: 5,
        });

        // Simulate 10 minutes of no activity
        mon.last_activity = Utc::now() - Duration::minutes(10);

        let alerts = mon.update(
            PostureKind::Unknown,
            ActivityLevel::Stationary,
            "r1",
            Some("bedroom".into()),
            Utc::now(),
        );
        assert!(alerts
            .iter()
            .any(|a| matches!(a.kind, CareAlertKind::InactivityAlert { .. })));
    }

    #[test]
    fn posture_change_resets_timer() {
        let mut mon = ActivityMonitor::new(ActivityMonitorConfig {
            prolonged_posture_minutes: 10,
            home_inactivity_minutes: 60,
        });
        let old_time = Utc::now() - Duration::minutes(20);
        mon.posture_since = old_time;
        mon.current_posture = PostureKind::Sitting;

        // Change to standing — should reset
        mon.update(
            PostureKind::Standing,
            ActivityLevel::Active,
            "r1",
            None,
            Utc::now(),
        );
        assert!(mon.posture_duration_minutes() < 2);
    }
}
