use std::time::Instant;

/// Local edits are compared with their baseline, independently of ECU persistence.
#[derive(Debug, Clone)]
pub(super) struct MapState {
    pub ram: Vec<i16>,
    pub eeprom: Vec<i16>,
    pub edited: Vec<i16>,
    baseline: Vec<i16>,
    pub last_read: Instant,
    pub read_error: Option<String>,
}

impl MapState {
    pub fn new(ram: Vec<i16>, eeprom: Vec<i16>) -> Self {
        Self {
            baseline: ram.clone(),
            edited: ram.clone(),
            ram,
            eeprom,
            last_read: Instant::now(),
            read_error: None,
        }
    }

    pub fn has_local_edits(&self) -> bool {
        self.edited != self.baseline
    }

    pub fn ram_differs_from_eeprom(&self) -> bool {
        self.ram != self.eeprom
    }

    pub fn discard_local_edits(&mut self) {
        self.edited = self.ram.clone();
        self.baseline = self.ram.clone();
    }

    pub fn confirm_ram_write(&mut self) -> bool {
        if self.ram != self.edited || self.read_error.is_some() {
            return false;
        }
        self.baseline = self.edited.clone();
        true
    }

    pub fn has_conflict(&self) -> bool {
        self.has_local_edits() && self.ram != self.baseline
    }

    pub fn keep_local_version(&mut self) {
        self.baseline = self.ram.clone();
    }

    pub fn apply_read(
        &mut self,
        result: Result<(Vec<i16>, Vec<i16>), String>,
    ) -> Result<(), String> {
        let result = result.and_then(|(ram, eeprom)| {
            if ram.len() != self.edited.len() || eeprom.len() != self.edited.len() {
                Err("Map dimensions changed; reopen the map.".into())
            } else {
                Ok((ram, eeprom))
            }
        });
        match result {
            Ok((ram, eeprom)) => {
                let preserve_edits = self.has_local_edits();
                self.ram = ram;
                self.eeprom = eeprom;
                if !preserve_edits {
                    self.discard_local_edits();
                }
                self.last_read = Instant::now();
                self.read_error = None;
                Ok(())
            }
            Err(error) => {
                self.read_error = Some(error.clone());
                Err(error)
            }
        }
    }

    pub fn cell_is_edited(&self, index: usize) -> bool {
        self.edited[index] != self.baseline[index]
    }
}

#[cfg(test)]
mod tests {
    use super::MapState;

    #[test]
    fn existing_ram_eeprom_difference_is_not_a_local_edit() {
        let state = MapState::new(vec![10], vec![5]);
        assert!(!state.has_local_edits());
        assert!(!state.cell_is_edited(0));
        assert!(state.ram_differs_from_eeprom());
    }

    #[test]
    fn editing_back_to_eeprom_is_still_a_local_edit() {
        let mut state = MapState::new(vec![10], vec![5]);
        state.edited[0] = 5;
        assert!(state.has_local_edits());
        assert!(state.cell_is_edited(0));
    }

    #[test]
    fn undo_changes_only_the_local_buffer() {
        let mut state = MapState::new(vec![10], vec![5]);
        state.edited[0] = 20;
        state.discard_local_edits();
        assert_eq!(state.edited, vec![10]);
        assert_eq!(state.ram, vec![10]);
        assert_eq!(state.eeprom, vec![5]);
        assert!(!state.has_local_edits());
    }

    #[test]
    fn ram_write_keeps_eeprom_snapshot_and_clears_local_edits() {
        let mut state = MapState::new(vec![10], vec![5]);
        state.edited[0] = 20;
        state.apply_read(Ok((vec![20], vec![5]))).unwrap();
        assert!(state.confirm_ram_write());
        assert_eq!(state.ram, vec![20]);
        assert_eq!(state.eeprom, vec![5]);
        assert!(!state.has_local_edits());
        assert!(state.ram_differs_from_eeprom());
    }

