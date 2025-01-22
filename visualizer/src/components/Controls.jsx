import React from 'react';

const Controls = ({ scrollSpeed, setScrollSpeed, isPlaying, handlePlayPause }) => {
  return (
    <>
      <div className="absolute top-4 right-4 flex items-center space-x-2">
        <label className="text-white text-sm">Scroll Speed: </label>
        <input
          type="range"
          min="0.5"
          max="2"
          step="0.1"
          value={scrollSpeed}
          onChange={(e) => setScrollSpeed(parseFloat(e.target.value))}
          className="w-32"
        />
        <span className="text-white text-sm">{scrollSpeed.toFixed(1)}x</span>
      </div>
      <div className="flex justify-between items-center mb-4">
        <h1 className="text-2xl font-bold text-white">
          Rhythmix Visualizer
        </h1>
        <button
          onClick={handlePlayPause}
          className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
        >
          {isPlaying ? "Pause" : "Play"}
        </button>
      </div>
    </>
  );
};

export default Controls;