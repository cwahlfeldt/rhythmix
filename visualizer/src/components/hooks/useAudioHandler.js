import { useRef, useEffect } from 'react';

export const useAudioHandler = (isPlaying, setIsPlaying, calibrationOffset, refreshRate, gameData, gameStateRef, setCurrentTime) => {
  const audioRef = useRef(null);
  const animationFrameRef = useRef(null);

  useEffect(() => {
    const updateGame = (timestamp) => {
      if (!gameData || !gameData.notes) return;

      const deltaTime = (timestamp - gameStateRef.current.lastFrameTime) / 1000;
      gameStateRef.current.lastFrameTime = timestamp;
      gameStateRef.current.frameCount++;

      if (isPlaying && audioRef.current) {
        const audioTime = audioRef.current.currentTime;
        const compensatedTime = audioTime + calibrationOffset;

        // Update timing with improved precision
        if (Math.abs(compensatedTime - gameStateRef.current.lastCurrentTime) > (1 / refreshRate)) {
          setCurrentTime(compensatedTime);
          gameStateRef.current.lastCurrentTime = compensatedTime;
        }
      }

      if (isPlaying) {
        animationFrameRef.current = requestAnimationFrame(updateGame);
      }
    };

    if (isPlaying) {
      gameStateRef.current.frameCount = 0;
      animationFrameRef.current = requestAnimationFrame(updateGame);
    }

    return () => {
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, [isPlaying, gameData, refreshRate, calibrationOffset, setCurrentTime]);

  const handlePlayPause = () => {
    if (audioRef.current) {
      if (isPlaying) {
        audioRef.current.pause();
      } else {
        audioRef.current.play();
      }
      setIsPlaying(!isPlaying);
    }
  };

  return {
    audioRef,
    handlePlayPause
  };
};

export default useAudioHandler;