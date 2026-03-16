// Data Processor - WiFi DensePose 3D Visualization
// Transforms API data into Three.js geometry updates

export class DataProcessor {
  constructor(options = {}) {
    // Demo mode state
    this.demoMode = false;
    this.demoElapsed = 0;
    this.demoPoseIndex = 0;
    this.demoPoseCycleTime = 4; // seconds per pose transition

    // Room dimensions (meters) for mapping people into the 3D environment.
    // Defaults match the Environment room unless overridden by the caller.
    this.roomWidth = options.roomWidth || 8;
    this.roomDepth = options.roomDepth || 6;

    // Pre-recorded demo poses (COCO 17-keypoint format, normalized [0,1])
    // Each pose: array of {x, y, confidence} for 17 keypoints
    this.demoPoses = this._buildDemoPoses();

    // Smoothing buffers
    this._lastProcessedPersons = [];
    this._smoothingFactor = 0.3;
  }

  // Process incoming WebSocket message into visualization-ready data
  processMessage(message) {
    if (!message) return null;

    const result = {
      persons: [],
      zoneOccupancy: {},
      signalData: null,
      nodes: null,
      metadata: {
        isRealData: false,
        timestamp: null,
        processingTime: 0,
        frameId: null,
        sensingMode: 'Mock'
      }
    };

    // Sensing server sends raw SensingUpdate: msg_type, nodes, persons, source, ...
    const isSensingUpdate = message.msg_type === 'sensing_update' || message.type === 'sensing_update';
    if (isSensingUpdate) {
      const payload = message;
      const persons = this._extractPersons(payload);
      result.persons = this._attachWorldPositions(persons);
      result.zoneOccupancy = this._extractZoneOccupancy(payload, message.zone_id);
      result.signalData = this._extractSignalData(payload);
      if (Array.isArray(payload.nodes) && payload.nodes.length > 0) {
        result.nodes = payload.nodes.map((n) => ({
          node_id: n.node_id,
          position: Array.isArray(n.position) ? n.position : [0, 0, 0],
          rssi_dbm: n.rssi_dbm
        }));
      }
      const source = payload.source || '';
      result.metadata.isRealData = source !== 'mock' && source !== 'demo' && source !== '';
      result.metadata.timestamp = payload.timestamp;
      result.metadata.sensingMode = (() => {
        const sourceMap = {
          'esp32': 'CSI', 'csi': 'CSI', 'wifi': 'WiFi',
          'rssi': 'RSSI', 'simulated': 'Simulated', 'simulate': 'Simulated'
        };
        return sourceMap[source] || (source || 'Unknown');
      })();
      return result;
    }

    // Handle pose_data wrapper from API
    if (message.type === 'pose_data') {
      const payload = message.data || message.payload;
      if (payload) {
        const persons = this._extractPersons(payload);
        result.persons = this._attachWorldPositions(persons);
        result.zoneOccupancy = this._extractZoneOccupancy(payload, message.zone_id);
        result.signalData = this._extractSignalData(payload);
        if (Array.isArray(payload.nodes) && payload.nodes.length > 0) {
          result.nodes = payload.nodes.map((n) => ({
            node_id: n.node_id,
            position: Array.isArray(n.position) ? n.position : [0, 0, 0],
            rssi_dbm: n.rssi_dbm
          }));
        }

        const meta = payload.metadata || {};
        const source = meta.source || '';
        result.metadata.isRealData = source !== 'mock' && source !== 'demo' && source !== '';
        result.metadata.timestamp = message.timestamp;
        result.metadata.processingTime = meta.processing_time_ms || 0;
        result.metadata.frameId = meta.frame_id;
        result.metadata.poseSource = payload.pose_source || 'unknown';
        result.metadata.signalStrength = meta.signal_strength;
        result.metadata.motionBandPower = meta.motion_band_power;

        const sourceMap = {
          'esp32': 'CSI', 'csi': 'CSI', 'wifi': 'WiFi',
          'rssi': 'RSSI', 'simulated': 'Simulated', 'simulate': 'Simulated',
        };
        result.metadata.sensingMode = sourceMap[source] || (source || 'Unknown');
      }
    }

    return result;
  }

