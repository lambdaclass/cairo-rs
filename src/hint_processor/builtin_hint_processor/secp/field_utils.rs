use crate::any_box;
use crate::stdlib::{collections::HashMap, prelude::*};

use crate::{
    hint_processor::{
        builtin_hint_processor::hint_utils::{insert_value_from_var_name, insert_value_into_ap},
        hint_processor_definition::HintReference,
    },
    math_utils::div_mod,
    serde::deserialize_program::ApTracking,
    types::exec_scope::ExecutionScopes,
    vm::{errors::hint_errors::HintError, vm_core::VirtualMachine},
};
use felt::Felt252;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Zero};

use super::secp_utils::SECP_P;
use super::{bigint_utils::BigInt3, secp_utils::pack};

/*
Implements hint:
%{
    from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack

    q, r = divmod(pack(ids.val, PRIME), SECP_P)
    assert r == 0, f"verify_zero: Invalid input {ids.val.d0, ids.val.d1, ids.val.d2}."
    ids.q = q % PRIME
%}
*/
pub fn verify_zero(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    exec_scopes.assign_or_update_variable("SECP_P", any_box!(SECP_P.clone()));
    let val = pack(BigInt3::from_var_name("val", vm, ids_data, ap_tracking)?);
    let (q, r) = val.div_rem(&SECP_P);
    if !r.is_zero() {
        return Err(HintError::SecpVerifyZero(val));
    }

    insert_value_from_var_name("q", Felt252::new(q), vm, ids_data, ap_tracking)
}

/*
Implements hint:
%{
    from starkware.cairo.common.cairo_secp.secp_utils import pack

    q, r = divmod(pack(ids.val, PRIME), SECP_P)
    assert r == 0, f"verify_zero: Invalid input {ids.val.d0, ids.val.d1, ids.val.d2}."
    ids.q = q % PRIME
%}
*/
pub fn verify_zero_with_external_const(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let secp_p = exec_scopes.get_ref("SECP_P")?;
    let val = pack(BigInt3::from_var_name("val", vm, ids_data, ap_tracking)?);
    let (q, r) = val.div_rem(&secp_p);
    if !r.is_zero() {
        return Err(HintError::SecpVerifyZero(val));
    }

    insert_value_from_var_name("q", Felt252::new(q), vm, ids_data, ap_tracking)
}

/*
Implements hint:
%{
    from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack

    value = pack(ids.x, PRIME) % SECP_P
%}
*/
pub fn reduce(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    exec_scopes.assign_or_update_variable("SECP_P", any_box!(SECP_P.clone()));
    let value = pack(BigInt3::from_var_name("x", vm, ids_data, ap_tracking)?);
    exec_scopes.insert_value("value", value.mod_floor(&SECP_P));
    Ok(())
}

/*
Implements hint:
%{
    from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack

    x = pack(ids.x, PRIME) % SECP_P
%}
*/
pub fn is_zero_pack(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    exec_scopes.assign_or_update_variable("SECP_P", any_box!(SECP_P.clone()));

    let x_packed = pack(BigInt3::from_var_name("x", vm, ids_data, ap_tracking)?);
    let x = x_packed.mod_floor(&SECP_P);
    exec_scopes.insert_value("x", x);
    Ok(())
}

/*
Implements hint:
in .cairo program
if nondet %{ x == 0 %} != 0:

On .json compiled program
"memory[ap] = to_felt_or_relocatable(x == 0)"
*/
pub fn is_zero_nondet(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
) -> Result<(), HintError> {
    //Get `x` variable from vm scope
    let x = exec_scopes.get::<BigInt>("x")?;

    let value = if x.is_zero() {
        Felt252::one()
    } else {
        Felt252::zero()
    };
    insert_value_into_ap(vm, value)
}

