use std::collections::HashMap;

use num_bigint::BigInt;

use crate::{
    serde::deserialize_program::ApTracking,
    types::exec_scope::PyValueType,
    vm::{errors::vm_errors::VirtualMachineError, vm_core::VirtualMachine},
};

use super::hint_utils::{
    get_integer_from_var_name, get_ptr_from_var_name, insert_value_from_var_name,
};
//DictAccess struct has three memebers, so the size of DictAccess* is 3
pub const DICT_ACCESS_SIZE: usize = 3;

fn copy_initial_dict(vm: &mut VirtualMachine) -> Option<HashMap<BigInt, BigInt>> {
    let mut initial_dict: Option<HashMap<BigInt, BigInt>> = None;
    if let Some(variables) = vm.exec_scopes.get_local_variables() {
        if let Some(PyValueType::Dictionary(py_initial_dict)) = variables.get("initial_dict") {
            initial_dict = Some(py_initial_dict.clone());
        }
    }
    initial_dict
}

/*Implements hint:
   if '__dict_manager' not in globals():
           from starkware.cairo.common.dict import DictManager
           __dict_manager = DictManager()

       memory[ap] = __dict_manager.new_dict(segments, initial_dict)
       del initial_dict

For now, the functionality to create a dictionary from a previously defined initial_dict (using a hint)
is not available
*/
pub fn dict_new(vm: &mut VirtualMachine) -> Result<(), VirtualMachineError> {
    //Get initial dictionary from scope (defined by an earlier hint)
    let initial_dict = copy_initial_dict(vm).ok_or(VirtualMachineError::NoInitialDict)?;
    let base = vm
        .dict_manager
        .new_dict(&mut vm.segments, &mut vm.memory, initial_dict)?;
    vm.memory
        .insert(&vm.run_context.ap, &base)
        .map_err(VirtualMachineError::MemoryError)
}

/*Implements hint:
   if '__dict_manager' not in globals():
            from starkware.cairo.common.dict import DictManager
            __dict_manager = DictManager()

        memory[ap] = __dict_manager.new_default_dict(segments, ids.default_value)

For now, the functionality to create a dictionary from a previously defined initial_dict (using a hint)
is not available, an empty dict is created always
*/
pub fn default_dict_new(
    vm: &mut VirtualMachine,
    ids: &HashMap<String, BigInt>,
    hint_ap_tracking: Option<&ApTracking>,
) -> Result<(), VirtualMachineError> {
    //Check that ids contains the reference id for each variable used by the hint
    let default_value = get_integer_from_var_name(
        "default_value",
        ids,
        &vm.memory,
        &vm.references,
        &vm.run_context,
        hint_ap_tracking,
    )?
    .clone();
    //Get initial dictionary from scope (defined by an earlier hint) if available
    let initial_dict = copy_initial_dict(vm);

    let base = vm.dict_manager.new_default_dict(
        &mut vm.segments,
        &mut vm.memory,
        &default_value,
        initial_dict,
    )?;
    vm.memory
        .insert(&vm.run_context.ap, &base)
        .map_err(VirtualMachineError::MemoryError)
}

/* Implements hint:
   dict_tracker = __dict_manager.get_tracker(ids.dict_ptr)
   dict_tracker.current_ptr += ids.DictAccess.SIZE
   ids.value = dict_tracker.data[ids.key]
*/
pub fn dict_read(
    vm: &mut VirtualMachine,
    ids: &HashMap<String, BigInt>,
    hint_ap_tracking: Option<&ApTracking>,
) -> Result<(), VirtualMachineError> {
    let key = get_integer_from_var_name(
        "key",
        ids,
        &vm.memory,
        &vm.references,
        &vm.run_context,
        hint_ap_tracking,
    )?;
    let dict_ptr = get_ptr_from_var_name(
        "dict_ptr",
        ids,
        &vm.memory,
        &vm.references,
        &vm.run_context,
        hint_ap_tracking,
    )?;
    let tracker = vm.dict_manager.get_tracker(&dict_ptr)?;
    tracker.current_ptr.offset += DICT_ACCESS_SIZE;
    let value = tracker.get_value(key)?;
    insert_value_from_var_name(
        "value",
        value.clone(),
        ids,
        &mut vm.memory,
        &vm.references,
        &vm.run_context,
        hint_ap_tracking,
    )
}

/* Implements hint:
    dict_tracker = __dict_manager.get_tracker(ids.dict_ptr)
    dict_tracker.current_ptr += ids.DictAccess.SIZE
    ids.dict_ptr.prev_value = dict_tracker.data[ids.key]
    dict_tracker.data[ids.key] = ids.new_value
*/
pub fn dict_write(
    vm: &mut VirtualMachine,
    ids: &HashMap<String, BigInt>,
    hint_ap_tracking: Option<&ApTracking>,
) -> Result<(), VirtualMachineError> {
    let key = get_integer_from_var_name(
        "key",
        ids,
        &vm.memory,
        &vm.references,
        &vm.run_context,
        hint_ap_tracking,
    )?;
    let new_value = get_integer_from_var_name(
        "new_value",
        ids,
        &vm.memory,
        &vm.references,
        &vm.run_context,
        hint_ap_tracking,
    )?;
    let dict_ptr = get_ptr_from_var_name(
        "dict_ptr",
        ids,
        &vm.memory,
        &vm.references,
        &vm.run_context,
        hint_ap_tracking,
    )?;
    //Get tracker for dictionary
    let tracker = vm.dict_manager.get_tracker(&dict_ptr)?;
    //dict_ptr is a pointer to a struct, with the ordered fields (key, prev_value, new_value),
    //dict_ptr.prev_value will be equal to dict_ptr + 1
    let dict_ptr_prev_value = dict_ptr + 1;
    //Tracker set to track next dictionary entry
    tracker.current_ptr.offset += DICT_ACCESS_SIZE;
    //Get previous value
    let prev_value = tracker.get_value(key)?.clone();
    //Insert new value into tracker
    tracker.insert_value(key, new_value);
    //Insert previous value into dict_ptr.prev_value
    //Addres for dict_ptr.prev_value should be dict_ptr* + 1 (defined above)
    vm.memory.insert_value(&dict_ptr_prev_value, prev_value)?;
    Ok(())
}

