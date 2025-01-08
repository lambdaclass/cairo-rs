use crate::Felt252;
use crate::{
    hint_processor::{
        builtin_hint_processor::{
            hint_utils::{insert_value_from_var_name, insert_value_into_ap},
            secp::{bigint_utils::Uint384, secp_utils::SECP_P},
        },
        hint_processor_definition::HintReference,
    },
    math_utils::div_mod,
    serde::deserialize_program::ApTracking,
    stdlib::{boxed::Box, collections::HashMap, prelude::*},
    types::exec_scope::ExecutionScopes,
    vm::{errors::hint_errors::HintError, vm_core::VirtualMachine},
};
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Zero};

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
    secp_p: &BigInt,
) -> Result<(), HintError> {
    exec_scopes.insert_value("SECP_P", secp_p.clone());
    let val = Uint384::from_var_name("val", vm, ids_data, ap_tracking)?.pack86();
    let (q, r) = val.div_mod_floor(secp_p);
    if !r.is_zero() {
        return Err(HintError::SecpVerifyZero(Box::new(val)));
    }

    insert_value_from_var_name("q", Felt252::from(&q), vm, ids_data, ap_tracking)
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
    let val = Uint384::from_var_name("val", vm, ids_data, ap_tracking)?.pack86();
    let (q, r) = val.div_mod_floor(secp_p);
    if !r.is_zero() {
        return Err(HintError::SecpVerifyZero(Box::new(val)));
    }

    insert_value_from_var_name("q", Felt252::from(&q), vm, ids_data, ap_tracking)
}

/*
Implements hint:
%{
    from starkware.cairo.common.cairo_secp.secp_utils import SECP_P, pack

    value = pack(ids.x, PRIME) % SECP_P
%}
*/
pub fn reduce_v1(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    exec_scopes.insert_value("SECP_P", SECP_P.clone());
    let value = Uint384::from_var_name("x", vm, ids_data, ap_tracking)?.pack86();
    exec_scopes.insert_value("value", value.mod_floor(&SECP_P));
    Ok(())
}

/*
Implements hint:
%{
    from starkware.cairo.common.cairo_secp.secp_utils import pack
    value = pack(ids.x, PRIME) % SECP_P
%}
*/
pub fn reduce_v2(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let secp_p = exec_scopes.get_ref("SECP_P")?;
    let value = Uint384::from_var_name("x", vm, ids_data, ap_tracking)?.pack86();
    exec_scopes.insert_value("value", value.mod_floor(secp_p));
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
    exec_scopes.insert_value("SECP_P", SECP_P.clone());
    let x_packed = Uint384::from_var_name("x", vm, ids_data, ap_tracking)?.pack86();
    let x = x_packed.mod_floor(&SECP_P);
    exec_scopes.insert_value("x", x);
    Ok(())
}

pub fn is_zero_pack_external_secp(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
) -> Result<(), HintError> {
    let secp_p = exec_scopes.get_ref("SECP_P")?;
    let x_packed = Uint384::from_var_name("x", vm, ids_data, ap_tracking)?.pack86();
    let x = x_packed.mod_floor(secp_p);
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
        Felt252::ONE
    } else {
        Felt252::ZERO
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
    exec_scopes.insert_value("SECP_P", SECP_P.clone());
    //Get `x` variable from vm scope
    let x = exec_scopes.get::<BigInt>("x")?;

    let value = div_mod(&BigInt::one(), &x, &SECP_P)?;
    exec_scopes.insert_value("value", value.clone());
    exec_scopes.insert_value("x_inv", value);
    Ok(())
}

