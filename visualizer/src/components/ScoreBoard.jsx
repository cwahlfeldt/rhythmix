import React from 'react';

const ScoreBoard = ({ score }) => {
  const scoreCategories = [
    { label: 'Perfect', value: score.perfect, bgColor: 'bg-green-500' },
    { label: 'Great', value: score.great, bgColor: 'bg-blue-500' },
    { label: 'Good', value: score.good, bgColor: 'bg-yellow-500' },
    { label: 'Miss', value: score.miss, bgColor: 'bg-red-500' }
  ];

  return (
    <div className="mt-4 grid grid-cols-4 gap-4 text-center">
      {scoreCategories.map(({ label, value, bgColor }) => (
        <div key={label} className={`${bgColor} rounded p-2`}>
          <div className="text-white font-bold">{label}</div>
          <div className="text-white">{value}</div>
        </div>
      ))}
    </div>
  );
};

export default ScoreBoard;