  // Attach world-space floor positions (meters) for each person.
  // Uses ankle center when available (feet on floor), else hip, so 3D placement and room lookup are correct.
  _attachWorldPositions(persons) {
    if (!persons || persons.length === 0) return [];

    const roomW = this.roomWidth || 8;
    const roomD = this.roomDepth || 6;

    return persons.map((person) => {
      const kp = person.keypoints || [];
      if (kp.length < 13) {
        return { ...person, worldPosition: null };
      }

      const leftHip = kp[11];
      const rightHip = kp[12];
      const leftAnkle = kp[15];
      const rightAnkle = kp[16];

      const hipOk = leftHip && rightHip && (leftHip.confidence ?? 0) > 0.05 && (rightHip.confidence ?? 0) > 0.05;
      const ankleOk = leftAnkle && rightAnkle && (leftAnkle.confidence ?? 0) > 0.1 && (rightAnkle.confidence ?? 0) > 0.1;

      let normX, normZ;
      if (ankleOk) {
        normX = (leftAnkle.x + rightAnkle.x) / 2;
        normZ = (leftAnkle.y + rightAnkle.y) / 2;
      } else if (hipOk) {
        normX = (leftHip.x + rightHip.x) / 2;
        normZ = (leftHip.y + rightHip.y) / 2;
      } else {
        return { ...person, worldPosition: null };
      }

      // Map normalized [0,1] to room: center (0.5,0.5) -> (0,0), same scale as heatmap
      const worldX = (normX - 0.5) * roomW;
      const worldZ = (normZ - 0.5) * roomD;

      return {
        ...person,
        worldPosition: { x: worldX, z: worldZ }
      };
    });
  }

  // Extract person data with keypoints in COCO format
  _extractPersons(payload) {
    const persons = [];

    const poseStateFromServer = (p) => {
      const v = p.pose_state ?? p.activity ?? p.posture ?? p.classification;
      if (v == null) return null;
      const s = String(v).toLowerCase();
      if (s.includes('sit')) return 'Sitting';
      if (s.includes('walk') || s.includes('moving')) return 'Walking';
      if (s.includes('stand') || s.includes('still')) return 'Standing';
      return null;
    };

    if (payload.pose && payload.pose.persons) {
      for (const person of payload.pose.persons) {
        const keypoints = this._normalizeKeypoints(person.keypoints);
        const serverPosture = poseStateFromServer(person);
        const payloadRoot = payload.classification ?? payload.activity;
        const rootPosture = payloadRoot != null ? poseStateFromServer({ classification: payloadRoot }) : null;
        persons.push({
          id: person.id || `person_${persons.length}`,
          confidence: person.confidence || 0,
          keypoints,
          bbox: person.bbox || null,
          body_parts: person.densepose_parts || person.body_parts || null,
          pose_state: serverPosture ?? rootPosture ?? this._classifyPoseState(keypoints)
        });
      }
    } else if (payload.persons) {
      for (const person of payload.persons) {
        const keypoints = this._normalizeKeypoints(person.keypoints);
        const serverPosture = poseStateFromServer(person);
        persons.push({
          id: person.id || `person_${persons.length}`,
          confidence: person.confidence || 0,
          keypoints,
          bbox: person.bbox || null,
          body_parts: person.densepose_parts || person.body_parts || null,
          pose_state: serverPosture ?? this._classifyPoseState(keypoints)
        });
      }
    }

    return persons;
  }

  // Normalize keypoints to {x, y, confidence} format in [0,1] range
  _normalizeKeypoints(keypoints) {
    if (!keypoints || keypoints.length === 0) return [];

    const raw = keypoints.map(kp => {
      if (Array.isArray(kp)) {
        return { x: kp[0], y: kp[1], confidence: kp[2] || 0.5 };
      }
      return {
        x: kp.x !== undefined ? kp.x : 0,
        y: kp.y !== undefined ? kp.y : 0,
        confidence: kp.confidence !== undefined ? kp.confidence : (kp.score || 0.5)
      };
    });

    // Auto-detect if values are in pixel coords (>1.0) and normalize to [0,1]
    const maxX = Math.max(...raw.map(k => Math.abs(k.x)));
    const maxY = Math.max(...raw.map(k => Math.abs(k.y)));

    if (maxX > 1.5 || maxY > 1.5) {
      const frameW = Math.max(maxX * 1.1, 640);
      const frameH = Math.max(maxY * 1.1, 480);
      return raw.map(kp => ({
        x: Math.max(0, Math.min(1, kp.x / frameW)),
        y: Math.max(0, Math.min(1, kp.y / frameH)),
        confidence: kp.confidence
      }));
    }

    return raw;
  }

