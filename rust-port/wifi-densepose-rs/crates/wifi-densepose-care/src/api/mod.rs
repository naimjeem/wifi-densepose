//! HTTP API endpoints for the care monitor dashboard.
//!
//! Exposes:
//!   GET  /care/alerts           — all alerts (paginated)
//!   GET  /care/alerts/pending   — unacknowledged alerts
//!   GET  /care/alerts/critical  — critical alerts only
//!   POST /care/alerts/:id/ack   — acknowledge an alert
//!
//! These are plain data-transfer types — wire them to any HTTP framework
//! (Axum, Actix, etc.) or use the types directly with WebSocket streaming.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::domain::CareAlert;

/// API response wrapping a list of alerts.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AlertsResponse {
    pub alerts: Vec<AlertDto>,
    pub total: usize,
}

/// Data-transfer object for a single care alert.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AlertDto {
    pub id: String,
    pub resident_id: String,
    pub priority: String,
    pub kind: String,
    pub message: String,
    pub room: Option<String>,
    pub timestamp: String,
    pub acknowledged: bool,
}

impl From<&CareAlert> for AlertDto {
    fn from(a: &CareAlert) -> Self {
        Self {
            id: a.id.clone(),
            resident_id: a.resident_id.clone(),
            priority: a.priority.to_string(),
            kind: a.kind.to_string(),
            message: a.message.clone(),
            room: a.room.clone(),
            timestamp: a.timestamp.to_rfc3339(),
            acknowledged: a.acknowledged,
        }
    }
}

/// Acknowledge request body.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AckRequest {
    pub alert_id: String,
}

/// Acknowledge response.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AckResponse {
    pub success: bool,
    pub alert_id: String,
}

/// Build an `AlertsResponse` from a slice of `CareAlert`.
pub fn build_alerts_response(alerts: &[CareAlert]) -> AlertsResponse {
    let dtos: Vec<AlertDto> = alerts.iter().map(AlertDto::from).collect();
    let total = dtos.len();
    AlertsResponse {
        alerts: dtos,
        total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CareAlertKind, CareAlertPriority};

    #[test]
    fn dto_from_alert() {
        let alert = CareAlert::new(
            "resident-1",
            CareAlertKind::FallDetected,
            CareAlertPriority::Critical,
            Some("bedroom".into()),
        );
        let dto = AlertDto::from(&alert);
        assert_eq!(dto.resident_id, "resident-1");
        assert_eq!(dto.priority, "CRITICAL");
        assert_eq!(dto.room, Some("bedroom".into()));
        assert!(!dto.acknowledged);
    }

    #[test]
    fn build_response_counts_correctly() {
        let alerts = vec![
            CareAlert::new(
                "r1",
                CareAlertKind::FallDetected,
                CareAlertPriority::Critical,
                None,
            ),
            CareAlert::new(
                "r1",
                CareAlertKind::PossibleApnea { breathing_bpm: 2.0 },
                CareAlertPriority::Critical,
                None,
            ),
        ];
        let resp = build_alerts_response(&alerts);
        assert_eq!(resp.total, 2);
        assert_eq!(resp.alerts.len(), 2);
    }
}
