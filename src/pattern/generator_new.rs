    fn generate_notes_with_features(&mut self, analysis: &AnalysisResults) -> Result<Vec<Note>> {
        let mut notes = Vec::new();
        let duration = analysis.onset_times.last().copied().unwrap_or(0.0);
        
        // Calculate quarter note duration (in seconds)
        let quarter_note = 60.0 / self.bpm;
        
        // Generate a note for each quarter note beat
        let mut current_time = 0.0;
        while current_time < duration {
            // Simple left-center-right-center pattern
            let lane = match (current_time / quarter_note).floor() as i32 % 4 {
                0 => 0,  // Beat 1: Left
                1 => 1,  // Beat 2: Center
                2 => 2,  // Beat 3: Right
                3 => 1,  // Beat 4: Center
                _ => 1,  // Shouldn't happen
            };
            
            notes.push(Note {
                timestamp: current_time,
                note_type: NoteType::Tap,
                lane: Lane::new(lane, 3)?,
                intensity: 1.0,
            });
            
            // Move to next quarter note
            current_time += quarter_note;
        }
        
        Ok(notes)
    }