  // Extract zone occupancy data
  _extractZoneOccupancy(payload, zoneId) {
    const occupancy = {};

    if (payload.zone_summary) {
      Object.assign(occupancy, payload.zone_summary);
    }

    if (zoneId && payload.pose?.persons?.length > 0) {
      occupancy[zoneId] = payload.pose.persons.length;
    }

    return occupancy;
  }

  // Extract signal/CSI data if available
  _extractSignalData(payload) {
    if (payload.signal_data || payload.csi_data) {
      const sig = payload.signal_data || payload.csi_data;
      return {
        amplitude: sig.amplitude || null,
        phase: sig.phase || null,
        doppler: sig.doppler || sig.doppler_spectrum || null,
        motionEnergy: sig.motion_energy !== undefined ? sig.motion_energy : null
      };
    }
    return null;
  }

  // Demo path through the flat (world x, z) so HUD shows different rooms, not always Hallway.
  // Waypoints align with building-layout room centers; cycle ~24s, then 3s "empty" (0 persons).
  _getDemoPathPosition(elapsed) {
    const pathSec = 24;
    const emptySec = 3;
    const cycle = pathSec + emptySec;
    const t = (elapsed % cycle) / pathSec; // 0..1 over path, then we're in "empty" phase when t wraps
    const inEmptyPhase = (elapsed % cycle) >= pathSec;
    if (inEmptyPhase) return { x: 0, z: 0, empty: true };

    // Waypoints: Living -> Hallway -> Bedroom 1 -> Hallway -> Bedroom 2 -> Kitchen -> Living
    const waypoints = [
      [0, -1.5], [0, 0.5], [-2.5, 2.0], [0, 0.5], [2.5, 2.0], [2.75, -1.5], [0, -1.5]
    ];
    const seg = 1 / (waypoints.length - 1);
    const segIdx = Math.min(Math.floor(t / seg), waypoints.length - 2);
    const localT = (t - segIdx * seg) / seg;
    const smoothT = localT * localT * (3 - 2 * localT);
    const a = waypoints[segIdx];
    const b = waypoints[segIdx + 1];
    const x = a[0] + (b[0] - a[0]) * smoothT;
    const z = a[1] + (b[1] - a[1]) * smoothT;
    return { x, z, empty: false };
  }

  // Generate demo data that cycles through poses and moves through rooms (so count/room vary).
  generateDemoData(deltaTime) {
    this.demoElapsed += deltaTime;

    const pathState = this._getDemoPathPosition(this.demoElapsed);
    if (pathState.empty) {
      return {
        persons: [],
        zoneOccupancy: {},
        signalData: null,
        metadata: {
          isRealData: false,
          timestamp: new Date().toISOString(),
          processingTime: 10,
          frameId: `demo_${Math.floor(this.demoElapsed * 30)}`,
          sensingMode: 'Mock'
        }
      };
    }

    const totalPoses = this.demoPoses.length;
    const cycleProgress = (this.demoElapsed % (this.demoPoseCycleTime * totalPoses)) / this.demoPoseCycleTime;
    const currentPoseIdx = Math.floor(cycleProgress) % totalPoses;
    const nextPoseIdx = (currentPoseIdx + 1) % totalPoses;
    const t = cycleProgress - Math.floor(cycleProgress);
    const smoothT = t * t * (3 - 2 * t);

    const currentPose = this.demoPoses[currentPoseIdx];
    const nextPose = this.demoPoses[nextPoseIdx];

    const interpolatedKeypoints = currentPose.map((kp, i) => {
      const next = nextPose[i];
      return {
        x: kp.x + (next.x - kp.x) * smoothT,
        y: kp.y + (next.y - kp.y) * smoothT,
        confidence: 0.7 + Math.sin(this.demoElapsed * 2 + i * 0.5) * 0.2
      };
    });

    const baseConf = 0.65 + Math.sin(this.demoElapsed * 0.5) * 0.2;
    const roomW = this.roomWidth || 8;
    const roomD = this.roomDepth || 6;
    const hipX = (interpolatedKeypoints[11].x + interpolatedKeypoints[12].x) / 2;
    let activeZone = 'zone_2';
    if (hipX < 0.35) activeZone = 'zone_1';
    else if (hipX > 0.65) activeZone = 'zone_3';

    const poseIndexMod = currentPoseIdx % 10;
    let poseState = 'Standing';
    if (poseIndexMod === 1 || poseIndexMod === 3) poseState = 'Walking';
    else if (poseIndexMod === 6) poseState = 'Sitting';

    return {
      persons: [{
        id: 'demo_person_0',
        confidence: Math.max(0, Math.min(1, baseConf)),
        keypoints: interpolatedKeypoints,
        bbox: null,
        body_parts: this._generateDemoBodyParts(this.demoElapsed),
        worldPosition: { x: pathState.x, z: pathState.z },
        pose_state: poseState
      }],
      zoneOccupancy: { [activeZone]: 1 },
      signalData: null,
      metadata: {
        isRealData: false,
        timestamp: new Date().toISOString(),
        processingTime: 8 + Math.random() * 5,
        frameId: `demo_${Math.floor(this.demoElapsed * 30)}`,
        sensingMode: 'Mock'
      }
    };
  }

