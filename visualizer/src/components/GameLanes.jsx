import React, { useMemo } from 'react';
import { LANE_COUNT, LANE_COLORS, LANE_HEIGHT, BEAT_LINE_POSITION } from './GameConstants';

const GameLanes = ({ gameData, currentTime, scrollSpeed, refreshRate, calculateHitWindow }) => {
  // Calculate dynamic pixel spacing based on BPM and scroll speed
  const calculatePixelSpacing = (bpm, height, speed) => {
    const beatsPerSecond = bpm / 60;
    return (height * speed) / beatsPerSecond;
  };

  // Dynamic pixel spacing calculation
  const pixelsBetweenBeats = useMemo(() => {
    const bpm = gameData?.metadata?.bpm || 120;
    return calculatePixelSpacing(bpm, LANE_HEIGHT, scrollSpeed);
  }, [gameData?.metadata?.bpm, scrollSpeed]);

  // Improved note speed calculation with refresh rate compensation
  const NOTE_SPEED = useMemo(() => {
    const bpm = gameData?.metadata?.bpm || 120;
    const baseSpeed = pixelsBetweenBeats * (bpm / 60);
    // Compensate for refresh rate to ensure smooth movement
    return baseSpeed * (60 / refreshRate);
  }, [gameData?.metadata?.bpm, pixelsBetweenBeats, refreshRate]);

  const calculateBeatPosition = (timestamp, currentTime, interpolationFactor = 0) => {
    // Calculate precise time difference with interpolation
    const timeOffset = (timestamp - currentTime) + (interpolationFactor / refreshRate);

    // Apply scroll speed and convert to pixels
    const position = timeOffset * NOTE_SPEED * scrollSpeed;

    // Calculate position with improved accuracy
    const exactPosition = BEAT_LINE_POSITION - position;

    // Ensure smooth movement by rounding to the nearest pixel
    return Math.round(exactPosition);
  };

  return (
    <div className="relative w-full h-96 bg-gray-900 rounded-lg overflow-hidden mb-4">
      {/* Lane grid */}
      <div className="absolute inset-0 flex">
        {Array.from({ length: LANE_COUNT }).map((_, laneIndex) => (
          <div
            key={laneIndex}
            className="flex-1 border-r border-gray-700 relative"
          >
            {/* Enhanced beat markers for this lane */}
            {gameData?.notes
              ?.filter((note) => note.lane === laneIndex)
              ?.map((note) => {
                const position = calculateBeatPosition(note.timestamp, currentTime);
                const hitWindow = calculateHitWindow(note.timestamp);
                const isInHitWindow = hitWindow !== 'miss';

                return (
                  <div
                    key={`${note.timestamp}-${note.lane}`}
                    className={`absolute left-1 right-1 h-8 rounded-lg transition-all
                      ${LANE_COLORS[note.lane]}
                      ${isInHitWindow ? 'scale-110' : ''}
                      opacity-90 shadow-lg border border-white/20`}
                    style={{
                      top: `${position}px`,
                      transform: `scale(${isInHitWindow ? 1.1 : 1})`,
                      boxShadow: isInHitWindow ? '0 0 12px rgba(255, 255, 255, 0.6)' : 'none'
                    }}
                  />
                );
              })}
          </div>
        ))}
      </div>

      {/* Beat line */}
      <div
        className="absolute left-0 right-0 h-1 bg-white"
        style={{ top: `${BEAT_LINE_POSITION}px` }}
      />

      {/* Lane numbers */}
      <div className="absolute bottom-0 left-0 right-0 flex text-white text-opacity-50">
        {Array.from({ length: LANE_COUNT }).map((_, index) => (
          <div key={index} className="flex-1 text-center pb-1">
            {index + 1}
          </div>
        ))}
      </div>
    </div>
  );
};

export default GameLanes;