/* Implements hint:
    # Verify dict pointer and prev value.
        dict_tracker = __dict_manager.get_tracker(ids.dict_ptr)
        current_value = dict_tracker.data[ids.key]
        assert current_value == ids.prev_value, \
            f'Wrong previous value in dict. Got {ids.prev_value}, expected {current_value}.'

        # Update value.
        dict_tracker.data[ids.key] = ids.new_value
        dict_tracker.current_ptr += ids.DictAccess.SIZE
*/
pub fn dict_update(
    vm: &mut VirtualMachine,
    ids: &HashMap<String, BigInt>,
    hint_ap_tracking: Option<&ApTracking>,
) -> Result<(), VirtualMachineError> {
    let key = get_integer_from_var_name(
        "key",
        ids,
        &vm.memory,
        &vm.references,
        &vm.run_context,
        hint_ap_tracking,
    )?;
    let prev_value = get_integer_from_var_name(
        "prev_value",
        ids,
        &vm.memory,
        &vm.references,
        &vm.run_context,
        hint_ap_tracking,
    )?;
    let new_value = get_integer_from_var_name(
        "new_value",
        ids,
        &vm.memory,
        &vm.references,
        &vm.run_context,
        hint_ap_tracking,
    )?;
    let dict_ptr = get_ptr_from_var_name(
        "dict_ptr",
        ids,
        &vm.memory,
        &vm.references,
        &vm.run_context,
        hint_ap_tracking,
    )?;

    //Get tracker for dictionary
    let tracker = vm.dict_manager.get_tracker(&dict_ptr)?;
    //Check that prev_value is equal to the current value at the given key
    let current_value = tracker.get_value(key)?;
    if current_value != prev_value {
        return Err(VirtualMachineError::WrongPrevValue(
            prev_value.clone(),
            current_value.clone(),
            key.clone(),
        ));
    }
    //Update Value
    tracker.insert_value(key, new_value);
    tracker.current_ptr.offset += DICT_ACCESS_SIZE;
    Ok(())
}

/* Implements hint:
   # Prepare arguments for dict_new. In particular, the same dictionary values should be copied
   # to the new (squashed) dictionary.
   vm_enter_scope({
       # Make __dict_manager accessible.
       '__dict_manager': __dict_manager,
       # Create a copy of the dict, in case it changes in the future.
       'initial_dict': dict(__dict_manager.get_dict(ids.dict_accesses_end)),
   })
*/
pub fn dict_squash_copy_dict(
    vm: &mut VirtualMachine,
    ids: &HashMap<String, BigInt>,
    hint_ap_tracking: Option<&ApTracking>,
) -> Result<(), VirtualMachineError> {
    let dict_accesses_end = get_ptr_from_var_name(
        "dict_accesses_end",
        ids,
        &vm.memory,
        &vm.references,
        &vm.run_context,
        hint_ap_tracking,
    )?;
    let dict_copy = vm
        .dict_manager
        .get_tracker(&dict_accesses_end)?
        .get_dictionary_copy();

    vm.exec_scopes.enter_scope(HashMap::from([(
        String::from("initial_dict"),
        PyValueType::Dictionary(dict_copy),
    )]));
    Ok(())
}

