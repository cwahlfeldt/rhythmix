export class BeatClock {
    private bpm: number;
    private secondsPerBeat: number;
    private microsPerBeat: number;

    constructor(bpm: number) {
        this.bpm = bpm;
        // Calculate with microsecond precision
        this.secondsPerBeat = (60.0 / bpm);
        this.microsPerBeat = Math.round(this.secondsPerBeat * 1_000_000) / 1_000_000;
    }

    getGridPosition(time: number): number {
        // Snap to nearest grid position with microsecond precision
        const beatPosition = time / this.microsPerBeat;
        return Math.round(beatPosition * 1_000_000) / 1_000_000;
    }

    getBeatTime(beatNumber: number): number {
        // Convert beat number to precise time
        return beatNumber * this.microsPerBeat;
    }
}