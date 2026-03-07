//! Room-level presence tracking.

use chrono::{DateTime, Utc};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Room presence state for a single room.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RoomPresence {
    /// Room name / label.
    pub room: String,
    /// True when at least one person is detected.
    pub occupied: bool,
    /// Number of people detected (best estimate).
    pub person_count: u8,
    /// When the room was last occupied.
    pub last_occupied: Option<DateTime<Utc>>,
    /// When this reading was taken.
    pub timestamp: DateTime<Utc>,
}

impl RoomPresence {
    /// Create a new presence reading.
    pub fn new(room: impl Into<String>, occupied: bool, person_count: u8) -> Self {
        let now = Utc::now();
        Self {
            room: room.into(),
            occupied,
            person_count,
            last_occupied: if occupied { Some(now) } else { None },
            timestamp: now,
        }
    }

    /// Minutes since the room was last occupied.
    /// Returns None if the room has never been recorded as occupied.
    pub fn minutes_since_occupied(&self) -> Option<i64> {
        self.last_occupied.map(|t| (Utc::now() - t).num_minutes())
    }

    /// True if the room has been empty for longer than `threshold` minutes.
    pub fn inactive_for(&self, threshold_minutes: i64) -> bool {
        match self.minutes_since_occupied() {
            Some(mins) => !self.occupied && mins >= threshold_minutes,
            None => false,
        }
    }
}

/// Tracks presence across all rooms in the home.
#[derive(Debug, Default)]
pub struct HomePresenceMap {
    rooms: Vec<RoomPresence>,
}

impl HomePresenceMap {
    /// Create a new empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update or insert presence data for a room.
    pub fn update(&mut self, presence: RoomPresence) {
        if let Some(existing) = self.rooms.iter_mut().find(|r| r.room == presence.room) {
            // Carry forward last_occupied if newly empty
            if !presence.occupied && existing.last_occupied.is_some() {
                let last = existing.last_occupied;
                *existing = presence;
                existing.last_occupied = last;
            } else {
                *existing = presence;
            }
        } else {
            self.rooms.push(presence);
        }
    }

    /// Get presence for a specific room.
    pub fn get(&self, room: &str) -> Option<&RoomPresence> {
        self.rooms.iter().find(|r| r.room == room)
    }

    /// Total number of people detected across all rooms.
    pub fn total_persons(&self) -> u8 {
        self.rooms.iter().map(|r| r.person_count).sum()
    }

    /// True if no one is detected anywhere in the home.
    pub fn home_empty(&self) -> bool {
        self.total_persons() == 0
    }

    /// Returns all rooms that have been empty longer than `threshold_minutes`.
    pub fn inactive_rooms(&self, threshold_minutes: i64) -> Vec<&RoomPresence> {
        self.rooms
            .iter()
            .filter(|r| r.inactive_for(threshold_minutes))
            .collect()
    }

    /// Return all rooms.
    pub fn all_rooms(&self) -> &[RoomPresence] {
        &self.rooms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn occupied_room() {
        let p = RoomPresence::new("bedroom", true, 1);
        assert!(p.occupied);
        assert_eq!(p.person_count, 1);
        assert!(p.last_occupied.is_some());
    }

    #[test]
    fn empty_room_not_inactive_immediately() {
        let p = RoomPresence::new("kitchen", false, 0);
        // minutes_since_occupied = None because we never recorded it occupied
        assert!(!p.inactive_for(30));
    }

    #[test]
    fn home_presence_map_total() {
        let mut map = HomePresenceMap::new();
        map.update(RoomPresence::new("bedroom", true, 1));
        map.update(RoomPresence::new("living-room", true, 1));
        assert_eq!(map.total_persons(), 2);
        assert!(!map.home_empty());
    }

    #[test]
    fn home_presence_map_update() {
        let mut map = HomePresenceMap::new();
        map.update(RoomPresence::new("bedroom", true, 1));
        map.update(RoomPresence::new("bedroom", false, 0));
        // last_occupied should be carried over
        assert!(map.get("bedroom").unwrap().last_occupied.is_some());
    }

    #[test]
    fn home_empty_when_all_zero() {
        let mut map = HomePresenceMap::new();
        map.update(RoomPresence::new("bedroom", false, 0));
        map.update(RoomPresence::new("kitchen", false, 0));
        assert!(map.home_empty());
    }
}
