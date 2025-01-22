import React, { useCallback, useEffect, useMemo } from "react";
import { TIMING_WINDOWS, KEY_TO_LANE } from './GameConstants';
import useGameState from './hooks/useGameState';
import useAudioHandler from './hooks/useAudioHandler';
import Controls from './Controls';
import GameLanes from './GameLanes';
import HitEffects from './HitEffects';
import ScoreBoard from './ScoreBoard';
import GameStats from './GameStats';

const RhythmVisualizer = () => {
  // Initialize game state and audio handler
  const {
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
    setCalibrationOffset
  } = useGameState();

  const { audioRef, handlePlayPause } = useAudioHandler(
    isPlaying,
    setIsPlaying,
    calibrationOffset,
    refreshRate,
    gameData,
    gameStateRef,
    setCurrentTime
  );

  // Calculate hit timing window for visual feedback
  const calculateHitWindow = useCallback((timestamp) => {
    const timeDiff = Math.abs(timestamp - currentTime);
    if (timeDiff <= TIMING_WINDOWS.PERFECT) return 'perfect';
    if (timeDiff <= TIMING_WINDOWS.GREAT) return 'great';
    if (timeDiff <= TIMING_WINDOWS.GOOD) return 'good';
    return 'miss';
  }, [currentTime]);

  // Handle keyboard input for note hits
  const handleKeyPress = useCallback((event) => {
    if (!isPlaying || !gameData?.notes) return;

    // Debounce key presses to prevent double hits
    const now = performance.now();
    if (now - lastHitTime < 50) return; // 50ms debounce

    const lane = KEY_TO_LANE[event.key.toLowerCase()];
    if (lane === undefined) return;

    setLastHitTime(now);

    const currentAudioTime = audioRef.current?.currentTime || 0;
    const compensatedTime = currentAudioTime + calibrationOffset;

    // Find the closest note in the lane within the hit window
    const availableNotes = gameData.notes
      .filter(note =>
        note.lane === lane &&
        !hitNotes.has(`${note.timestamp}-${note.lane}`) &&
        Math.abs(note.timestamp - compensatedTime) <= TIMING_WINDOWS.GOOD
      )
      .sort((a, b) =>
        Math.abs(a.timestamp - compensatedTime) - Math.abs(b.timestamp - compensatedTime)
      );

    if (availableNotes.length > 0) {
      const closestNote = availableNotes[0];
      const hitAccuracy = calculateHitWindow(closestNote.timestamp);

      if (hitAccuracy !== 'miss') {
        // Mark note as hit and update score
        const noteKey = `${closestNote.timestamp}-${closestNote.lane}`;
        setHitNotes(prev => new Set([...prev, noteKey]));
        updateScore(hitAccuracy);
        addHitEffect(hitAccuracy, lane);
      }
    }
  }, [isPlaying, gameData, calibrationOffset, hitNotes, lastHitTime, calculateHitWindow, addHitEffect, updateScore, audioRef]);

  // Add keyboard event listener
  useEffect(() => {
    window.addEventListener('keydown', handleKeyPress);
    return () => window.removeEventListener('keydown', handleKeyPress);
  }, [handleKeyPress]);

  // Auto-miss tracking for notes that pass the hit line
  useEffect(() => {
    if (!isPlaying || !gameData?.notes) return;

    const checkMissedNotes = () => {
      const currentAudioTime = audioRef.current?.currentTime || 0;
      const compensatedTime = currentAudioTime + calibrationOffset;

      gameData.notes.forEach(note => {
        const noteKey = `${note.timestamp}-${note.lane}`;
        if (!hitNotes.has(noteKey) && (note.timestamp + TIMING_WINDOWS.GOOD) < compensatedTime) {
          setHitNotes(prev => new Set([...prev, noteKey]));
          setScore(prev => ({
            ...prev,
            miss: prev.miss + 1
          }));

        }
      });
    };

    const missCheckInterval = setInterval(checkMissedNotes, 100);
    return () => clearInterval(missCheckInterval);
  }, [isPlaying, gameData, hitNotes, calibrationOffset]);


  // Screen refresh rate detection
  return (
    <div className="w-full max-w-4xl mx-auto p-4">
      <div className="bg-gray-800 rounded-lg p-4 shadow-lg relative">
        <Controls
          scrollSpeed={scrollSpeed}
          setScrollSpeed={setScrollSpeed}
          isPlaying={isPlaying}
          handlePlayPause={handlePlayPause}
        />

        <GameLanes
          gameData={gameData}
          currentTime={currentTime}
          scrollSpeed={scrollSpeed}
          refreshRate={refreshRate}
          calculateHitWindow={calculateHitWindow}
        />

        <audio
          ref={audioRef}
          src="/test.mp3"
          preload="auto"
          onEnded={() => setIsPlaying(false)}
        />

        <HitEffects hitEffects={hitEffects} />

        <GameStats
          gameData={gameData}
          currentTime={currentTime}
          scrollSpeed={scrollSpeed}
          refreshRate={refreshRate}
          calibrationOffset={calibrationOffset}
        />

        <ScoreBoard score={score} />

        {/* Control instructions */}
        <div className="mt-4 text-center text-gray-400 text-sm">
          <p>Controls: A/J = Lane 1, S/K = Lane 2, D/L = Lane 3</p>
        </div>
      </div>
    </div>
  );
};

export default RhythmVisualizer;