  _generateDemoBodyParts(elapsed) {
    const parts = {};
    for (let i = 1; i <= 24; i++) {
      // Simulate body parts being detected with varying confidence
      // Create a wave pattern across parts
      parts[i] = 0.4 + Math.sin(elapsed * 1.2 + i * 0.5) * 0.3 + Math.random() * 0.1;
      parts[i] = Math.max(0, Math.min(1, parts[i]));
    }
    return parts;
  }

  _buildDemoPoses() {
    // Pre-recorded poses: normalized COCO 17 keypoints
    // Each keypoint: {x, y, confidence}
    // Standing at center
    const standing = [
      { x: 0.50, y: 0.12, confidence: 0.9 },  // 0: nose
      { x: 0.48, y: 0.10, confidence: 0.8 },  // 1: left_eye
      { x: 0.52, y: 0.10, confidence: 0.8 },  // 2: right_eye
      { x: 0.46, y: 0.12, confidence: 0.7 },  // 3: left_ear
      { x: 0.54, y: 0.12, confidence: 0.7 },  // 4: right_ear
      { x: 0.42, y: 0.22, confidence: 0.9 },  // 5: left_shoulder
      { x: 0.58, y: 0.22, confidence: 0.9 },  // 6: right_shoulder
      { x: 0.38, y: 0.38, confidence: 0.85 }, // 7: left_elbow
      { x: 0.62, y: 0.38, confidence: 0.85 }, // 8: right_elbow
      { x: 0.36, y: 0.52, confidence: 0.8 },  // 9: left_wrist
      { x: 0.64, y: 0.52, confidence: 0.8 },  // 10: right_wrist
      { x: 0.45, y: 0.50, confidence: 0.9 },  // 11: left_hip
      { x: 0.55, y: 0.50, confidence: 0.9 },  // 12: right_hip
      { x: 0.44, y: 0.70, confidence: 0.85 }, // 13: left_knee
      { x: 0.56, y: 0.70, confidence: 0.85 }, // 14: right_knee
      { x: 0.44, y: 0.90, confidence: 0.8 },  // 15: left_ankle
      { x: 0.56, y: 0.90, confidence: 0.8 }   // 16: right_ankle
    ];

    // Walking - left leg forward
    const walkLeft = [
      { x: 0.50, y: 0.12, confidence: 0.9 },
      { x: 0.48, y: 0.10, confidence: 0.8 },
      { x: 0.52, y: 0.10, confidence: 0.8 },
      { x: 0.46, y: 0.12, confidence: 0.7 },
      { x: 0.54, y: 0.12, confidence: 0.7 },
      { x: 0.42, y: 0.22, confidence: 0.9 },
      { x: 0.58, y: 0.22, confidence: 0.9 },
      { x: 0.40, y: 0.35, confidence: 0.85 },
      { x: 0.60, y: 0.40, confidence: 0.85 },
      { x: 0.42, y: 0.48, confidence: 0.8 },
      { x: 0.56, y: 0.55, confidence: 0.8 },
      { x: 0.45, y: 0.50, confidence: 0.9 },
      { x: 0.55, y: 0.50, confidence: 0.9 },
      { x: 0.40, y: 0.68, confidence: 0.85 },
      { x: 0.58, y: 0.72, confidence: 0.85 },
      { x: 0.38, y: 0.88, confidence: 0.8 },
      { x: 0.56, y: 0.90, confidence: 0.8 }
    ];

    // Walking - right leg forward
    const walkRight = [
      { x: 0.50, y: 0.12, confidence: 0.9 },
      { x: 0.48, y: 0.10, confidence: 0.8 },
      { x: 0.52, y: 0.10, confidence: 0.8 },
      { x: 0.46, y: 0.12, confidence: 0.7 },
      { x: 0.54, y: 0.12, confidence: 0.7 },
      { x: 0.42, y: 0.22, confidence: 0.9 },
      { x: 0.58, y: 0.22, confidence: 0.9 },
      { x: 0.38, y: 0.40, confidence: 0.85 },
      { x: 0.62, y: 0.35, confidence: 0.85 },
      { x: 0.36, y: 0.55, confidence: 0.8 },
      { x: 0.60, y: 0.48, confidence: 0.8 },
      { x: 0.45, y: 0.50, confidence: 0.9 },
      { x: 0.55, y: 0.50, confidence: 0.9 },
      { x: 0.47, y: 0.72, confidence: 0.85 },
      { x: 0.52, y: 0.68, confidence: 0.85 },
      { x: 0.47, y: 0.90, confidence: 0.8 },
      { x: 0.50, y: 0.88, confidence: 0.8 }
    ];

    // Arms raised
    const armsUp = [
      { x: 0.50, y: 0.12, confidence: 0.9 },
      { x: 0.48, y: 0.10, confidence: 0.8 },
      { x: 0.52, y: 0.10, confidence: 0.8 },
      { x: 0.46, y: 0.12, confidence: 0.7 },
      { x: 0.54, y: 0.12, confidence: 0.7 },
      { x: 0.42, y: 0.22, confidence: 0.9 },
      { x: 0.58, y: 0.22, confidence: 0.9 },
      { x: 0.38, y: 0.15, confidence: 0.85 },
      { x: 0.62, y: 0.15, confidence: 0.85 },
      { x: 0.36, y: 0.05, confidence: 0.8 },
      { x: 0.64, y: 0.05, confidence: 0.8 },
      { x: 0.45, y: 0.50, confidence: 0.9 },
      { x: 0.55, y: 0.50, confidence: 0.9 },
      { x: 0.44, y: 0.70, confidence: 0.85 },
      { x: 0.56, y: 0.70, confidence: 0.85 },
      { x: 0.44, y: 0.90, confidence: 0.8 },
      { x: 0.56, y: 0.90, confidence: 0.8 }
    ];

    // Sitting
    const sitting = [
      { x: 0.50, y: 0.22, confidence: 0.9 },
      { x: 0.48, y: 0.20, confidence: 0.8 },
      { x: 0.52, y: 0.20, confidence: 0.8 },
      { x: 0.46, y: 0.22, confidence: 0.7 },
      { x: 0.54, y: 0.22, confidence: 0.7 },
      { x: 0.42, y: 0.32, confidence: 0.9 },
      { x: 0.58, y: 0.32, confidence: 0.9 },
      { x: 0.38, y: 0.45, confidence: 0.85 },
      { x: 0.62, y: 0.45, confidence: 0.85 },
      { x: 0.40, y: 0.55, confidence: 0.8 },
      { x: 0.60, y: 0.55, confidence: 0.8 },
      { x: 0.45, y: 0.55, confidence: 0.9 },
      { x: 0.55, y: 0.55, confidence: 0.9 },
      { x: 0.42, y: 0.58, confidence: 0.85 },
      { x: 0.58, y: 0.58, confidence: 0.85 },
      { x: 0.38, y: 0.90, confidence: 0.8 },
      { x: 0.62, y: 0.90, confidence: 0.8 }
    ];

    // Waving (left hand up, right hand at side)
    const waving = [
      { x: 0.50, y: 0.12, confidence: 0.9 },
      { x: 0.48, y: 0.10, confidence: 0.8 },
      { x: 0.52, y: 0.10, confidence: 0.8 },
      { x: 0.46, y: 0.12, confidence: 0.7 },
      { x: 0.54, y: 0.12, confidence: 0.7 },
      { x: 0.42, y: 0.22, confidence: 0.9 },
      { x: 0.58, y: 0.22, confidence: 0.9 },
      { x: 0.35, y: 0.12, confidence: 0.85 },
      { x: 0.62, y: 0.38, confidence: 0.85 },
      { x: 0.30, y: 0.04, confidence: 0.8 },
      { x: 0.64, y: 0.52, confidence: 0.8 },
      { x: 0.45, y: 0.50, confidence: 0.9 },
      { x: 0.55, y: 0.50, confidence: 0.9 },
      { x: 0.44, y: 0.70, confidence: 0.85 },
      { x: 0.56, y: 0.70, confidence: 0.85 },
      { x: 0.44, y: 0.90, confidence: 0.8 },
      { x: 0.56, y: 0.90, confidence: 0.8 }
    ];

    return [standing, walkLeft, standing, walkRight, armsUp, standing, sitting, standing, waving, standing];
  }