/* Implements Hint:
    # Update the DictTracker's current_ptr to point to the end of the squashed dict.
    __dict_manager.get_tracker(ids.squashed_dict_start).current_ptr = \
    ids.squashed_dict_end.address_
*/
pub fn dict_squash_update_ptr(
    vm: &mut VirtualMachine,
    ids: &HashMap<String, BigInt>,
    hint_ap_tracking: Option<&ApTracking>,
) -> Result<(), VirtualMachineError> {
    let squashed_dict_start = get_ptr_from_var_name(
        "squashed_dict_start",
        ids,
        &vm.memory,
        &vm.references,
        &vm.run_context,
        hint_ap_tracking,
    )?;
    let squashed_dict_end = get_ptr_from_var_name(
        "squashed_dict_end",
        ids,
        &vm.memory,
        &vm.references,
        &vm.run_context,
        hint_ap_tracking,
    )?;
    vm.dict_manager
        .get_tracker(&squashed_dict_start)?
        .current_ptr = squashed_dict_end;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use num_bigint::{BigInt, Sign};

    use crate::types::instruction::Register;
    use crate::types::relocatable::Relocatable;
    use crate::vm::errors::memory_errors::MemoryError;
    use crate::vm::hints::dict_manager::{DictManager, Dictionary};
    use crate::vm::hints::execute_hint::HintReference;
    use crate::{bigint, relocatable};
    use crate::{
        types::relocatable::MaybeRelocatable,
        vm::hints::{dict_manager::DictTracker, execute_hint::execute_hint},
    };

    use super::*;
    #[test]
    fn run_dict_new_with_initial_dict_empty() {
        let hint_code = "if '__dict_manager' not in globals():\n    from starkware.cairo.common.dict import DictManager\n    __dict_manager = DictManager()\n\nmemory[ap] = __dict_manager.new_dict(segments, initial_dict)\ndel initial_dict".as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            //ap value is (0,0)
            Vec::new(),
            false,
        );
        //Store initial dict in scope
        vm.exec_scopes
            .assign_or_update_variable("initial_dict", PyValueType::Dictionary(HashMap::new()));
        //ids and references are not needed for this test
        execute_hint(&mut vm, hint_code, HashMap::new(), &ApTracking::new())
            .expect("Error while executing hint");
        //first new segment is added for the dictionary
        assert_eq!(vm.segments.num_segments, 1);
        //new segment base (0,0) is inserted into ap (0,0)
        assert_eq!(
            vm.memory.get(&MaybeRelocatable::from((0, 0))),
            Ok(Some(&MaybeRelocatable::from((0, 0))))
        );
        //Check the dict manager has a tracker for segment 0,
        //and that tracker contains the ptr (0,0) and an empty dict
        assert_eq!(
            vm.dict_manager.trackers.get(&0),
            Some(&DictTracker::new_empty(&relocatable!(0, 0)))
        );
    }

    #[test]
    fn run_dict_new_with_no_initial_dict() {
        let hint_code = "if '__dict_manager' not in globals():\n    from starkware.cairo.common.dict import DictManager\n    __dict_manager = DictManager()\n\nmemory[ap] = __dict_manager.new_dict(segments, initial_dict)\ndel initial_dict".as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            //ap value is (0,0)
            Vec::new(),
            false,
        );
        //ids and references are not needed for this test
        assert_eq!(
            execute_hint(&mut vm, hint_code, HashMap::new(), &ApTracking::new()),
            Err(VirtualMachineError::NoInitialDict)
        );
    }

    #[test]
    fn run_dict_new_ap_is_taken() {
        let hint_code = "if '__dict_manager' not in globals():\n    from starkware.cairo.common.dict import DictManager\n    __dict_manager = DictManager()\n\nmemory[ap] = __dict_manager.new_dict(segments, initial_dict)\ndel initial_dict".as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            //ap value is (0,0)
            Vec::new(),
            false,
        );
        vm.segments.add(&mut vm.memory, None);
        vm.exec_scopes
            .assign_or_update_variable("initial_dict", PyValueType::Dictionary(HashMap::new()));
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(1)),
            )
            .unwrap();
        //ids and references are not needed for this test
        assert_eq!(
            execute_hint(&mut vm, hint_code, HashMap::new(), &ApTracking::new()),
            Err(VirtualMachineError::MemoryError(
                MemoryError::InconsistentMemory(
                    MaybeRelocatable::from((0, 0)),
                    MaybeRelocatable::from(bigint!(1)),
                    MaybeRelocatable::from((1, 0))
                )
            ))
        );
    }

    #[test]
    fn run_dict_read_valid() {
        let hint_code = "dict_tracker = __dict_manager.get_tracker(ids.dict_ptr)\ndict_tracker.current_ptr += ids.DictAccess.SIZE\nids.value = dict_tracker.data[ids.key]"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 3));
        //Create tracker
        let mut tracker = DictTracker::new_empty(&relocatable!(1, 0));
        tracker.insert_value(&bigint!(5_i32), &bigint!(12_i32));
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //Insert ids into memory
        //ids.key
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(5)),
            )
            .unwrap();
        //ids.value
        //ids.dict_ptr
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 2)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("key"), bigint!(0));
        ids.insert(String::from("value"), bigint!(1));
        ids.insert(String::from("dict_ptr"), bigint!(2));
        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -3,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                2,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Ok(())
        );
        //Check that value variable (at address (0,1)) contains the proper value
        assert_eq!(
            vm.memory.get(&MaybeRelocatable::from((0, 1))),
            Ok(Some(&MaybeRelocatable::from(bigint!(12))))
        );
        //Check that the tracker's current_ptr has moved accordingly
        assert_eq!(
            vm.dict_manager.trackers.get(&1).unwrap().current_ptr,
            relocatable!(1, 3)
        );
    }

    #[test]
    fn run_dict_read_invalid_key() {
        let hint_code = "dict_tracker = __dict_manager.get_tracker(ids.dict_ptr)\ndict_tracker.current_ptr += ids.DictAccess.SIZE\nids.value = dict_tracker.data[ids.key]"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 3));
        //Initialize dictionary
        let mut dictionary = HashMap::<BigInt, BigInt>::new();
        dictionary.insert(bigint!(5), bigint!(12));
        //Create tracker
        let mut tracker = DictTracker::new_empty(&relocatable!(1, 0));
        tracker.data = Dictionary::SimpleDictionary(dictionary);
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //Insert ids into memory
        //ids.key
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(6)),
            )
            .unwrap();
        //ids.value
        //ids.dict_ptr
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 2)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("key"), bigint!(0));
        ids.insert(String::from("value"), bigint!(1));
        ids.insert(String::from("dict_ptr"), bigint!(2));
        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -3,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                2,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Err(VirtualMachineError::NoValueForKey(bigint!(6)))
        );
    }
    #[test]
    fn run_dict_read_no_tracker() {
        let hint_code = "dict_tracker = __dict_manager.get_tracker(ids.dict_ptr)\ndict_tracker.current_ptr += ids.DictAccess.SIZE\nids.value = dict_tracker.data[ids.key]"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 3));
        //Create manager
        let dict_manager = DictManager::new();
        vm.dict_manager = dict_manager;
        //Insert ids into memory
        //ids.key
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(6)),
            )
            .unwrap();
        //ids.value
        //ids.dict_ptr
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 2)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("key"), bigint!(0));
        ids.insert(String::from("value"), bigint!(1));
        ids.insert(String::from("dict_ptr"), bigint!(2));
        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -3,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                2,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Err(VirtualMachineError::NoDictTracker(1))
        );
    }

    #[test]
    fn run_default_dict_new_valid() {
        let hint_code = "if '__dict_manager' not in globals():\n    from starkware.cairo.common.dict import DictManager\n    __dict_manager = DictManager()\n\nmemory[ap] = __dict_manager.new_default_dict(segments, ids.default_value)".as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            //ap value is (0,0)
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((1, 1));
        //insert ids.default_value into memory
        vm.memory
            .insert(
                &MaybeRelocatable::from((1, 0)),
                &MaybeRelocatable::from(bigint!(17)),
            )
            .unwrap();
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("default_value"), bigint!(0));
        //Create references
        vm.references = HashMap::from([(
            0,
            HintReference {
                register: Register::FP,
                offset1: -1,
                offset2: 0,
                inner_dereference: false,
                ap_tracking_data: None,
                immediate: None,
            },
        )]);
        execute_hint(&mut vm, hint_code, ids, &ApTracking::new())
            .expect("Error while executing hint");
        //third new segment is added for the dictionary
        assert_eq!(vm.segments.num_segments, 3);
        //new segment base (2,0) is inserted into ap (0,0)
        assert_eq!(
            vm.memory.get(&MaybeRelocatable::from((0, 0))),
            Ok(Some(&MaybeRelocatable::from((2, 0))))
        );
        //Check the dict manager has a tracker for segment 2,
        //and that tracker contains the ptr (2,0) and an empty dict
        assert_eq!(
            vm.dict_manager.trackers.get(&2),
            Some(&DictTracker::new_default_dict(
                &relocatable!(2, 0),
                &bigint!(17),
                None
            ))
        );
    }

    #[test]
    fn run_default_dict_new_no_default_value() {
        let hint_code = "if '__dict_manager' not in globals():\n    from starkware.cairo.common.dict import DictManager\n    __dict_manager = DictManager()\n\nmemory[ap] = __dict_manager.new_default_dict(segments, ids.default_value)".as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            //ap value is (0,0)
            Vec::new(),
            false,
        );
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 1));
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("default_value"), bigint!(0));
        //Create references
        vm.references = HashMap::from([(
            0,
            HintReference {
                register: Register::FP,
                offset1: -1,
                offset2: 0,
                inner_dereference: false,
                ap_tracking_data: None,
                immediate: None,
            },
        )]);
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Err(VirtualMachineError::ExpectedInteger(
                MaybeRelocatable::from((0, 0))
            ))
        );
    }

    #[test]
    fn run_dict_write_default_valid_empty_dict() {
        let hint_code = "dict_tracker = __dict_manager.get_tracker(ids.dict_ptr)\ndict_tracker.current_ptr += ids.DictAccess.SIZE\nids.dict_ptr.prev_value = dict_tracker.data[ids.key]\ndict_tracker.data[ids.key] = ids.new_value"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 3));
        //Create tracker
        //current_ptr = dict_ptr = (1, 0)
        let tracker = DictTracker::new_default_dict(&relocatable!(1, 0), &bigint!(2), None);
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //Insert ids into memory
        //ids.key
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(5)),
            )
            .unwrap();
        //ids.value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 1)),
                &MaybeRelocatable::from(bigint!(17)),
            )
            .unwrap();
        //ids.value (at (1, 0))
        //ids.dict_ptr (1, 0):
        //  dict_ptr.key = (1, 1)
        //  dict_ptr.prev_value = (1, 2)
        //  dict_ptr.new_value = (1, 3)
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 2)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("key"), bigint!(0));
        ids.insert(String::from("new_value"), bigint!(1));
        ids.insert(String::from("dict_ptr"), bigint!(2));
        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -3,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                2,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Ok(())
        );
        //Check that the dictionary was updated with the new key-value pair (5, 17)
        assert_eq!(
            vm.dict_manager
                .trackers
                .get_mut(&1)
                .unwrap()
                .get_value(&bigint!(5)),
            Ok(&bigint!(17))
        );
        //Check that the tracker's current_ptr has moved accordingly
        assert_eq!(
            vm.dict_manager.trackers.get(&1).unwrap().current_ptr,
            relocatable!(1, 3)
        );
        //Check the value of dict_ptr.prev_value, should be equal to the default_value (2)
        assert_eq!(
            vm.memory.get(&MaybeRelocatable::from((1, 1))),
            Ok(Some(&MaybeRelocatable::from(bigint!(2))))
        );
    }

    #[test]
    fn run_dict_write_default_valid_overwrite_value() {
        let hint_code = "dict_tracker = __dict_manager.get_tracker(ids.dict_ptr)\ndict_tracker.current_ptr += ids.DictAccess.SIZE\nids.dict_ptr.prev_value = dict_tracker.data[ids.key]\ndict_tracker.data[ids.key] = ids.new_value"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 3));
        //Create tracker
        //current_ptr = dict_ptr = (1, 0)
        let mut tracker = DictTracker::new_default_dict(&relocatable!(1, 0), &bigint!(2), None);
        //Add key-value pair (5, 10)
        tracker.insert_value(&bigint!(5_i32), &bigint!(10_i32));
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //Insert ids into memory
        //ids.key
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(5)),
            )
            .unwrap();
        //ids.value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 1)),
                &MaybeRelocatable::from(bigint!(17)),
            )
            .unwrap();
        //ids.value (at (1, 0))
        //ids.dict_ptr (1, 0):
        //  dict_ptr.key = (1, 1)
        //  dict_ptr.prev_value = (1, 2)
        //  dict_ptr.new_value = (1, 3)
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 2)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("key"), bigint!(0));
        ids.insert(String::from("new_value"), bigint!(1));
        ids.insert(String::from("dict_ptr"), bigint!(2));
        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -3,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                2,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Ok(())
        );
        //Check that the dictionary was updated with the new key-value pair (5, 17)
        assert_eq!(
            vm.dict_manager
                .trackers
                .get_mut(&1)
                .unwrap()
                .get_value(&bigint!(5)),
            Ok(&bigint!(17))
        );
        //Check that the tracker's current_ptr has moved accordingly
        assert_eq!(
            vm.dict_manager.trackers.get(&1).unwrap().current_ptr,
            relocatable!(1, 3)
        );
        //Check the value of dict_ptr.prev_value, should be equal to the previously inserted value (10)
        assert_eq!(
            vm.memory.get(&MaybeRelocatable::from((1, 1))),
            Ok(Some(&MaybeRelocatable::from(bigint!(10))))
        );
    }

    #[test]
    fn run_dict_write_simple_valid_overwrite_value() {
        let hint_code = "dict_tracker = __dict_manager.get_tracker(ids.dict_ptr)\ndict_tracker.current_ptr += ids.DictAccess.SIZE\nids.dict_ptr.prev_value = dict_tracker.data[ids.key]\ndict_tracker.data[ids.key] = ids.new_value"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 3));
        //Create tracker
        //current_ptr = dict_ptr = (1, 0)
        let mut tracker = DictTracker::new_empty(&relocatable!(1, 0));
        //Add key-value pair (5, 10)
        tracker.insert_value(&bigint!(5), &bigint!(10));
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //Insert ids into memory
        //ids.key
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(5)),
            )
            .unwrap();
        //ids.value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 1)),
                &MaybeRelocatable::from(bigint!(17)),
            )
            .unwrap();
        //ids.value (at (1, 0))
        //ids.dict_ptr (1, 0):
        //  dict_ptr.key = (1, 1)
        //  dict_ptr.prev_value = (1, 2)
        //  dict_ptr.new_value = (1, 3)
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 2)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("key"), bigint!(0));
        ids.insert(String::from("new_value"), bigint!(1));
        ids.insert(String::from("dict_ptr"), bigint!(2));
        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -3,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                2,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Ok(())
        );
        //Check that the dictionary was updated with the new key-value pair (5, 17)
        assert_eq!(
            vm.dict_manager
                .trackers
                .get_mut(&1)
                .unwrap()
                .get_value(&bigint!(5)),
            Ok(&bigint!(17))
        );
        //Check that the tracker's current_ptr has moved accordingly
        assert_eq!(
            vm.dict_manager.trackers.get(&1).unwrap().current_ptr,
            relocatable!(1, 3)
        );
        //Check the value of dict_ptr.prev_value, should be equal to the previously inserted value (10)
        assert_eq!(
            vm.memory.get(&MaybeRelocatable::from((1, 1))),
            Ok(Some(&MaybeRelocatable::from(bigint!(10))))
        );
    }

    #[test]
    fn run_dict_write_simple_valid_cant_write_new_key() {
        let hint_code = "dict_tracker = __dict_manager.get_tracker(ids.dict_ptr)\ndict_tracker.current_ptr += ids.DictAccess.SIZE\nids.dict_ptr.prev_value = dict_tracker.data[ids.key]\ndict_tracker.data[ids.key] = ids.new_value"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 3));
        //Create tracker
        //current_ptr = dict_ptr = (1, 0)
        let tracker = DictTracker::new_empty(&relocatable!(1, 0));
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //Insert ids into memory
        //ids.key
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(5)),
            )
            .unwrap();
        //ids.value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 1)),
                &MaybeRelocatable::from(bigint!(17)),
            )
            .unwrap();
        //ids.value (at (1, 0))
        //ids.dict_ptr (1, 0):
        //  dict_ptr.key = (1, 1)
        //  dict_ptr.prev_value = (1, 2)
        //  dict_ptr.new_value = (1, 3)
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 2)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("key"), bigint!(0));
        ids.insert(String::from("new_value"), bigint!(1));
        ids.insert(String::from("dict_ptr"), bigint!(2));
        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -3,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                2,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Err(VirtualMachineError::NoValueForKey(bigint!(5)))
        );
    }

    #[test]
    fn run_dict_update_simple_valid() {
        let hint_code = "# Verify dict pointer and prev value.\ndict_tracker = __dict_manager.get_tracker(ids.dict_ptr)\ncurrent_value = dict_tracker.data[ids.key]\nassert current_value == ids.prev_value, \\\n    f'Wrong previous value in dict. Got {ids.prev_value}, expected {current_value}.'\n\n# Update value.\ndict_tracker.data[ids.key] = ids.new_value\ndict_tracker.current_ptr += ids.DictAccess.SIZE"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 4));
        //Create tracker
        //current_ptr = dict_ptr = (1, 0)
        let mut tracker = DictTracker::new_empty(&relocatable!(1, 0));
        //Add key-value pair (5, 10)
        tracker.insert_value(&bigint!(5), &bigint!(10));
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //Insert ids into memory
        //ids.key
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(5)),
            )
            .unwrap();
        //ids.prev_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 1)),
                &MaybeRelocatable::from(bigint!(10)),
            )
            .unwrap();
        //ids.new_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 2)),
                &MaybeRelocatable::from(bigint!(20)),
            )
            .unwrap();
        //ids.dict_ptr
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 3)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //ids.dict_ptr (1, 0):
        //  dict_ptr.key = (1, 1)
        //  dict_ptr.prev_value = (1, 2)
        //  dict_ptr.new_value = (1, 3)
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("key"), bigint!(0));
        ids.insert(String::from("prev_value"), bigint!(1));
        ids.insert(String::from("new_value"), bigint!(2));
        ids.insert(String::from("dict_ptr"), bigint!(3));

        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -4,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -3,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                2,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                3,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Ok(())
        );
        //Check that the dictionary was updated with the new key-value pair (5, 20)
        assert_eq!(
            vm.dict_manager
                .trackers
                .get_mut(&1)
                .unwrap()
                .get_value(&bigint!(5)),
            Ok(&bigint!(20))
        );
        //Check that the tracker's current_ptr has moved accordingly
        assert_eq!(
            vm.dict_manager.trackers.get(&1).unwrap().current_ptr,
            relocatable!(1, 3)
        );
    }

    #[test]
    fn run_dict_update_simple_valid_no_change() {
        let hint_code = "# Verify dict pointer and prev value.\ndict_tracker = __dict_manager.get_tracker(ids.dict_ptr)\ncurrent_value = dict_tracker.data[ids.key]\nassert current_value == ids.prev_value, \\\n    f'Wrong previous value in dict. Got {ids.prev_value}, expected {current_value}.'\n\n# Update value.\ndict_tracker.data[ids.key] = ids.new_value\ndict_tracker.current_ptr += ids.DictAccess.SIZE"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 4));
        //Create tracker
        //current_ptr = dict_ptr = (1, 0)
        let mut tracker = DictTracker::new_empty(&relocatable!(1, 0));
        //Add key-value pair (5, 10)
        tracker.insert_value(&bigint!(5), &bigint!(10));
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //Insert ids into memory
        //ids.key
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(5)),
            )
            .unwrap();
        //ids.prev_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 1)),
                &MaybeRelocatable::from(bigint!(10)),
            )
            .unwrap();
        //ids.new_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 2)),
                &MaybeRelocatable::from(bigint!(10)),
            )
            .unwrap();
        //ids.dict_ptr
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 3)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //ids.dict_ptr (1, 0):
        //  dict_ptr.key = (1, 1)
        //  dict_ptr.prev_value = (1, 2)
        //  dict_ptr.new_value = (1, 3)
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("key"), bigint!(0));
        ids.insert(String::from("prev_value"), bigint!(1));
        ids.insert(String::from("new_value"), bigint!(2));
        ids.insert(String::from("dict_ptr"), bigint!(3));

        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -4,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -3,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                2,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                3,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Ok(())
        );
        //Check that the dictionary was updated with the new key-value pair (5, 20)
        assert_eq!(
            vm.dict_manager
                .trackers
                .get_mut(&1)
                .unwrap()
                .get_value(&bigint!(5)),
            Ok(&bigint!(10))
        );
        //Check that the tracker's current_ptr has moved accordingly
        assert_eq!(
            vm.dict_manager.trackers.get(&1).unwrap().current_ptr,
            relocatable!(1, 3)
        );
    }

    #[test]
    fn run_dict_update_simple_invalid_wrong_prev_key() {
        let hint_code = "# Verify dict pointer and prev value.\ndict_tracker = __dict_manager.get_tracker(ids.dict_ptr)\ncurrent_value = dict_tracker.data[ids.key]\nassert current_value == ids.prev_value, \\\n    f'Wrong previous value in dict. Got {ids.prev_value}, expected {current_value}.'\n\n# Update value.\ndict_tracker.data[ids.key] = ids.new_value\ndict_tracker.current_ptr += ids.DictAccess.SIZE"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 4));
        //Create tracker
        //current_ptr = dict_ptr = (1, 0)
        let mut tracker = DictTracker::new_empty(&relocatable!(1, 0));
        //Add key-value pair (5, 10)
        tracker.insert_value(&bigint!(5), &bigint!(10));
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //Insert ids into memory
        //ids.key
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(5)),
            )
            .unwrap();
        //ids.prev_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 1)),
                &MaybeRelocatable::from(bigint!(11)),
            )
            .unwrap();
        //ids.new_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 2)),
                &MaybeRelocatable::from(bigint!(10)),
            )
            .unwrap();
        //ids.dict_ptr
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 3)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //ids.dict_ptr (1, 0):
        //  dict_ptr.key = (1, 1)
        //  dict_ptr.prev_value = (1, 2)
        //  dict_ptr.new_value = (1, 3)
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("key"), bigint!(0));
        ids.insert(String::from("prev_value"), bigint!(1));
        ids.insert(String::from("new_value"), bigint!(2));
        ids.insert(String::from("dict_ptr"), bigint!(3));

        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -4,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -3,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                2,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                3,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Err(VirtualMachineError::WrongPrevValue(
                bigint!(11),
                bigint!(10),
                bigint!(5)
            ))
        );
    }

    #[test]
    fn run_dict_update_simple_invalid_wrong_key() {
        let hint_code = "# Verify dict pointer and prev value.\ndict_tracker = __dict_manager.get_tracker(ids.dict_ptr)\ncurrent_value = dict_tracker.data[ids.key]\nassert current_value == ids.prev_value, \\\n    f'Wrong previous value in dict. Got {ids.prev_value}, expected {current_value}.'\n\n# Update value.\ndict_tracker.data[ids.key] = ids.new_value\ndict_tracker.current_ptr += ids.DictAccess.SIZE"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 4));
        //Create tracker
        //current_ptr = dict_ptr = (1, 0)
        let mut tracker = DictTracker::new_empty(&relocatable!(1, 0));
        //Add key-value pair (5, 10)
        tracker.insert_value(&bigint!(5), &bigint!(10));
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //Insert ids into memory
        //ids.key
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(6)),
            )
            .unwrap();
        //ids.prev_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 1)),
                &MaybeRelocatable::from(bigint!(10)),
            )
            .unwrap();
        //ids.new_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 2)),
                &MaybeRelocatable::from(bigint!(10)),
            )
            .unwrap();
        //ids.dict_ptr
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 3)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //ids.dict_ptr (1, 0):
        //  dict_ptr.key = (1, 1)
        //  dict_ptr.prev_value = (1, 2)
        //  dict_ptr.new_value = (1, 3)
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("key"), bigint!(0));
        ids.insert(String::from("prev_value"), bigint!(1));
        ids.insert(String::from("new_value"), bigint!(2));
        ids.insert(String::from("dict_ptr"), bigint!(3));

        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -4,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -3,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                2,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                3,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Err(VirtualMachineError::NoValueForKey(bigint!(6),))
        );
    }

    #[test]
    fn run_dict_update_default_valid() {
        let hint_code = "# Verify dict pointer and prev value.\ndict_tracker = __dict_manager.get_tracker(ids.dict_ptr)\ncurrent_value = dict_tracker.data[ids.key]\nassert current_value == ids.prev_value, \\\n    f'Wrong previous value in dict. Got {ids.prev_value}, expected {current_value}.'\n\n# Update value.\ndict_tracker.data[ids.key] = ids.new_value\ndict_tracker.current_ptr += ids.DictAccess.SIZE"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 4));
        //Create tracker
        //current_ptr = dict_ptr = (1, 0)
        let mut tracker =
            DictTracker::new_default_dict(&relocatable!(1, 0), &bigint!(17), Some(HashMap::new()));
        //Add key-value pair (5, 10)
        tracker.insert_value(&bigint!(5), &bigint!(10));
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //Insert ids into memory
        //ids.key
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(5)),
            )
            .unwrap();
        //ids.prev_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 1)),
                &MaybeRelocatable::from(bigint!(10)),
            )
            .unwrap();
        //ids.new_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 2)),
                &MaybeRelocatable::from(bigint!(20)),
            )
            .unwrap();
        //ids.dict_ptr
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 3)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //ids.dict_ptr (1, 0):
        //  dict_ptr.key = (1, 1)
        //  dict_ptr.prev_value = (1, 2)
        //  dict_ptr.new_value = (1, 3)
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("key"), bigint!(0));
        ids.insert(String::from("prev_value"), bigint!(1));
        ids.insert(String::from("new_value"), bigint!(2));
        ids.insert(String::from("dict_ptr"), bigint!(3));

        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -4,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -3,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                2,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                3,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Ok(())
        );
        //Check that the dictionary was updated with the new key-value pair (5, 20)
        assert_eq!(
            vm.dict_manager
                .trackers
                .get_mut(&1)
                .unwrap()
                .get_value(&bigint!(5)),
            Ok(&bigint!(20))
        );
        //Check that the tracker's current_ptr has moved accordingly
        assert_eq!(
            vm.dict_manager.trackers.get(&1).unwrap().current_ptr,
            relocatable!(1, 3)
        );
    }

    #[test]
    fn run_dict_update_default_valid_no_change() {
        let hint_code = "# Verify dict pointer and prev value.\ndict_tracker = __dict_manager.get_tracker(ids.dict_ptr)\ncurrent_value = dict_tracker.data[ids.key]\nassert current_value == ids.prev_value, \\\n    f'Wrong previous value in dict. Got {ids.prev_value}, expected {current_value}.'\n\n# Update value.\ndict_tracker.data[ids.key] = ids.new_value\ndict_tracker.current_ptr += ids.DictAccess.SIZE"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 4));
        //Create tracker
        //current_ptr = dict_ptr = (1, 0)
        let mut tracker =
            DictTracker::new_default_dict(&relocatable!(1, 0), &bigint!(17), Some(HashMap::new()));
        //Add key-value pair (5, 10)
        tracker.insert_value(&bigint!(5), &bigint!(10));
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //Insert ids into memory
        //ids.key
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(5)),
            )
            .unwrap();
        //ids.prev_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 1)),
                &MaybeRelocatable::from(bigint!(10)),
            )
            .unwrap();
        //ids.new_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 2)),
                &MaybeRelocatable::from(bigint!(10)),
            )
            .unwrap();
        //ids.dict_ptr
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 3)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //ids.dict_ptr (1, 0):
        //  dict_ptr.key = (1, 1)
        //  dict_ptr.prev_value = (1, 2)
        //  dict_ptr.new_value = (1, 3)
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("key"), bigint!(0));
        ids.insert(String::from("prev_value"), bigint!(1));
        ids.insert(String::from("new_value"), bigint!(2));
        ids.insert(String::from("dict_ptr"), bigint!(3));

        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -4,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -3,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                2,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                3,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Ok(())
        );
        //Check that the dictionary was updated with the new key-value pair (5, 20)
        assert_eq!(
            vm.dict_manager
                .trackers
                .get_mut(&1)
                .unwrap()
                .get_value(&bigint!(5)),
            Ok(&bigint!(10))
        );
        //Check that the tracker's current_ptr has moved accordingly
        assert_eq!(
            vm.dict_manager.trackers.get(&1).unwrap().current_ptr,
            relocatable!(1, 3)
        );
    }

    #[test]
    fn run_dict_update_default_invalid_wrong_prev_key() {
        let hint_code = "# Verify dict pointer and prev value.\ndict_tracker = __dict_manager.get_tracker(ids.dict_ptr)\ncurrent_value = dict_tracker.data[ids.key]\nassert current_value == ids.prev_value, \\\n    f'Wrong previous value in dict. Got {ids.prev_value}, expected {current_value}.'\n\n# Update value.\ndict_tracker.data[ids.key] = ids.new_value\ndict_tracker.current_ptr += ids.DictAccess.SIZE"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 4));
        //Create tracker
        //current_ptr = dict_ptr = (1, 0)
        let mut tracker =
            DictTracker::new_default_dict(&relocatable!(1, 0), &bigint!(17), Some(HashMap::new()));
        //Add key-value pair (5, 10)
        tracker.insert_value(&bigint!(5), &bigint!(10));
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //Insert ids into memory
        //ids.key
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(5)),
            )
            .unwrap();
        //ids.prev_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 1)),
                &MaybeRelocatable::from(bigint!(11)),
            )
            .unwrap();
        //ids.new_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 2)),
                &MaybeRelocatable::from(bigint!(10)),
            )
            .unwrap();
        //ids.dict_ptr
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 3)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //ids.dict_ptr (1, 0):
        //  dict_ptr.key = (1, 1)
        //  dict_ptr.prev_value = (1, 2)
        //  dict_ptr.new_value = (1, 3)
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("key"), bigint!(0));
        ids.insert(String::from("prev_value"), bigint!(1));
        ids.insert(String::from("new_value"), bigint!(2));
        ids.insert(String::from("dict_ptr"), bigint!(3));

        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -4,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -3,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                2,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                3,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Err(VirtualMachineError::WrongPrevValue(
                bigint!(11),
                bigint!(10),
                bigint!(5)
            ))
        );
    }

    #[test]
    fn run_dict_update_default_invalid_wrong_key() {
        let hint_code = "# Verify dict pointer and prev value.\ndict_tracker = __dict_manager.get_tracker(ids.dict_ptr)\ncurrent_value = dict_tracker.data[ids.key]\nassert current_value == ids.prev_value, \\\n    f'Wrong previous value in dict. Got {ids.prev_value}, expected {current_value}.'\n\n# Update value.\ndict_tracker.data[ids.key] = ids.new_value\ndict_tracker.current_ptr += ids.DictAccess.SIZE"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 4));
        //Create tracker
        //current_ptr = dict_ptr = (1, 0)
        let mut tracker =
            DictTracker::new_default_dict(&relocatable!(1, 0), &bigint!(17), Some(HashMap::new()));
        //Add key-value pair (5, 10)
        tracker.insert_value(&bigint!(5), &bigint!(10));
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //Insert ids into memory
        //ids.key
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(6)),
            )
            .unwrap();
        //ids.prev_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 1)),
                &MaybeRelocatable::from(bigint!(10)),
            )
            .unwrap();
        //ids.new_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 2)),
                &MaybeRelocatable::from(bigint!(10)),
            )
            .unwrap();
        //ids.dict_ptr
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 3)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //ids.dict_ptr (1, 0):
        //  dict_ptr.key = (1, 1)
        //  dict_ptr.prev_value = (1, 2)
        //  dict_ptr.new_value = (1, 3)
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("key"), bigint!(0));
        ids.insert(String::from("prev_value"), bigint!(1));
        ids.insert(String::from("new_value"), bigint!(2));
        ids.insert(String::from("dict_ptr"), bigint!(3));

        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -4,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -3,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                2,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                3,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Err(VirtualMachineError::WrongPrevValue(
                bigint!(10),
                bigint!(17),
                bigint!(6)
            ))
        );
    }

    #[test]
    fn run_dict_update_default_valid_no_key_prev_value_equals_default() {
        let hint_code = "# Verify dict pointer and prev value.\ndict_tracker = __dict_manager.get_tracker(ids.dict_ptr)\ncurrent_value = dict_tracker.data[ids.key]\nassert current_value == ids.prev_value, \\\n    f'Wrong previous value in dict. Got {ids.prev_value}, expected {current_value}.'\n\n# Update value.\ndict_tracker.data[ids.key] = ids.new_value\ndict_tracker.current_ptr += ids.DictAccess.SIZE"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 4));
        //Create tracker
        //current_ptr = dict_ptr = (1, 0)
        let tracker =
            DictTracker::new_default_dict(&relocatable!(1, 0), &bigint!(17), Some(HashMap::new()));
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //Insert ids into memory
        //ids.key
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from(bigint!(5)),
            )
            .unwrap();
        //ids.prev_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 1)),
                &MaybeRelocatable::from(bigint!(17)),
            )
            .unwrap();
        //ids.new_value
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 2)),
                &MaybeRelocatable::from(bigint!(20)),
            )
            .unwrap();
        //ids.dict_ptr
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 3)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //ids.dict_ptr (1, 0):
        //  dict_ptr.key = (1, 1)
        //  dict_ptr.prev_value = (1, 2)
        //  dict_ptr.new_value = (1, 3)
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("key"), bigint!(0));
        ids.insert(String::from("prev_value"), bigint!(1));
        ids.insert(String::from("new_value"), bigint!(2));
        ids.insert(String::from("dict_ptr"), bigint!(3));

        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -4,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -3,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                2,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                3,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Ok(())
        );
        //Check that the dictionary was updated with the new key-value pair (5, 20)
        assert_eq!(
            vm.dict_manager
                .trackers
                .get_mut(&1)
                .unwrap()
                .get_value(&bigint!(5)),
            Ok(&bigint!(20))
        );
        //Check that the tracker's current_ptr has moved accordingly
        assert_eq!(
            vm.dict_manager.trackers.get(&1).unwrap().current_ptr,
            relocatable!(1, 3)
        );
    }

    #[test]
    fn run_dict_squash_copy_dict_valid_empty_dict() {
        let hint_code = "# Prepare arguments for dict_new. In particular, the same dictionary values should be copied\n# to the new (squashed) dictionary.\nvm_enter_scope({\n    # Make __dict_manager accessible.\n    '__dict_manager': __dict_manager,\n    # Create a copy of the dict, in case it changes in the future.\n    'initial_dict': dict(__dict_manager.get_dict(ids.dict_accesses_end)),\n})"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 1));
        //Initialize dictionary
        let dictionary = HashMap::<BigInt, BigInt>::new();
        //Create tracker
        let mut tracker = DictTracker::new_empty(&relocatable!(1, 0));
        tracker.data = Dictionary::SimpleDictionary(dictionary);
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //ids.dict_access
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("dict_accesses_end"), bigint!(0));
        //Create references
        vm.references = HashMap::from([(
            0,
            HintReference {
                register: Register::FP,
                offset1: -1,
                offset2: 0,
                inner_dereference: false,
                ap_tracking_data: None,
                immediate: None,
            },
        )]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Ok(())
        );
        //Check that a new exec scope has been created
        assert_eq!(vm.exec_scopes.data.len(), 2);
        //Check that this scope contains the expected initial-dict
        let variables = vm.exec_scopes.get_local_variables().unwrap();
        assert_eq!(variables.len(), 1);
        assert_eq!(
            variables.get("initial_dict"),
            Some(&PyValueType::Dictionary(HashMap::new()))
        );
    }

    #[test]
    fn run_dict_squash_copy_dict_valid_non_empty_dict() {
        let hint_code = "# Prepare arguments for dict_new. In particular, the same dictionary values should be copied\n# to the new (squashed) dictionary.\nvm_enter_scope({\n    # Make __dict_manager accessible.\n    '__dict_manager': __dict_manager,\n    # Create a copy of the dict, in case it changes in the future.\n    'initial_dict': dict(__dict_manager.get_dict(ids.dict_accesses_end)),\n})"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 1));
        //Initialize dictionary
        let mut dictionary = HashMap::<BigInt, BigInt>::new();
        dictionary.insert(bigint!(1), bigint!(2));
        dictionary.insert(bigint!(3), bigint!(4));
        dictionary.insert(bigint!(5), bigint!(6));
        //Create tracker
        let mut tracker = DictTracker::new_empty(&relocatable!(1, 0));
        tracker.data = Dictionary::SimpleDictionary(dictionary);
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //ids.dict_access
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("dict_accesses_end"), bigint!(0));
        //Create references
        vm.references = HashMap::from([(
            0,
            HintReference {
                register: Register::FP,
                offset1: -1,
                offset2: 0,
                inner_dereference: false,
                ap_tracking_data: None,
                immediate: None,
            },
        )]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Ok(())
        );
        //Check that a new exec scope has been created
        assert_eq!(vm.exec_scopes.data.len(), 2);
        //Check that this scope contains the expected initial-dict
        let variables = vm.exec_scopes.get_local_variables().unwrap();
        assert_eq!(variables.len(), 1);
        assert_eq!(
            variables.get("initial_dict"),
            Some(&PyValueType::Dictionary(HashMap::from([
                (bigint!(1), bigint!(2)),
                (bigint!(3), bigint!(4)),
                (bigint!(5), bigint!(6))
            ])))
        );
    }

    #[test]
    fn run_dict_squash_copy_dict_invalid_no_dict() {
        let hint_code = "# Prepare arguments for dict_new. In particular, the same dictionary values should be copied\n# to the new (squashed) dictionary.\nvm_enter_scope({\n    # Make __dict_manager accessible.\n    '__dict_manager': __dict_manager,\n    # Create a copy of the dict, in case it changes in the future.\n    'initial_dict': dict(__dict_manager.get_dict(ids.dict_accesses_end)),\n})"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 1));
        //Create manager
        let dict_manager = DictManager::new();
        vm.dict_manager = dict_manager;
        //ids.dict_access
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("dict_accesses_end"), bigint!(0));
        //Create references
        vm.references = HashMap::from([(
            0,
            HintReference {
                register: Register::FP,
                offset1: -1,
                offset2: 0,
                inner_dereference: false,
                ap_tracking_data: None,
                immediate: None,
            },
        )]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Err(VirtualMachineError::NoDictTracker(1))
        );
    }

    #[test]
    fn run_dict_squash_update_ptr_no_tracker() {
        let hint_code = "# Update the DictTracker's current_ptr to point to the end of the squashed dict.\n__dict_manager.get_tracker(ids.squashed_dict_start).current_ptr = \\\n    ids.squashed_dict_end.address_"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 2));
        //Create manager
        let dict_manager = DictManager::new();
        vm.dict_manager = dict_manager;
        //ids.squash_dict_start
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //ids.squash_dict_end
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 1)),
                &MaybeRelocatable::from((1, 3)),
            )
            .unwrap();
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("squashed_dict_start"), bigint!(0));
        ids.insert(String::from("squashed_dict_end"), bigint!(0));
        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Err(VirtualMachineError::NoDictTracker(1))
        );
    }

    #[test]
    fn run_dict_squash_update_ptr_valid() {
        let hint_code = "# Update the DictTracker's current_ptr to point to the end of the squashed dict.\n__dict_manager.get_tracker(ids.squashed_dict_start).current_ptr = \\\n    ids.squashed_dict_end.address_"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 2));
        //Initialize dictionary
        let mut dictionary = HashMap::<BigInt, BigInt>::new();
        dictionary.insert(bigint!(1), bigint!(2));
        //Create tracker
        let mut tracker = DictTracker::new_empty(&relocatable!(1, 0));
        tracker.data = Dictionary::SimpleDictionary(dictionary);
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //ids.squash_dict_start
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from((1, 0)),
            )
            .unwrap();
        //ids.squash_dict_end
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 1)),
                &MaybeRelocatable::from((1, 3)),
            )
            .unwrap();
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("squashed_dict_start"), bigint!(0));
        ids.insert(String::from("squashed_dict_end"), bigint!(1));
        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Ok(())
        );
        //Check the updated pointer
        assert_eq!(
            vm.dict_manager
                .get_tracker(&relocatable!(1, 3))
                .unwrap()
                .current_ptr,
            relocatable!(1, 3)
        );
    }

    #[test]
    fn run_dict_squash_update_ptr_mismatched_dict_ptr() {
        let hint_code = "# Update the DictTracker's current_ptr to point to the end of the squashed dict.\n__dict_manager.get_tracker(ids.squashed_dict_start).current_ptr = \\\n    ids.squashed_dict_end.address_"
            .as_bytes();
        let mut vm = VirtualMachine::new(
            BigInt::new(Sign::Plus, vec![1, 0, 0, 0, 0, 0, 17, 134217728]),
            Vec::new(),
            false,
        );
        for _ in 0..2 {
            vm.segments.add(&mut vm.memory, None);
        }
        //Initialize fp
        vm.run_context.fp = MaybeRelocatable::from((0, 2));
        //Initialize dictionary
        let mut dictionary = HashMap::<BigInt, BigInt>::new();
        dictionary.insert(bigint!(1), bigint!(2));
        //Create tracker
        let mut tracker = DictTracker::new_empty(&relocatable!(1, 0));
        tracker.data = Dictionary::SimpleDictionary(dictionary);
        //Create manager
        let mut dict_manager = DictManager::new();
        dict_manager.trackers.insert(1, tracker);
        vm.dict_manager = dict_manager;
        //ids.squash_dict_start
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 0)),
                &MaybeRelocatable::from((1, 3)),
            )
            .unwrap();
        //ids.squash_dict_end
        vm.memory
            .insert(
                &MaybeRelocatable::from((0, 1)),
                &MaybeRelocatable::from((1, 6)),
            )
            .unwrap();
        //Create ids
        let mut ids = HashMap::<String, BigInt>::new();
        ids.insert(String::from("squashed_dict_start"), bigint!(0));
        ids.insert(String::from("squashed_dict_end"), bigint!(1));
        //Create references
        vm.references = HashMap::from([
            (
                0,
                HintReference {
                    register: Register::FP,
                    offset1: -2,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
            (
                1,
                HintReference {
                    register: Register::FP,
                    offset1: -1,
                    offset2: 0,
                    inner_dereference: false,
                    ap_tracking_data: None,
                    immediate: None,
                },
            ),
        ]);
        //Execute the hint
        assert_eq!(
            execute_hint(&mut vm, hint_code, ids, &ApTracking::new()),
            Err(VirtualMachineError::MismatchedDictPtr(
                relocatable!(1, 0),
                relocatable!(1, 3)
            ))
        );
    }
}
