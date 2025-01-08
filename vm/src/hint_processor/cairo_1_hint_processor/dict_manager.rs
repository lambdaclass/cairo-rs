// Most of the structs and implementations of these Dictionaries are based on the `cairo-lang-runner` crate.
// Reference: https://github.com/starkware-libs/cairo/blob/main/crates/cairo-lang-runner/src/casm_run/dict_manager.rs

use num_traits::One;

use crate::stdlib::collections::HashMap;
use crate::stdlib::prelude::*;

use crate::types::relocatable::MaybeRelocatable;
use crate::vm::errors::hint_errors::HintError;
use crate::Felt252;
use crate::{types::relocatable::Relocatable, vm::vm_core::VirtualMachine};

/// Stores the data of a specific dictionary.
pub struct DictTrackerExecScope {
    /// The data of the dictionary.
    data: HashMap<Felt252, MaybeRelocatable>,
    /// The start of the segment of the dictionary.
    start: Relocatable,
    /// The end of this segment, if finalized.
    end: Option<Relocatable>,
}

/// Helper object to allocate, track and destruct all dictionaries in the run.
#[derive(Default)]
pub struct DictManagerExecScope {
    /// Maps between a segment index and the DictTrackerExecScope associated with it.
    segment_to_tracker: HashMap<isize, usize>,
    /// The actual trackers of the dictionaries, in the order of allocation.
    trackers: Vec<DictTrackerExecScope>,
    // If set to true, dictionaries will be created on temporary segments which can then be relocated into a single segment by the end of the run
    // If set to false, each dictionary will use a single real segment
    use_temporary_segments: bool,
}

impl DictTrackerExecScope {
    /// Creates a new tracker starting at `start`.
    pub fn new(start: Relocatable) -> Self {
        Self {
            data: HashMap::default(),
            start,
            end: None,
        }
    }
}

impl DictManagerExecScope {
    pub const DICT_DEFAULT_VALUE: usize = 0;

    // Creates a new DictManagerExecScope
    pub fn new(use_temporary_segments: bool) -> Self {
        Self {
            use_temporary_segments,
            ..Default::default()
        }
    }

    /// Allocates a new segment for a new dictionary and return the start of the segment.
    pub fn new_default_dict(&mut self, vm: &mut VirtualMachine) -> Result<Relocatable, HintError> {
        let dict_segment = if self.use_temporary_segments && self.trackers.len() > 0 {
            vm.add_temporary_segment()
        } else {
            vm.add_memory_segment()
        };
        let tracker = DictTrackerExecScope::new(dict_segment);
        if self
            .segment_to_tracker
            .insert(dict_segment.segment_index, self.trackers.len())
            .is_some()
        {
            return Err(HintError::CantCreateDictionaryOnTakenSegment(
                dict_segment.segment_index,
            ));
        }

        self.trackers.push(tracker);
        Ok(dict_segment)
    }

    /// Returns a mut reference for a dict tracker corresponding to a given pointer to a dict
    /// segment.
    fn get_dict_tracker_mut(&mut self, dict_end: Relocatable) -> &mut DictTrackerExecScope {
        let idx = self
            .get_dict_infos_index(dict_end)
            .expect("The given value does not point to a known dictionary.");
        &mut self.trackers[idx]
    }

    /// Returns the index of the dict tracker corresponding to a given pointer to a dict segment.
    pub fn get_dict_infos_index(&self, dict_end: Relocatable) -> Result<usize, HintError> {
        Ok(*self
            .segment_to_tracker
            .get(&dict_end.segment_index)
            .ok_or_else(|| {
                HintError::CustomHint(
                    "The given value does not point to a known dictionary."
                        .to_string()
                        .into_boxed_str(),
                )
            })?)
    }

    /// Finalizes a segment of a dictionary.
    /// Does nothing if use_temporary_segments is set to false
    pub fn finalize_segment(&mut self, dict_end: Relocatable) -> Result<(), HintError> {
        if self.use_temporary_segments {
            let tracker_idx = self.get_dict_infos_index(dict_end)?;
            let tracker = &mut self.trackers[tracker_idx];
            if let Some(prev) = tracker.end {
                return Err(HintError::CustomHint(
                    format!(
                        "The segment is already finalized. \
                    Attempting to override next start {prev}, with: {dict_end}.",
                    )
                    .into_boxed_str(),
                ));
            }
            tracker.end = Some(dict_end);
        }
        Ok(())
    }

