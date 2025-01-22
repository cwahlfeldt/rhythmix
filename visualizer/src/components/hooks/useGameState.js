import { useState, useEffect, useRef } from 'react';
import { BASE_SCROLL_SPEED } from '../GameConstants';

export const useGameState = (initialRefreshRate = 60) => {
  const [gameData, setGameData] = useState(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [scrollSpeed, setScrollSpeed] = useState(BASE_SCROLL_SPEED);
  const [refreshRate, setRefreshRate] = useState(initialRefreshRate);
  const [hitEffects, setHitEffects] = useState([]);
  const [calibrationOffset, setCalibrationOffset] = useState(0);
  const [hitNotes, setHitNotes] = useState(new Set());
  const [score, setScore] = useState({ perfect: 0, great: 0, good: 0, miss: 0 });
  const [lastHitTime, setLastHitTime] = useState(0);

  // Track game state with enhanced timing
  const gameStateRef = useRef({
    lastFrameTime: 0,
    lastCurrentTime: 0,
    frameCount: 0
  });

  // Load game data
  useEffect(() => {
    const loadData = async () => {
      try {
        const response = await fetch("/test.json");
        const parsedData = await response.json();

        if (parsedData.success && parsedData.data) {
          setGameData(parsedData.data);
          console.log("Loaded beat data:", parsedData.data);
        }
      } catch (error) {
        console.error("Error loading data:", error);
      }
    };
    loadData();
  }, []);

  // Detect screen refresh rate
  useEffect(() => {
    const detectRefreshRate = () => {
      let startTime = performance.now();
      let frames = 0;

      const countFrames = (timestamp) => {
        frames++;
        const elapsed = timestamp - startTime;

        if (elapsed < 1000) {
          requestAnimationFrame(countFrames);
        } else {
          const fps = Math.round((frames * 1000) / elapsed);
          setRefreshRate(fps);
        }
      };

      requestAnimationFrame(countFrames);
    };

    detectRefreshRate();
  }, []);

  // Setup audio latency calibration
  useEffect(() => {
    const setupAudio = async () => {
      try {
        const audioContext = new (window.AudioContext || window.webkitAudioContext)();
        const latency = audioContext.baseLatency || 0;
        const estimatedLatency = latency + (1 / refreshRate); // Add one frame of latency
        setCalibrationOffset(estimatedLatency);
      } catch (error) {
        console.error('Audio context setup failed:', error);
        setCalibrationOffset(1 / refreshRate); // Fallback to one frame of latency
      }
    };

    setupAudio();
  }, [refreshRate]);

  const addHitEffect = (effectType, lane) => {
    const effectId = Date.now();
    setHitEffects(prev => [
      ...prev,
      { id: effectId, type: effectType, lane }
    ]);

    setTimeout(() => {
      setHitEffects(prev => prev.filter(effect => effect.id !== effectId));
    }, 160); // slightly longer than animation to ensure smooth removal
  };

  const updateScore = (hitAccuracy) => {
    setScore(prev => ({
      ...prev,
      [hitAccuracy]: prev[hitAccuracy] + 1
    }));
  };

  return {
    gameData,
    isPlaying,
    setIsPlaying,
    currentTime,
    setCurrentTime,
    scrollSpeed,
    setScrollSpeed,
    refreshRate,
    hitEffects,
    calibrationOffset,
    hitNotes,
    setHitNotes,
    score,
    lastHitTime,
    setLastHitTime,
    gameStateRef,
    addHitEffect,
    updateScore,
    setScore,
  };
};

export default useGameState;