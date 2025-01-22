import { useState, useEffect, useRef, useMemo } from "react";

const RhythmVisualizer = () => {
  const [gameData, setGameData] = useState(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const audioRef = useRef(null);
  const animationFrameRef = useRef(null);
  const [visibleBeatMarkers, setVisibleBeatMarkers] = useState([]);

  // Constants for visualization
  const LANE_HEIGHT = 500;
  const LANE_COUNT = 3; // Number of lanes
  const BEAT_LINE_POSITION = LANE_HEIGHT - 100; // Position from top where beats should be hit
  const SPAWN_AHEAD_TIME = 3; // How many seconds ahead to spawn notes
  const DESPAWN_AFTER_TIME = 6; // How many seconds after beat line to keep notes
  // How much screen space between consecutive beats (at current BPM)
  const pixelsBetweenBeats = 84; // Visual distance between beats in pixels
  const NOTE_SPEED = useMemo(() => {
    const bpm = gameData?.metadata?.bpm || 175;
    // Convert BPM to pixels/second: (beats/min) * (pixels/beat) / (sec/min)
    return pixelsBetweenBeats * (bpm / 60);
  }, [gameData?.metadata?.bpm]);

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

  // Keep track of game state
  const gameStateRef = useRef({
    lastFrameTime: 0,
    lastCurrentTime: 0,
  });

  useEffect(() => {
    const updateGame = (timestamp) => {
      if (!gameData || !gameData.beat_markers) return;

      // const deltaTime = (timestamp - gameStateRef.current.lastFrameTime) / 1000;
      gameStateRef.current.lastFrameTime = timestamp;

      if (isPlaying && audioRef.current) {
        const audioTime = audioRef.current.currentTime;

        // Only update time if it has changed significantly
        if (Math.abs(audioTime - gameStateRef.current.lastCurrentTime) > 0.01) {
          setCurrentTime(audioTime);
          gameStateRef.current.lastCurrentTime = audioTime;
        }

        const currentBeats = gameData.beat_markers.filter(
          (beat) =>
            beat.timestamp >= audioTime - DESPAWN_AFTER_TIME &&
            beat.timestamp <= audioTime + SPAWN_AHEAD_TIME,
        );

        // Only update if we have different beats
        if (
          currentBeats.length !== visibleBeatMarkers.length ||
          currentBeats.some((beat, i) => beat !== visibleBeatMarkers[i])
        ) {
          setVisibleBeatMarkers(currentBeats);
        }
      }

      if (isPlaying) {
        animationFrameRef.current = requestAnimationFrame(updateGame);
      }
    };

    if (isPlaying) {
      animationFrameRef.current = requestAnimationFrame(updateGame);
    }

    return () => {
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, [isPlaying, gameData]);

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

  const calculateBeatPosition = (timestamp) => {
    // Time difference between current time and when this beat should be hit
    const timeOffset = timestamp - currentTime;

    // Convert time difference to pixels based on speed
    const position = timeOffset * NOTE_SPEED;

    // Position relative to beat line
    return BEAT_LINE_POSITION - position;
  };

  return (
    <div className="w-full max-w-4xl mx-auto p-4">
      <div className="bg-gray-800 rounded-lg p-4 shadow-lg">
        <div className="flex justify-between items-center mb-4">
          <h1 className="text-2xl font-bold text-white">
            Rhythm Game Visualizer
          </h1>
          <button
            onClick={handlePlayPause}
            className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
          >
            {isPlaying ? "Pause" : "Play"}
          </button>
        </div>

        <div className="relative w-full h-96 bg-gray-900 rounded-lg overflow-hidden mb-4">
          {/* Lane grid */}
          <div className="absolute inset-0 flex">
            {Array.from({ length: LANE_COUNT }).map((_, laneIndex) => (
              <div
                key={laneIndex}
                className="flex-1 border-r border-gray-700 relative"
              >
                {/* Beat markers for this lane */}
                {gameData?.notes
                  ?.filter((note) => note.lane === laneIndex)
                  ?.map((note) => (
                    <div
                      key={`${note.timestamp}-${note.lane}`}
                      className={`absolute left-1 right-1 h-4 rounded ${note.type === "tap" ? "bg-blue-500" : "bg-yellow-500"
                        } opacity-80 transition-transform duration-100`}
                      style={{
                        top: `${calculateBeatPosition(note.timestamp)}px`,
                      }}
                    />
                  ))}
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

        <audio
          ref={audioRef}
          src="/test.mp3"
          preload="auto"
          onEnded={() => setIsPlaying(false)}
        />

        {gameData && (
          <div className="text-sm text-gray-300 space-y-1">
            <p>Time: {currentTime.toFixed(2)}s</p>
            <p>Visible Beats: {visibleBeatMarkers.length}</p>
            <p>Total Beats: {gameData.beat_markers.length}</p>
          </div>
        )}
      </div>
    </div>
  );
};

export default RhythmVisualizer;
