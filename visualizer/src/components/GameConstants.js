// Core game constants
export const BASE_SCROLL_SPEED = 1.0;
export const TIMING_WINDOWS = {
  PERFECT: 0.05,  // ±50ms
  GREAT: 0.10,    // ±100ms
  GOOD: 0.15      // ±150ms
};

// Lane configuration
export const LANE_COLORS = {
  0: 'bg-indigo-500 hover:bg-indigo-400',
  1: 'bg-emerald-500 hover:bg-emerald-400',
  2: 'bg-rose-500 hover:bg-rose-400'
};

// Visualization constants
export const LANE_HEIGHT = 500;
export const LANE_COUNT = 3;
export const BEAT_LINE_POSITION = Math.floor(LANE_HEIGHT / 2); // Center of screen
export const SPAWN_AHEAD_TIME = 3;
export const DESPAWN_AFTER_TIME = 6;

// Key mappings
export const KEY_TO_LANE = {
  'a': 0, 's': 1, 'd': 2,  // Left hand
  'j': 0, 'k': 1, 'l': 2   // Right hand
};

// Utility functions
export const calculatePixelSpacing = (bpm, height, speed) => {
  const beatsPerSecond = bpm / 60;
  return (height * speed) / beatsPerSecond;
};