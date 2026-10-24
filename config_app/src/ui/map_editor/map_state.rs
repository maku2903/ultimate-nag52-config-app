/// Local edits are compared with their baseline, independently of ECU persistence.
#[derive(Debug, Clone)]
pub(super) struct MapState {
    pub ram: Vec<i16>,
    pub eeprom: Vec<i16>,
    pub edited: Vec<i16>,
    baseline: Vec<i16>,
}

impl MapState {
    pub fn new(ram: Vec<i16>, eeprom: Vec<i16>) -> Self {
        Self {
            baseline: ram.clone(),
            edited: ram.clone(),
            ram,
            eeprom,
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

    pub fn ram_write_succeeded(&mut self) {
        self.ram = self.edited.clone();
        self.baseline = self.edited.clone();
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
        state.ram_write_succeeded();
        assert_eq!(state.ram, vec![20]);
        assert_eq!(state.eeprom, vec![5]);
        assert!(!state.has_local_edits());
        assert!(state.ram_differs_from_eeprom());
    }
}