/*
Implements hint:
%{
    from starkware.python.math_utils import div_mod

    value = x_inv = div_mod(1, x, SECP_P)
%}
*/
pub fn is_zero_assign_scope_variables_external_const(
    exec_scopes: &mut ExecutionScopes,
) -> Result<(), HintError> {
    //Get variables from vm scope
    let secp_p = exec_scopes.get_ref::<BigInt>("SECP_P")?;
    let x = exec_scopes.get_ref::<BigInt>("x")?;

    let value = div_mod(&BigInt::one(), x, secp_p)?;
    exec_scopes.insert_value("value", value.clone());
    exec_scopes.insert_value("x_inv", value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hint_processor::builtin_hint_processor::hint_code::{self, REDUCE_V2};
    use crate::stdlib::string::ToString;

    use crate::{
        any_box,
        hint_processor::{
            builtin_hint_processor::builtin_hint_processor_definition::{
                BuiltinHintProcessor, HintProcessorData,
            },
            hint_processor_definition::HintProcessorLogic,
        },
        types::{
            exec_scope::ExecutionScopes,
            relocatable::{MaybeRelocatable, Relocatable},
        },
        utils::test_utils::*,
        vm::errors::memory_errors::MemoryError,
    };
    use assert_matches::assert_matches;

    use rstest::rstest;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_verify_zero_ok() {
        let hint_codes = vec![
            &hint_code::VERIFY_ZERO_V1,
            &hint_code::VERIFY_ZERO_V2,
            &hint_code::VERIFY_ZERO_V3,
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
    fn run_verify_zero_v3_ok() {
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
            Err(HintError::SecpVerifyZero(bx)) if *bx == bigint_str!(
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
                MemoryError::InconsistentMemory(bx)
            )) if *bx == (Relocatable::from((1, 9)),
                    MaybeRelocatable::from(Felt252::from(55_i32)),
                    MaybeRelocatable::from(Felt252::ZERO))
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
            Err(HintError::IdentifierHasNoMember(bx))
            if *bx == ("x".to_string(), "d0".to_string())
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_reduce_v2_ok() {
        let hint_code = REDUCE_V2;
        let mut vm = vm_with_range_check!();
        add_segments!(vm, 3);

        //Initialize fp
        vm.run_context.fp = 25;

        //Create hint data
        let ids_data = non_continuous_ids_data![("x", -5)];

        vm.segments = segments![
            ((1, 20), ("132181232131231239112312312313213083892150", 10)),
            ((1, 21), ("12354812987893128791212331231233", 10)),
            ((1, 22), ("654867675805132187", 10))
        ];

        let mut exec_scopes = ExecutionScopes::new();
        exec_scopes.insert_value("SECP_P", SECP_P.clone());
        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Ok(()));

        //Check 'value' is defined in the vm scope
        assert_matches!(
            exec_scopes.get::<BigInt>("value"),
            Ok(x) if x == bigint_str!(
                "3920241379018821570896271640300310233395357371896837069219347149797814"
            )
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_reduce_v2_with_no_secp() {
        let hint_code = REDUCE_V2;
        let mut vm = vm_with_range_check!();
        add_segments!(vm, 3);

        //Initialize fp
        vm.run_context.fp = 25;

        //Create hint data
        let ids_data = non_continuous_ids_data![("x", -5)];

        vm.segments = segments![((1, 20), 1), ((1, 21), 1), ((1, 22), 1)];

        let mut exec_scopes = ExecutionScopes::new();
        // Skip the SECP definition, so the hint fails
        // exec_scopes.insert_value("SECP_P", SECP_P.clone());
        //Execute the hint
        assert_matches!(run_hint!(vm, ids_data, hint_code, &mut exec_scopes), Err(_));
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_is_zero_pack_ok() {
        let mut exec_scopes = ExecutionScopes::new();
        let hint_codes = vec![
            hint_code::IS_ZERO_PACK_V1,
            hint_code::IS_ZERO_PACK_V2,
            // NOTE: this one requires IS_ZERO_ASSIGN_SCOPE_VARS to execute first.
            hint_code::IS_ZERO_PACK_EXTERNAL_SECP_V1,
            hint_code::IS_ZERO_PACK_EXTERNAL_SECP_V2,
        ];
        for hint_code in hint_codes {
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
            Err(HintError::IdentifierHasNoMember(bx))
            if *bx == ("x".to_string(), "d0".to_string())
        );
    }

    #[rstest]
    #[case(hint_code::IS_ZERO_NONDET)]
    #[case(hint_code::IS_ZERO_INT)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_is_zero_nondet_ok_true(#[case] hint_code: &str) {
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

    #[rstest]
    #[case(hint_code::IS_ZERO_NONDET)]
    #[case(hint_code::IS_ZERO_INT)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_is_zero_nondet_ok_false(#[case] hint_code: &str) {
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

    #[rstest]
    #[case(hint_code::IS_ZERO_NONDET)]
    #[case(hint_code::IS_ZERO_INT)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_is_zero_nondet_scope_error(#[case] hint_code: &str) {
        let mut vm = vm_with_range_check!();

        //Initialize memory
        add_segments!(vm, 2);

        //Initialize ap
        vm.run_context.ap = 15;

        //Skip `x` assignment

        //Execute the hint
        assert_matches!(
            run_hint!(vm, HashMap::new(), hint_code),
            Err(HintError::VariableNotInScopeError(bx)) if bx.as_ref() == "x"
        );
    }

    #[rstest]
    #[case(hint_code::IS_ZERO_NONDET)]
    #[case(hint_code::IS_ZERO_INT)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn run_is_zero_nondet_invalid_memory_insert(#[case] hint_code: &str) {
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
                MemoryError::InconsistentMemory(bx)
            )) if *bx == (vm.run_context.get_ap(),
                MaybeRelocatable::from(Felt252::from(55i32)),
                MaybeRelocatable::from(Felt252::from(1i32)))
        );
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn is_zero_assign_scope_variables_ok() {
        let mut exec_scopes = ExecutionScopes::new();
        let hint_codes = vec![
            hint_code::IS_ZERO_ASSIGN_SCOPE_VARS,
            // NOTE: this one requires IS_ZERO_ASSIGN_SCOPE_VARS to execute first.
            hint_code::IS_ZERO_ASSIGN_SCOPE_VARS_EXTERNAL_SECP,
        ];

        for hint_code in hint_codes {
            let mut vm = vm_with_range_check!();

            //Initialize vm scope with variable `x`
            exec_scopes.assign_or_update_variable(
                "x",
                any_box!(bigint_str!(
                    "52621538839140286024584685587354966255185961783273479086367"
                )),
            );
            //Execute the hint
            assert!(run_hint!(vm, HashMap::new(), hint_code, &mut exec_scopes).is_ok());

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
    }

    #[test]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn is_zero_assign_scope_variables_scope_error() {
        let hint_code = hint_code::IS_ZERO_ASSIGN_SCOPE_VARS;
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
            Err(HintError::VariableNotInScopeError(bx)) if bx.as_ref() == "x"
        );
    }
}
