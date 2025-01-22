import React from 'react';
import { LANE_COUNT } from './GameConstants';

const HitEffects = ({ hitEffects }) => {
  const getEffectColor = (type) => {
    switch(type) {
      case 'perfect': return 'rgba(52, 211, 153, 0.2)'; // green
      case 'great': return 'rgba(96, 165, 250, 0.2)';   // blue
      case 'good': return 'rgba(251, 191, 36, 0.2)';    // yellow
      default: return 'rgba(255, 255, 255, 0.2)';
    }
  };

  return (
    <>
      {hitEffects.map((effect) => (
        <div
          key={effect.id}
          className="absolute inset-y-0 pointer-events-none transition-all duration-150"
          style={{
            left: `${(effect.lane / LANE_COUNT) * 100}%`,
            width: `${100 / LANE_COUNT}%`,
            background: getEffectColor(effect.type),
            animation: 'laneGlow 150ms ease-out'
          }}
        />
      ))}

      <style jsx>{`
        @keyframes laneGlow {
          0% { opacity: 1; }
          100% { opacity: 0; }
        }
      `}</style>
    </>
  );
};

export default HitEffects;