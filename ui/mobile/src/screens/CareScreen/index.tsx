import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  Animated,
  ScrollView,
  StyleSheet,
  TouchableOpacity,
  View,
} from 'react-native';
import { ThemedText } from '@/components/ThemedText';
import { ThemedView } from '@/components/ThemedView';
import { colors } from '@/theme/colors';

// ─── Types ────────────────────────────────────────────────────────────────────

type AlertPriority = 'CRITICAL' | 'HIGH' | 'MEDIUM' | 'LOW' | 'INFO';
type SleepStage = 'awake' | 'light' | 'deep' | 'REM' | 'unknown';
type PostureKind = 'lying' | 'sitting' | 'standing' | 'walking' | 'unknown';

interface CareAlert {
  id: string;
  resident_id: string;
  priority: AlertPriority;
  kind: string;
  message: string;
  room: string | null;
  timestamp: string;
  acknowledged: boolean;
}

interface ResidentStatus {
  resident_id: string;
  room: string;
  posture: PostureKind;
  sleep_stage: SleepStage;
  breathing_bpm: number | null;
  heart_rate_bpm: number | null;
  signal_quality: number;
  confidence: number;
  last_updated: string;
}

// ─── Simulated data (replace with real WebSocket/REST calls) ─────────────────

const SIMULATED_STATUS: ResidentStatus = {
  resident_id: 'resident-1',
  room: 'Bedroom',
  posture: 'lying',
  sleep_stage: 'light',
  breathing_bpm: 14.2,
  heart_rate_bpm: 61,
  signal_quality: 0.87,
  confidence: 0.91,
  last_updated: new Date().toISOString(),
};

const SIMULATED_ALERTS: CareAlert[] = [
  {
    id: 'a1',
    resident_id: 'resident-1',
    priority: 'CRITICAL',
    kind: 'FallDetected',
    message: 'Fall detected',
    room: 'Living Room',
    timestamp: new Date(Date.now() - 3 * 60000).toISOString(),
    acknowledged: false,
  },
  {
    id: 'a2',
    resident_id: 'resident-1',
    priority: 'HIGH',
    kind: 'BradycardiaAlert',
    message: 'Low heart rate: 38.0 BPM',
    room: 'Bedroom',
    timestamp: new Date(Date.now() - 12 * 60000).toISOString(),
    acknowledged: false,
  },
  {
    id: 'a3',
    resident_id: 'resident-1',
    priority: 'MEDIUM',
    kind: 'InactivityAlert',
    message: 'No activity in Kitchen for 75 min',
    room: 'Kitchen',
    timestamp: new Date(Date.now() - 75 * 60000).toISOString(),
    acknowledged: true,
  },
];

// ─── Helpers ──────────────────────────────────────────────────────────────────

const priorityColor = (p: AlertPriority): string => {
  switch (p) {
    case 'CRITICAL': return colors.danger;
    case 'HIGH':     return colors.warn;
    case 'MEDIUM':   return '#A855F7';
    case 'LOW':      return colors.accent;
    default:         return colors.muted;
  }
};

const sleepStageColor = (s: SleepStage): string => {
  switch (s) {
    case 'deep':  return '#6366F1';
    case 'REM':   return '#8B5CF6';
    case 'light': return colors.accent;
    case 'awake': return colors.success;
    default:      return colors.muted;
  }
};

const postureIcon = (p: PostureKind): string => {
  switch (p) {
    case 'lying':    return '🛌';
    case 'sitting':  return '🪑';
    case 'standing': return '🧍';
    case 'walking':  return '🚶';
    default:         return '❓';
  }
};