/*
Implements hint:
%{
    from starkware.cairo.common.cairo_secp.secp_utils import SECP_P
    from starkware.python.math_utils import div_mod

    value = x_inv = div_mod(1, x, SECP_P)
%}
*/
pub fn is_zero_assign_scope_variables(exec_scopes: &mut ExecutionScopes) -> Result<(), HintError> {
    exec_scopes.assign_or_update_variable("SECP_P", any_box!(SECP_P.clone()));
    //Get `x` variable from vm scope
    let x = exec_scopes.get::<BigInt>("x")?;

    let value = div_mod(&BigInt::one(), &x, &SECP_P);
    exec_scopes.insert_value("value", value.clone());
    exec_scopes.insert_value("x_inv", value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::string::ToString;
    use crate::vm::vm_memory::memory_segments::MemorySegmentManager;
    use crate::{
        any_box,
        hint_processor::{
            builtin_hint_processor::builtin_hint_processor_definition::{
                BuiltinHintProcessor, HintProcessorData,
            },
            hint_processor_definition::HintProcessor,
        },
        types::{
            exec_scope::ExecutionScopes,
            relocatable::{MaybeRelocatable, Relocatable},
        },
        utils::test_utils::*,
        vm::{
            errors::memory_errors::MemoryError, runners::builtin_runner::RangeCheckBuiltinRunner,
            vm_memory::memory::Memory,
        },
    };
    use assert_matches::assert_matches;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_verify_zero_ok() {
        let hint_codes = vec![
            "from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack\n\nq, r = divmod(pack(ids.val, PRIME), SECP_P)\nassert r == 0, f\"verify_zero: Invalid input {ids.val.d0, ids.val.d1, ids.val.d2}.\"\nids.q = q % PRIME",
            "from starkware.cairo.common.cairo_secp.secp_utils import SECP_P\nq, r = divmod(pack(ids.val, PRIME), SECP_P)\nassert r == 0, f\"verify_zero: Invalid input {ids.val.d0, ids.val.d1, ids.val.d2}.\"\nids.q = q % PRIME",
        ];
        for hint_code in hint_codes {
            let mut vm = vm_with_range_check!();
            //Initialize run_context
            run_context!(vm, 0, 9, 9);
            //Create hint data
            let ids_data = non_continuous_ids_data![("val", -5), ("q", 0)];
            vm.segments = segments![((1, 4), 0), ((1, 5), 0), ((1, 6), 0)];
            //Execute the hint
            assert!(run_hint!(vm, ids_data, hint_code, exec_scopes_ref!()).is_ok());
            //Check hint memory inserts
            //ids.q
            check_memory![vm.segments.memory, ((1, 9), 0)];
        }
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_verify_zero_with_external_const_ok() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import pack\n\nq, r = divmod(pack(ids.val, PRIME), SECP_P)\nassert r == 0, f\"verify_zero: Invalid input {ids.val.d0, ids.val.d1, ids.val.d2}.\"\nids.q = q % PRIME";
        let mut vm = vm_with_range_check!();
        //Initialize run_context
        run_context!(vm, 0, 9, 9);
        //Create hint data
        let ids_data = non_continuous_ids_data![("val", -5), ("q", 0)];
        vm.segments = segments![((1, 4), 55), ((1, 5), 0), ((1, 6), 0)];

        let new_secp_p = 55;

        let mut exec_scopes = ExecutionScopes::new();
        exec_scopes.assign_or_update_variable("SECP_P", any_box!(bigint!(new_secp_p)));

        //Execute the hint
        assert!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes).is_ok());
        //Check hint memory inserts
        //ids.q
        check_memory![vm.segments.memory, ((1, 9), 1)];
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_verify_zero_error() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack\n\nq, r = divmod(pack(ids.val, PRIME), SECP_P)\nassert r == 0, f\"verify_zero: Invalid input {ids.val.d0, ids.val.d1, ids.val.d2}.\"\nids.q = q % PRIME";
        let mut vm = vm_with_range_check!();
        add_segments!(vm, 3);
        //Initialize run_context
        run_context!(vm, 0, 9, 9);
        //Create hint data
        let ids_data = non_continuous_ids_data![("val", -5), ("q", 0)];
        vm.segments = segments![((1, 4), 0), ((1, 5), 0), ((1, 6), 150)];
        //Execute the hint
        assert_matches!(
            run_hint!(
                vm,
                ids_data,
                hint_code,
                exec_scopes_ref!()
            ),
            Err(HintError::SecpVerifyZero(x)) if x == bigint_str!(
                "897946605976106752944343961220884287276604954404454400"
            )
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_verify_zero_invalid_memory_insert() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack\n\nq, r = divmod(pack(ids.val, PRIME), SECP_P)\nassert r == 0, f\"verify_zero: Invalid input {ids.val.d0, ids.val.d1, ids.val.d2}.\"\nids.q = q % PRIME";
        let mut vm = vm_with_range_check!();
        add_segments!(vm, 3);

        //Initialize run_context
        run_context!(vm, 0, 9, 9);

        //Create hint data
        let ids_data = non_continuous_ids_data![("val", -5), ("q", 0)];
        vm.segments = segments![((1, 4), 0), ((1, 5), 0), ((1, 6), 0), ((1, 9), 55)];
        //Execute the hint
        assert_matches!(
                    run_hint!(
                        vm,
                        ids_data,
                        hint_code,
                        exec_scopes_ref!()
                    ),
                    Err(HintError::Memory(
                        MemoryError::InconsistentMemory(
                            x,
                            y,
                            z
                        )
                    )) if x ==
        Relocatable::from((1, 9)) &&
                            y == MaybeRelocatable::from(Felt252::new(55_i32)) &&
                            z == MaybeRelocatable::from(Felt252::zero())
                );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_reduce_ok() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack\n\nvalue = pack(ids.x, PRIME) % SECP_P";
        let mut vm = vm_with_range_check!();
        add_segments!(vm, 3);

        //Initialize fp
        vm.run_context.fp = 25;

        //Create hint data
        let ids_data = non_continuous_ids_data![("x", -5)];

        vm.segments = segments![
            ((1, 20), ("132181232131231239112312312313213083892150", 10)),
            ((1, 21), 10),
            ((1, 22), 10)
        ];

        let mut exec_scopes = ExecutionScopes::new();
        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));

        //Check 'value' is defined in the vm scope
        assert_matches!(
            exec_scopes.get::<BigInt>("value"),
            Ok(x) if x == bigint_str!(
                "59863107065205964761754162760883789350782881856141750"
            )
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_reduce_error() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack\n\nvalue = pack(ids.x, PRIME) % SECP_P";
        let mut vm = vm_with_range_check!();
        add_segments!(vm, 3);

        //Initialize fp
        vm.run_context.fp = 25;

        //Create hint data
        let ids_data = HashMap::from([("x".to_string(), HintReference::new_simple(-5))]);
        //Skip ids.x values insert so the hint fails.
        //Execute the hint
        assert_matches!(
            run_hint!(
                vm,
                ids_data,
                hint_code,
                exec_scopes_ref!()
            ),
            Err(HintError::IdentifierHasNoMember(x, y
            )) if x == "x" && y == "d0"
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_is_zero_pack_ok() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack\n\nx = pack(ids.x, PRIME) % SECP_P";
        let mut vm = vm_with_range_check!();

        //Initialize fp
        vm.run_context.fp = 15;

        //Create hint data
        let ids_data = HashMap::from([("x".to_string(), HintReference::new_simple(-5))]);
        //Insert ids.x.d0, ids.x.d1, ids.x.d2 into memory
        vm.segments = segments![
            ((1, 10), 232113757366008801543585_i128),
            ((1, 11), 232113757366008801543585_i128),
            ((1, 12), 232113757366008801543585_i128)
        ];

        let mut exec_scopes = ExecutionScopes::new();

        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));

        //Check 'x' is defined in the vm scope
        check_scope!(
            &exec_scopes,
            [(
                "x",
                bigint_str!(
                    "1389505070847794345082847096905107459917719328738389700703952672838091425185"
                )
            )]
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_is_zero_pack_error() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack\n\nx = pack(ids.x, PRIME) % SECP_P";
        let mut vm = vm_with_range_check!();

        //Initialize fp
        vm.run_context.fp = 15;

        //Create hint data
        let ids_data = HashMap::from([("x".to_string(), HintReference::new_simple(-5))]);

        //Skip ids.x.d0, ids.x.d1, ids.x.d2 inserts so the hints fails

        //Execute the hint
        assert_matches!(
            run_hint!(
                vm,
                ids_data,
                hint_code,
                exec_scopes_ref!()
            ),
            Err(HintError::IdentifierHasNoMember(x, y
            )) if x == "x" && y == "d0"
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_is_zero_nondet_ok_true() {
        let hint_code = "memory[ap] = to_felt_or_relocatable(x == 0)";
        let mut vm = vm_with_range_check!();

        //Initialize memory
        add_segments!(vm, 2);

        //Initialize ap
        vm.run_context.ap = 15;

        let mut exec_scopes = ExecutionScopes::new();
        //Initialize vm scope with variable `x`
        exec_scopes.assign_or_update_variable("x", any_box!(BigInt::zero()));
        //Create hint data
        //Execute the hint
        assert_matches!(
            run_hint!(vm, HashMap::new(), hint_code, &mut exec_scopes),
            Ok(())
        );

        //Check hint memory insert
        //memory[ap] = to_felt_or_relocatable(x == 0)
        check_memory!(vm.segments.memory, ((1, 15), 1));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_is_zero_nondet_ok_false() {
        let hint_code = "memory[ap] = to_felt_or_relocatable(x == 0)";
        let mut vm = vm_with_range_check!();

        //Initialize memory
        add_segments!(vm, 2);

        //Initialize ap
        vm.run_context.ap = 15;

        //Initialize vm scope with variable `x`
        let mut exec_scopes = ExecutionScopes::new();
        exec_scopes.assign_or_update_variable("x", any_box!(bigint!(123890i32)));

        //Execute the hint
        assert_matches!(
            run_hint!(vm, HashMap::new(), hint_code, &mut exec_scopes),
            Ok(())
        );

        //Check hint memory insert
        //memory[ap] = to_felt_or_relocatable(x == 0)
        check_memory!(vm.segments.memory, ((1, 15), 0));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_is_zero_nondet_scope_error() {
        let hint_code = "memory[ap] = to_felt_or_relocatable(x == 0)";
        let mut vm = vm_with_range_check!();

        //Initialize memory
        add_segments!(vm, 2);

        //Initialize ap
        vm.run_context.ap = 15;

        //Skip `x` assignment

        //Execute the hint
        assert_matches!(
            run_hint!(vm, HashMap::new(), hint_code),
            Err(HintError::VariableNotInScopeError(x)) if x == *"x".to_string()
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_is_zero_nondet_invalid_memory_insert() {
        let hint_code = "memory[ap] = to_felt_or_relocatable(x == 0)";
        let mut vm = vm_with_range_check!();

        //Insert a value in ap before the hint execution, so the hint memory insert fails
        vm.segments = segments![((1, 15), 55)];

        //Initialize ap
        vm.run_context.ap = 15;

        //Initialize vm scope with variable `x`
        let mut exec_scopes = ExecutionScopes::new();
        exec_scopes.assign_or_update_variable("x", any_box!(BigInt::zero()));
        //Execute the hint
        assert_matches!(
                    run_hint!(vm, HashMap::new(), hint_code, &mut exec_scopes),
                    Err(HintError::Memory(
                        MemoryError::InconsistentMemory(
                            x,
                            y,
                            z
                        )
                    )) if x ==
        vm.run_context.get_ap() &&
                            y == MaybeRelocatable::from(Felt252::new(55i32)) &&
                            z == MaybeRelocatable::from(Felt252::new(1i32))
                );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn is_zero_assign_scope_variables_ok() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import SECP_P\nfrom starkware.python.math_utils import div_mod\n\nvalue = x_inv = div_mod(1, x, SECP_P)";
        let mut vm = vm_with_range_check!();

        //Initialize vm scope with variable `x`
        let mut exec_scopes = ExecutionScopes::new();
        exec_scopes.assign_or_update_variable(
            "x",
            any_box!(bigint_str!(
                "52621538839140286024584685587354966255185961783273479086367"
            )),
        );
        //Execute the hint
        assert_matches!(
            run_hint!(vm, HashMap::new(), hint_code, &mut exec_scopes),
            Ok(())
        );

        //Check 'value' is defined in the vm scope
        assert_matches!(
            exec_scopes.get::<BigInt>("value"),
            Ok(x) if x == bigint_str!(
                "19429627790501903254364315669614485084365347064625983303617500144471999752609"
            )
        );

        //Check 'x_inv' is defined in the vm scope
        assert_matches!(
            exec_scopes.get::<BigInt>("x_inv"),
            Ok(x) if x == bigint_str!(
                "19429627790501903254364315669614485084365347064625983303617500144471999752609"
            )
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn is_zero_assign_scope_variables_scope_error() {
        let hint_code = "from starkware.cairo.common.cairo_secp.secp_utils import SECP_P\nfrom starkware.python.math_utils import div_mod\n\nvalue = x_inv = div_mod(1, x, SECP_P)";
        let mut vm = vm_with_range_check!();
        //Skip `x` assignment
        //Execute the hint
        assert_matches!(
            run_hint!(
                vm,
                HashMap::new(),
                hint_code,
                exec_scopes_ref!()
            ),
            Err(HintError::VariableNotInScopeError(x)) if x == *"x".to_string()
        );
    }
}
