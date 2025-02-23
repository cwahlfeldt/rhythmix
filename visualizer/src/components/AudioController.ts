import { Audio } from 'expo-av';

export class AudioController {
    private sound: Audio.Sound;
    private startTime: number = 0;
    private baseLatency: number = 0.2; // Base system audio latency in seconds
    private lastKnownPosition: number = 0;
    private lastUpdateTime: number = 0;

    constructor(sound: Audio.Sound) {
        this.sound = sound;
    }

    async play() {
        this.startTime = performance.now() / 1000;
        await this.sound.playAsync();
        const status = await this.sound.getStatusAsync();
        this.lastKnownPosition = status.positionMillis / 1000;
        this.lastUpdateTime = performance.now() / 1000;
    }

    getCurrentTime(): number {
        // Calculate time since last known position
        const now = performance.now() / 1000;
        const timeSinceUpdate = now - this.lastUpdateTime;
        
        // Interpolate current position
        let interpolatedTime = this.lastKnownPosition + timeSinceUpdate;
        
        // Every second, sync with actual audio position to prevent drift
        if (timeSinceUpdate > 1.0) {
            this.sound.getStatusAsync().then(status => {
                this.lastKnownPosition = status.positionMillis / 1000;
                this.lastUpdateTime = now;
            });
        }
        
        return interpolatedTime - this.baseLatency;
    }

    setBaseLatency(latency: number) {
        this.baseLatency = latency;
    }
}