    /// Relocates all dictionaries into a single segment
    /// Does nothing if use_temporary_segments is set to false
    pub fn relocate_all_dictionaries(&mut self, vm: &mut VirtualMachine) -> Result<(), HintError> {
        // We expect the first segment to be a normal one, which doesn't require relocation. So
        // there is nothing to do unless there are at least two segments.
        if self.use_temporary_segments && self.trackers.len() > 1 {
            let first_segment = self.trackers.get(0).unwrap();
            if first_segment.start.segment_index < 0 {
                return Err(HintError::CustomHint(
                    "First dict segment should not be temporary"
                        .to_string()
                        .into_boxed_str(),
                ));
            }
            let mut prev_end = first_segment.end.unwrap_or_default();
            for tracker in &self.trackers[1..] {
                if tracker.start.segment_index >= 0 {
                    return Err(HintError::CustomHint(
                        "Dict segment should be temporary"
                            .to_string()
                            .into_boxed_str(),
                    ));
                }
                vm.add_relocation_rule(tracker.start, prev_end.into())?;
                prev_end += (tracker.end.unwrap_or_default() - tracker.start)?;
                prev_end += 1;
            }
        }
        Ok(())
    }

    /// Inserts a value to the dict tracker corresponding to a given pointer to a dict segment.
    pub fn insert_to_tracker(
        &mut self,
        dict_end: Relocatable,
        key: Felt252,
        value: MaybeRelocatable,
    ) {
        self.get_dict_tracker_mut(dict_end).data.insert(key, value);
    }

    /// Gets a value from the dict tracker corresponding to a given pointer to a dict segment.
    /// None if the key does not exist in the tracker data.
    pub fn get_from_tracker(
        &self,
        dict_end: Relocatable,
        key: &Felt252,
    ) -> Option<MaybeRelocatable> {
        self.trackers[self.get_dict_infos_index(dict_end).ok()?]
            .data
            .get(key)
            .cloned()
    }
}

/// Helper object for the management of dict_squash hints.
#[derive(Default, Debug)]
pub struct DictSquashExecScope {
    /// A map from key to the list of indices accessing it, each list in reverse order.
    pub(crate) access_indices: HashMap<Felt252, Vec<Felt252>>,
    /// Descending list of keys.
    pub(crate) keys: Vec<Felt252>,
}

impl DictSquashExecScope {
    /// Returns the current key to process.
    pub fn current_key(&self) -> Option<Felt252> {
        self.keys.last().cloned()
    }

    /// Removes the current key, and its access indices. Should be called when only the
    /// last key access is in the corresponding indices list.
    pub fn pop_current_key(&mut self) -> Result<(), HintError> {
        let current_key = self.current_key().ok_or_else(|| {
            HintError::CustomHint("Failed to get current key".to_string().into_boxed_str())
        })?;
        let key_accesses = self.access_indices.remove(&current_key).ok_or_else(|| {
            HintError::CustomHint(format!("No key accesses for key {current_key}").into_boxed_str())
        })?;
        if !key_accesses.len().is_one() {
            return Err(HintError::CustomHint(
                "Key popped but not all accesses were processed."
                    .to_string()
                    .into_boxed_str(),
            ));
        }
        self.keys.pop();
        Ok(())
    }

    /// Returns a reference to the access indices list of the current key.
    pub fn current_access_indices(&mut self) -> Option<&mut Vec<Felt252>> {
        let current_key = self.current_key()?;
        self.access_indices.get_mut(&current_key)
    }

    /// Returns a reference to the last index in the current access indices list.
    pub fn current_access_index(&mut self) -> Option<&Felt252> {
        self.current_access_indices()?.last()
    }

    /// Returns and removes the current access index.
    pub fn pop_current_access_index(&mut self) -> Option<Felt252> {
        self.current_access_indices()?.pop()
    }
}