  // Generate a confidence heatmap from person positions
  generateConfidenceHeatmap(persons, cols, rows) {
    const roomW = this.roomWidth || 8;
    const roomD = this.roomDepth || 6;

    const positions = (persons || []).map(p => {
      // Prefer worldPosition if already computed, otherwise fall back to hips
      if (p.worldPosition) {
        return {
          x: p.worldPosition.x,
          z: p.worldPosition.z,
          confidence: p.confidence
        };
      }

      if (!p.keypoints || p.keypoints.length < 13) return null;
      const hipX = (p.keypoints[11].x + p.keypoints[12].x) / 2;
      const hipY = (p.keypoints[11].y + p.keypoints[12].y) / 2;
      return {
        x: (hipX - 0.5) * roomW,
        z: (hipY - 0.5) * roomD,
        confidence: p.confidence
      };
    }).filter(Boolean);

    const map = new Float32Array(cols * rows);
    const cellW = roomW / cols;
    const cellD = roomD / rows;

    for (const pos of positions) {
      for (let r = 0; r < rows; r++) {
        for (let c = 0; c < cols; c++) {
          const cx = (c + 0.5) * cellW - roomW / 2;
          const cz = (r + 0.5) * cellD - roomD / 2;
          const dx = cx - pos.x;
          const dz = cz - pos.z;
          const dist = Math.sqrt(dx * dx + dz * dz);
          const conf = Math.exp(-dist * dist * 0.5) * pos.confidence;
          map[r * cols + c] = Math.max(map[r * cols + c], conf);
        }
      }
    }

    return map;
  }