const relativeTime = (iso: string): string => {
  const diff = Math.floor((Date.now() - new Date(iso).getTime()) / 1000);
  if (diff < 60) return `${diff}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  return `${Math.floor(diff / 3600)}h ago`;
};

// ─── Sub-components ───────────────────────────────────────────────────────────

const PulsingDot = ({ color }: { color: string }) => {
  const anim = useRef(new Animated.Value(1)).current;

  useEffect(() => {
    const pulse = Animated.loop(
      Animated.sequence([
        Animated.timing(anim, { toValue: 0.3, duration: 800, useNativeDriver: true }),
        Animated.timing(anim, { toValue: 1.0, duration: 800, useNativeDriver: true }),
      ]),
    );
    pulse.start();
    return () => pulse.stop();
  }, [anim]);

  return (
    <Animated.View style={[styles.dot, { backgroundColor: color, opacity: anim }]} />
  );
};

const VitalChip = ({
  label,
  value,
  unit,
  normal,
}: {
  label: string;
  value: number | null;
  unit: string;
  normal: boolean;
}) => (
  <View style={[styles.vitalChip, { borderColor: normal ? colors.success : colors.warn }]}>
    <ThemedText preset="labelSm" color="textSecondary">{label}</ThemedText>
    <ThemedText preset="headingMd" style={{ color: normal ? colors.success : colors.warn }}>
      {value !== null ? `${value.toFixed(1)}` : '—'}
    </ThemedText>
    <ThemedText preset="labelSm" color="textSecondary">{unit}</ThemedText>
  </View>
);

const AlertCard = ({
  alert,
  onAck,
}: {
  alert: CareAlert;
  onAck: (id: string) => void;
}) => {
  const pc = priorityColor(alert.priority);
  return (
    <View style={[styles.alertCard, { borderLeftColor: pc, opacity: alert.acknowledged ? 0.55 : 1 }]}>
      <View style={styles.alertHeader}>
        <View style={styles.alertHeaderLeft}>
          {!alert.acknowledged && <PulsingDot color={pc} />}
          <ThemedText preset="labelMd" style={{ color: pc, marginLeft: 6 }}>
            {alert.priority}
          </ThemedText>
          {alert.room && (
            <ThemedText preset="labelSm" color="textSecondary" style={{ marginLeft: 8 }}>
              {alert.room}
            </ThemedText>
          )}
        </View>
        <ThemedText preset="labelSm" color="textSecondary">
          {relativeTime(alert.timestamp)}
        </ThemedText>
      </View>

      <ThemedText preset="bodyMd" style={styles.alertMessage}>
        {alert.message}
      </ThemedText>

      {!alert.acknowledged && (
        <TouchableOpacity
          style={[styles.ackButton, { borderColor: pc }]}
          onPress={() => onAck(alert.id)}
        >
          <ThemedText preset="labelSm" style={{ color: pc }}>
            Acknowledge
          </ThemedText>
        </TouchableOpacity>
      )}
    </View>
  );
};

const StatusCard = ({ status }: { status: ResidentStatus }) => {
  const stageColor = sleepStageColor(status.sleep_stage);
  const breathOk = status.breathing_bpm !== null && status.breathing_bpm >= 12 && status.breathing_bpm <= 20;
  const hrOk = status.heart_rate_bpm !== null && status.heart_rate_bpm >= 50 && status.heart_rate_bpm <= 100;

  return (
    <View style={styles.statusCard}>
      {/* Room + posture */}
      <View style={styles.statusRow}>
        <ThemedText style={styles.postureIcon}>{postureIcon(status.posture)}</ThemedText>
        <View>
          <ThemedText preset="headingMd">{status.room}</ThemedText>
          <ThemedText preset="labelSm" color="textSecondary">
            {status.posture.charAt(0).toUpperCase() + status.posture.slice(1)}
          </ThemedText>
        </View>
        <View style={{ flex: 1 }} />
        <View style={[styles.sleepBadge, { backgroundColor: `${stageColor}22`, borderColor: stageColor }]}>
          <ThemedText preset="labelSm" style={{ color: stageColor }}>
            {status.sleep_stage.toUpperCase()}
          </ThemedText>
        </View>
      </View>

      {/* Vitals row */}
      <View style={styles.vitalsRow}>
        <VitalChip
          label="Breathing"
          value={status.breathing_bpm}
          unit="BPM"
          normal={breathOk}
        />
        <VitalChip
          label="Heart Rate"
          value={status.heart_rate_bpm}
          unit="BPM"
          normal={hrOk}
        />
        <View style={styles.vitalChip}>
          <ThemedText preset="labelSm" color="textSecondary">Signal</ThemedText>
          <ThemedText preset="headingMd" style={{ color: colors.accent }}>
            {Math.round(status.signal_quality * 100)}%
          </ThemedText>
          <ThemedText preset="labelSm" color="textSecondary">quality</ThemedText>
        </View>
      </View>

      {/* Confidence bar */}
      <View style={styles.confRow}>
        <ThemedText preset="labelSm" color="textSecondary">Confidence</ThemedText>
        <View style={styles.confTrack}>
          <View style={[styles.confFill, { width: `${status.confidence * 100}%` }]} />
        </View>
        <ThemedText preset="labelSm" color="textSecondary">
          {Math.round(status.confidence * 100)}%
        </ThemedText>
      </View>
    </View>
  );
};

// ─── Main Screen ──────────────────────────────────────────────────────────────

export default function CareScreen() {
  const [alerts, setAlerts] = useState<CareAlert[]>(SIMULATED_ALERTS);
  const [status] = useState<ResidentStatus>(SIMULATED_STATUS);
  const [filter, setFilter] = useState<'all' | 'pending'>('pending');

  const pendingCount = alerts.filter((a) => !a.acknowledged).length;

  const handleAck = useCallback((id: string) => {
    setAlerts((prev) =>
      prev.map((a) => (a.id === id ? { ...a, acknowledged: true } : a)),
    );
    // TODO: POST /care/alerts/:id/ack to backend
  }, []);

  const handleAckAll = useCallback(() => {
    setAlerts((prev) => prev.map((a) => ({ ...a, acknowledged: true })));
  }, []);

  const displayed = filter === 'pending'
    ? alerts.filter((a) => !a.acknowledged)
    : alerts;

  return (
    <ThemedView style={styles.screen}>
      {/* Header */}
      <View style={styles.header}>
        <View>
          <ThemedText preset="headingLg">Care Monitor</ThemedText>
          <ThemedText preset="labelSm" color="textSecondary">
            Resident-1 · Home
          </ThemedText>
        </View>
        {pendingCount > 0 && (
          <View style={styles.badge}>
            <ThemedText preset="labelMd" style={{ color: colors.danger }}>
              {pendingCount} PENDING
            </ThemedText>
          </View>
        )}
      </View>

      <ScrollView
        style={styles.scroll}
        contentContainerStyle={styles.scrollContent}
        showsVerticalScrollIndicator={false}
      >
        {/* Live status */}
        <ThemedText preset="labelMd" color="textSecondary" style={styles.sectionLabel}>
          LIVE STATUS
        </ThemedText>
        <StatusCard status={status} />

        {/* Alert header row */}
        <View style={styles.alertsHeader}>
          <ThemedText preset="labelMd" color="textSecondary">
            ALERTS
          </ThemedText>
          <View style={styles.filterRow}>
            {(['pending', 'all'] as const).map((f) => (
              <TouchableOpacity
                key={f}
                style={[styles.filterBtn, filter === f && styles.filterBtnActive]}
                onPress={() => setFilter(f)}
              >
                <ThemedText
                  preset="labelSm"
                  style={{ color: filter === f ? colors.accent : colors.textSecondary }}
                >
                  {f.toUpperCase()}
                </ThemedText>
              </TouchableOpacity>
            ))}
            {pendingCount > 0 && (
              <TouchableOpacity style={styles.ackAllBtn} onPress={handleAckAll}>
                <ThemedText preset="labelSm" style={{ color: colors.muted }}>
                  Ack All
                </ThemedText>
              </TouchableOpacity>
            )}
          </View>
        </View>

        {displayed.length === 0 ? (
          <View style={styles.emptyState}>
            <ThemedText style={styles.emptyIcon}>✅</ThemedText>
            <ThemedText preset="bodyMd" color="textSecondary">
              No {filter === 'pending' ? 'pending' : ''} alerts
            </ThemedText>
          </View>
        ) : (
          displayed.map((alert) => (
            <AlertCard key={alert.id} alert={alert} onAck={handleAck} />
          ))
        )}
      </ScrollView>
    </ThemedView>
  );
}

// ─── Styles ───────────────────────────────────────────────────────────────────

const styles = StyleSheet.create({
  screen: {
    flex: 1,
    backgroundColor: colors.bg,
    paddingTop: 48,
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 16,
    paddingBottom: 12,
    borderBottomWidth: 1,
    borderBottomColor: colors.border,
  },
  badge: {
    backgroundColor: `${colors.danger}22`,
    borderWidth: 1,
    borderColor: colors.danger,
    borderRadius: 8,
    paddingHorizontal: 10,
    paddingVertical: 4,
  },
  scroll: { flex: 1 },
  scrollContent: { padding: 16, gap: 12, paddingBottom: 40 },
  sectionLabel: { marginBottom: 4 },

  // Status card
  statusCard: {
    backgroundColor: colors.surface,
    borderRadius: 14,
    borderWidth: 1,
    borderColor: `${colors.accent}44`,
    padding: 14,
    gap: 12,
  },
  statusRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 10,
  },
  postureIcon: { fontSize: 28 },
  sleepBadge: {
    borderWidth: 1,
    borderRadius: 8,
    paddingHorizontal: 8,
    paddingVertical: 3,
  },
  vitalsRow: {
    flexDirection: 'row',
    gap: 8,
  },
  vitalChip: {
    flex: 1,
    alignItems: 'center',
    backgroundColor: colors.surfaceAlt,
    borderRadius: 10,
    borderWidth: 1,
    borderColor: colors.border,
    paddingVertical: 8,
    gap: 2,
  },
  confRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
  },
  confTrack: {
    flex: 1,
    height: 6,
    borderRadius: 999,
    backgroundColor: colors.surfaceAlt,
    overflow: 'hidden',
  },
  confFill: {
    height: '100%',
    backgroundColor: colors.accent,
    borderRadius: 999,
  },

  // Alert list
  alertsHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginTop: 8,
  },
  filterRow: {
    flexDirection: 'row',
    gap: 6,
    alignItems: 'center',
  },
  filterBtn: {
    paddingHorizontal: 8,
    paddingVertical: 3,
    borderRadius: 6,
    borderWidth: 1,
    borderColor: colors.border,
  },
  filterBtnActive: {
    borderColor: colors.accent,
    backgroundColor: `${colors.accent}18`,
  },
  ackAllBtn: {
    paddingHorizontal: 8,
    paddingVertical: 3,
  },

  // Alert card
  alertCard: {
    backgroundColor: colors.surface,
    borderRadius: 12,
    borderWidth: 1,
    borderColor: colors.border,
    borderLeftWidth: 3,
    padding: 12,
    gap: 8,
  },
  alertHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
  },
  alertHeaderLeft: {
    flexDirection: 'row',
    alignItems: 'center',
  },
  alertMessage: {
    color: colors.textPrimary,
  },
  ackButton: {
    alignSelf: 'flex-start',
    borderWidth: 1,
    borderRadius: 6,
    paddingHorizontal: 10,
    paddingVertical: 3,
  },

  // Misc
  dot: {
    width: 8,
    height: 8,
    borderRadius: 4,
  },
  emptyState: {
    alignItems: 'center',
    paddingVertical: 32,
    gap: 8,
  },
  emptyIcon: { fontSize: 32 },
});
