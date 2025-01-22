import React from 'react';
import { LANE_COUNT, BEAT_LINE_POSITION } from './GameConstants';

const HitEffects = ({ hitEffects }) => {
  return (
    <>
      {hitEffects.map((effect) => (
        <div
          key={effect.id}
          className={`absolute h-12 transform -translate-y-1/2 pointer-events-none transition-opacity duration-150
            ${effect.type === 'perfect' ? 'bg-green-500' :
              effect.type === 'great' ? 'bg-blue-500' :
                'bg-yellow-500'} 
            opacity-40`}
          style={{
            top: `${BEAT_LINE_POSITION}px`,
            left: `${(effect.lane / LANE_COUNT) * 100}%`,
            width: `${100 / LANE_COUNT}%`
          }}
        />
      ))}
    </>
  );
};

export default HitEffects;