  // Classify a coarse posture label (Standing / Walking / Sitting / Unknown)
  // from normalized COCO keypoints. y=0 is top (head), y=1 is bottom (feet).
  _classifyPoseState(keypoints) {
    if (!keypoints || keypoints.length < 17) return 'Unknown';

    const kp = keypoints;
    const safe = (i) => kp[i] && (kp[i].confidence ?? 0) > 0.12 ? kp[i] : null;

    const ls = safe(5);
    const rs = safe(6);
    const lh = safe(11);
    const rh = safe(12);
    const lk = safe(13);
    const rk = safe(14);
    const la = safe(15);
    const ra = safe(16);

    if (!lh || !rh || !la || !ra) return 'Unknown';

    const hipY = (lh.y + rh.y) / 2;
    const ankleY = (la.y + ra.y) / 2;
    const kneeY = (lk && rk) ? (lk.y + rk.y) / 2 : hipY;
    const shoulderY = (ls && rs) ? (ls.y + rs.y) / 2 : hipY - 0.25;

    const stepWidth = Math.abs(la.x - ra.x);
    const torsoLen = Math.abs(shoulderY - hipY);
    const legLen = Math.abs(hipY - ankleY);

    // Sitting: torso long vs legs short in image, or hips low with knees bent
    const legsCompressed = legLen < 0.35 && kneeY > hipY - 0.05;
    const hipsLow = hipY > 0.55 && ankleY > 0.8;
    if (hipsLow && (legsCompressed || (torsoLen > 0.2 && legLen < torsoLen * 1.4))) {
      return 'Sitting';
    }

    // Walking: wide step, hips at standing height
    if (stepWidth > 0.1 && hipY < 0.58 && hipY > 0.35) {
      return 'Walking';
    }

    return 'Standing';
  }

  dispose() {
    this.demoPoses = [];
  }
}
