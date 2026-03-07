//! Alert dispatching — sends care alerts to caregivers via webhook, MQTT,
//! or in-memory queue (for testing and local dashboard polling).

use std::sync::{Arc, Mutex};

use crate::domain::{CareAlert, CareAlertPriority};

/// A dispatched alert record kept in the in-memory log.
#[derive(Debug, Clone)]
pub struct DispatchedAlert {
    pub alert: CareAlert,
    pub channel: DispatchChannel,
}

/// The channel through which an alert was dispatched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchChannel {
    /// Stored in memory only (for dashboard polling / tests).
    InMemory,
    /// HTTP webhook POST.
    Webhook,
    /// MQTT publish.
    Mqtt,
}

/// Caregiver alert dispatcher.
///
/// Stores all alerts in-memory and (optionally) forwards urgent alerts
/// via webhook POST or MQTT publish. The webhook and MQTT integrations
/// are intentionally synchronous stubs — replace with async clients
/// (reqwest, rumqttc) for production use.
pub struct CareAlertDispatcher {
    /// In-memory alert log (shared so the dashboard can read it).
    log: Arc<Mutex<Vec<DispatchedAlert>>>,
    /// Optional HTTP webhook URL for urgent alerts.
    webhook_url: Option<String>,
    /// Optional MQTT topic for urgent alerts.
    mqtt_topic: Option<String>,
    /// Maximum alerts to retain in memory.
    max_log_size: usize,
}

impl CareAlertDispatcher {
    /// Create a dispatcher that stores alerts in memory only.
    pub fn in_memory() -> Self {
        Self {
            log: Arc::new(Mutex::new(Vec::new())),
            webhook_url: None,
            mqtt_topic: None,
            max_log_size: 500,
        }
    }

    /// Create a dispatcher with webhook and/or MQTT forwarding for urgent alerts.
    pub fn with_integrations(webhook_url: Option<String>, mqtt_topic: Option<String>) -> Self {
        Self {
            log: Arc::new(Mutex::new(Vec::new())),
            webhook_url,
            mqtt_topic,
            max_log_size: 500,
        }
    }

    /// Dispatch a care alert: log it and forward if urgent.
    pub fn dispatch(&self, alert: &CareAlert) {
        let is_urgent = alert.is_urgent();

        // Always log to memory
        self.log_alert(alert, DispatchChannel::InMemory);

        if is_urgent {
            if self.webhook_url.is_some() {
                self.dispatch_webhook(alert);
            }
            if self.mqtt_topic.is_some() {
                self.dispatch_mqtt(alert);
            }
        }

        tracing::info!(
            priority = %alert.priority,
            kind = %alert.kind,
            resident = %alert.resident_id,
            "CareAlert dispatched"
        );
    }

    fn log_alert(&self, alert: &CareAlert, channel: DispatchChannel) {
        if let Ok(mut log) = self.log.lock() {
            if log.len() >= self.max_log_size {
                log.remove(0);
            }
            log.push(DispatchedAlert {
                alert: alert.clone(),
                channel,
            });
        }
    }

    /// Stub: POST alert JSON to webhook URL.
    /// Replace with `reqwest::blocking::Client` or async equivalent in production.
    fn dispatch_webhook(&self, alert: &CareAlert) {
        tracing::info!(
            url = ?self.webhook_url,
            alert_id = %alert.id,
            "Webhook dispatch (stub — integrate reqwest for production)"
        );
        // Production: reqwest::blocking::Client::new()
        //   .post(url)
        //   .json(&alert)
        //   .send()?;
        self.log_alert(alert, DispatchChannel::Webhook);
    }

    /// Stub: Publish alert JSON to MQTT topic.
    /// Replace with `rumqttc` client in production.
    fn dispatch_mqtt(&self, alert: &CareAlert) {
        tracing::info!(
            topic = ?self.mqtt_topic,
            alert_id = %alert.id,
            "MQTT dispatch (stub — integrate rumqttc for production)"
        );
        // Production: client.publish(topic, QoS::AtLeastOnce, false, payload)?;
        self.log_alert(alert, DispatchChannel::Mqtt);
    }

    /// Return all logged alerts.
    pub fn all_alerts(&self) -> Vec<DispatchedAlert> {
        self.log.lock().map(|l| l.clone()).unwrap_or_default()
    }

    /// Return unacknowledged alerts only.
    pub fn pending_alerts(&self) -> Vec<CareAlert> {
        self.log
            .lock()
            .map(|l| {
                l.iter()
                    .filter(|d| !d.alert.acknowledged)
                    .map(|d| d.alert.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return all critical alerts from the log.
    pub fn critical_alerts(&self) -> Vec<CareAlert> {
        self.log
            .lock()
            .map(|l| {
                l.iter()
                    .filter(|d| d.alert.priority == CareAlertPriority::Critical)
                    .map(|d| d.alert.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Acknowledge an alert by ID.
    pub fn acknowledge(&self, alert_id: &str) -> bool {
        if let Ok(mut log) = self.log.lock() {
            for d in log.iter_mut() {
                if d.alert.id == alert_id {
                    d.alert.acknowledge();
                    return true;
                }
            }
        }
        false
    }

    /// Share the internal log reference (for dashboard integration).
    pub fn shared_log(&self) -> Arc<Mutex<Vec<DispatchedAlert>>> {
        Arc::clone(&self.log)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CareAlertKind, CareAlertPriority};

    fn make_alert(priority: CareAlertPriority) -> CareAlert {
        CareAlert::new("resident-1", CareAlertKind::FallDetected, priority, None)
    }

    #[test]
    fn in_memory_dispatch_stores_alert() {
        let dispatcher = CareAlertDispatcher::in_memory();
        dispatcher.dispatch(&make_alert(CareAlertPriority::Critical));
        assert_eq!(dispatcher.all_alerts().len(), 1);
    }

    #[test]
    fn pending_alerts_excludes_acknowledged() {
        let dispatcher = CareAlertDispatcher::in_memory();
        let alert = make_alert(CareAlertPriority::High);
        let id = alert.id.clone();
        dispatcher.dispatch(&alert);
        assert_eq!(dispatcher.pending_alerts().len(), 1);

        dispatcher.acknowledge(&id);
        assert_eq!(dispatcher.pending_alerts().len(), 0);
    }

    #[test]
    fn critical_alerts_filter() {
        let dispatcher = CareAlertDispatcher::in_memory();
        dispatcher.dispatch(&make_alert(CareAlertPriority::Critical));
        dispatcher.dispatch(&make_alert(CareAlertPriority::Low));
        assert_eq!(dispatcher.critical_alerts().len(), 1);
    }

    #[test]
    fn max_log_size_respected() {
        let mut dispatcher = CareAlertDispatcher::in_memory();
        dispatcher.max_log_size = 3;
        for _ in 0..10 {
            dispatcher.dispatch(&make_alert(CareAlertPriority::Info));
        }
        assert_eq!(dispatcher.all_alerts().len(), 3);
    }
}
