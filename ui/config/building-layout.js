// Building / flat layout configuration for the 3D visualization.
// Defines a single-floor apartment-style space split into three rooms
// that align with the existing logical zones (zone_1, zone_2, zone_3).

export const BUILDING_LAYOUT = {
  // Overall flat envelope (meters)
  // Roughly a 2-bedroom apartment: 10m x 8m, 3m ceiling height.
  roomWidth: 10,
  roomDepth: 8,
  roomHeight: 3,

  // Rooms are rectangles defined in the same space as Environment:
  // origin at (0, 0) in the middle of the flat,
  // X left/right, Z forward/back (toward/away from camera).
  rooms: [
    // Living room (front-center)
    {
      id: 'zone_1',
      name: 'Living Room',
      center: [0, -1.5],
      width: 5.0,
      depth: 3.0,
      color: 0x145ca8
    },
    // Kitchen / dining (front-right)
    {
      id: 'zone_2',
      name: 'Kitchen / Dining',
      center: [2.75, -1.5],
      width: 4.5,
      depth: 3.0,
      color: 0x1a7f4b
    },
    // Hallway (center strip)
    {
      id: 'hallway',
      name: 'Hallway',
      center: [0, 0.5],
      width: 2.0,
      depth: 3.0,
      color: 0x444e68
    },
    // Bedroom 1 (back-left)
    {
      id: 'zone_3',
      name: 'Bedroom 1',
      center: [-2.5, 2.0],
      width: 4.0,
      depth: 3.0,
      color: 0x9b6b16
    },
    // Bedroom 2 (back-right)
    {
      id: 'bedroom_2',
      name: 'Bedroom 2',
      center: [2.5, 2.0],
      width: 4.0,
      depth: 3.0,
      color: 0x7b3f98
    },
    // Bathroom (off hallway)
    {
      id: 'bathroom',
      name: 'Bathroom',
      center: [-3.5, 0.5],
      width: 2.0,
      depth: 2.0,
      color: 0x3f3f46
    }
  ],

  // Static furniture / objects laid out inside the flat.
  // Positions are in world meters, same coordinates as rooms.
  furniture: [
    // Living room
    {
      id: 'sofa',
      name: 'Sofa',
      type: 'sofa',
      center: [-1.3, -2.3],
      width: 2.2,
      depth: 0.8,
      height: 0.8,
      color: 0x1e3a8a
    },
    {
      id: 'coffee_table',
      name: 'Coffee Table',
      type: 'table',
      center: [0.0, -2.0],
      width: 1.0,
      depth: 0.6,
      height: 0.4,
      color: 0x92400e
    },
    // Kitchen / dining
    {
      id: 'dining_table',
      name: 'Dining Table',
      type: 'table',
      center: [2.7, -1.4],
      width: 1.6,
      depth: 0.9,
      height: 0.75,
      color: 0x854d0e
    },
    // Bedroom 1
    {
      id: 'bedroom1_bed',
      name: 'Bed',
      type: 'bed',
      center: [-2.5, 2.2],
      width: 2.0,
      depth: 1.4,
      height: 0.7,
      color: 0x7c2d12
    },
    // Bedroom 2
    {
      id: 'bedroom2_bed',
      name: 'Bed',
      type: 'bed',
      center: [2.5, 2.2],
      width: 2.0,
      depth: 1.4,
      height: 0.7,
      color: 0x6d28d9
    },
    // Bathroom
    {
      id: 'bathtub',
      name: 'Bath',
      type: 'bath',
      center: [-3.5, 0.7],
      width: 1.6,
      depth: 0.7,
      height: 0.6,
      color: 0x0f172a
    }
  ]
};

