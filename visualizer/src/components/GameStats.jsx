import React from 'react';
import { calculatePixelSpacing, LANE_HEIGHT } from './GameConstants';

const GameStats = ({ gameData, currentTime, scrollSpeed, refreshRate, calibrationOffset }) => {
  if (!gameData) return null;

  let bpmSpeed = calculatePixelSpacing(gameData?.metadata?.bpm, LANE_HEIGHT, scrollSpeed);

  const stats = [
    { label: 'Time', value: `${currentTime.toFixed(3)}s` },
    { label: 'BPM', value: `${gameData?.metadata?.bpm}` },
    { label: 'BPM Speed', value: bpmSpeed },
    { label: 'Total Notes', value: gameData.notes.length },
    { label: 'Scroll Speed', value: `${scrollSpeed.toFixed(1)}x` },
    { label: 'Refresh Rate', value: `${refreshRate}Hz` },
    { label: 'Latency Offset', value: `${(calibrationOffset * 1000).toFixed(2)}ms` }
  ];

  return (
    <div className="text-sm text-gray-300 space-y-1">
      {stats.map(({ label, value }) => (
        <p key={label}>{label}: {value}</p>
      ))}
    </div>
  );
};

export default GameStats;