    #[test]
    fn parking_save_updates_eeprom_without_creating_local_edits() {
        let mut state = MapState::new(vec![10], vec![5]);
        state.apply_read(Ok((vec![10], vec![10]))).unwrap();
        assert!(!state.ram_differs_from_eeprom());
        assert!(!state.has_local_edits());
        assert_eq!(state.edited, vec![10]);
    }

    #[test]
    fn refresh_without_local_edits_adopts_current_ram() {
        let mut state = MapState::new(vec![10], vec![5]);
        state.apply_read(Ok((vec![12], vec![5]))).unwrap();
        assert_eq!(state.edited, vec![12]);
        assert!(!state.has_local_edits());
    }

    #[test]
    fn refresh_preserves_edits_and_detects_external_ram_change() {
        let mut state = MapState::new(vec![10], vec![5]);
        state.edited[0] = 20;
        state.apply_read(Ok((vec![12], vec![5]))).unwrap();
        assert_eq!(state.edited, vec![20]);
        assert!(state.has_local_edits());
        assert!(state.has_conflict());
        state.keep_local_version();
        assert!(!state.has_conflict());
        assert!(state.has_local_edits());
        state.apply_read(Ok((vec![13], vec![5]))).unwrap();
        assert!(state.has_conflict());
    }

    #[test]
    fn eeprom_save_during_edit_does_not_change_edit_baseline() {
        let mut state = MapState::new(vec![10], vec![5]);
        state.edited[0] = 20;
        state.apply_read(Ok((vec![10], vec![10]))).unwrap();
        assert_eq!(state.edited, vec![20]);
        assert!(state.has_local_edits());
        assert!(!state.has_conflict());
    }

    #[test]
    fn choosing_tcu_version_resolves_conflict_without_changing_tcu_snapshots() {
        let mut state = MapState::new(vec![10], vec![5]);
        state.edited[0] = 20;
        state.apply_read(Ok((vec![12], vec![5]))).unwrap();
        state.discard_local_edits();
        assert_eq!(state.edited, vec![12]);
        assert_eq!(state.ram, vec![12]);
        assert_eq!(state.eeprom, vec![5]);
        assert!(!state.has_local_edits());
        assert!(!state.has_conflict());
    }

    #[test]
    fn failed_or_invalid_read_keeps_snapshots_and_edits() {
        let mut state = MapState::new(vec![10], vec![5]);
        state.edited[0] = 20;
        let last_read = state.last_read;
        for result in [Err("Read timed out".into()), Ok((vec![], vec![5]))] {
            assert!(state.apply_read(result).is_err());
            assert_eq!(state.ram, vec![10]);
            assert_eq!(state.eeprom, vec![5]);
            assert_eq!(state.edited, vec![20]);
            assert_eq!(state.last_read, last_read);
            assert!(state.read_error.is_some());
        }
        state.apply_read(Ok((vec![10], vec![5]))).unwrap();
        assert!(state.read_error.is_none());
    }

    #[test]
    fn acknowledged_write_without_matching_readback_keeps_local_edits() {
        let mut state = MapState::new(vec![10], vec![5]);
        state.edited[0] = 20;
        state.apply_read(Ok((vec![12], vec![5]))).unwrap();
        assert!(!state.confirm_ram_write());
        assert_eq!(state.edited, vec![20]);
        assert!(state.has_local_edits());
        assert!(state.has_conflict());
    }

    #[test]
    fn failed_write_readback_preserves_edits_and_last_read() {
        let mut state = MapState::new(vec![10], vec![5]);
        state.edited[0] = 20;
        assert!(
            state
                .apply_read(Err("Read timed out after write".into()))
                .is_err()
        );
        assert!(!state.confirm_ram_write());
        assert!(state.has_local_edits());
        assert_eq!(state.edited, vec![20]);
        assert_eq!(state.ram, vec![10]);
    }
}
