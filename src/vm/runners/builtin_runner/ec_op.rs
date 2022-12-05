use crate::math_utils::{ec_add, ec_double, safe_div_usize};
use crate::types::instance_definitions::ec_op_instance_def::{
    EcOpInstanceDef, CELLS_PER_EC_OP, INPUT_CELLS_PER_EC_OP,
};
use crate::types::relocatable::{MaybeRelocatable, Relocatable};
use crate::vm::errors::memory_errors::MemoryError;
use crate::vm::errors::runner_errors::RunnerError;
use crate::vm::vm_core::VirtualMachine;
use crate::vm::vm_memory::memory::Memory;
use crate::vm::vm_memory::memory_segments::MemorySegmentManager;
use crate::{bigint, bigint_str};
use num_bigint::BigInt;
use num_integer::{div_ceil, Integer};
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub struct EcOpBuiltinRunner {
    ratio: u32,
    pub base: isize,
    pub(crate) cells_per_instance: u32,
    pub(crate) n_input_cells: u32,
    ec_op_builtin: EcOpInstanceDef,
    pub(crate) stop_ptr: Option<usize>,
    _included: bool,
    instances_per_component: u32,
}

impl EcOpBuiltinRunner {
    pub(crate) fn new(instance_def: &EcOpInstanceDef, included: bool) -> Self {
        EcOpBuiltinRunner {
            base: 0,
            ratio: instance_def.ratio,
            n_input_cells: INPUT_CELLS_PER_EC_OP,
            cells_per_instance: CELLS_PER_EC_OP,
            ec_op_builtin: instance_def.clone(),
            stop_ptr: None,
            _included: included,
            instances_per_component: 1,
        }
    }
    ///Returns True if the point (x, y) is on the elliptic curve defined as
    ///y^2 = x^3 + alpha * x + beta (mod p)
    ///or False otherwise.
    fn point_on_curve(
        x: &BigInt,
        y: &BigInt,
        alpha: &BigInt,
        beta: &BigInt,
        prime: &BigInt,
    ) -> bool {
        (y.pow(2).mod_floor(prime)) == (x.pow(3) + alpha * x + beta).mod_floor(prime)
    }

    ///Returns the result of the EC operation P + m * Q.
    /// where P = (p_x, p_y), Q = (q_x, q_y) are points on the elliptic curve defined as
    /// y^2 = x^3 + alpha * x + beta (mod prime).
    /// Mimics the operation of the AIR, so that this function fails whenever the builtin AIR
    /// would not yield a correct result, i.e. when any part of the computation attempts to add
    /// two points with the same x coordinate.
    fn ec_op_impl(
        mut partial_sum: (BigInt, BigInt),
        mut doubled_point: (BigInt, BigInt),
        m: &BigInt,
        alpha: &BigInt,
        prime: &BigInt,
        height: u32,
    ) -> Result<(BigInt, BigInt), RunnerError> {
        let mut slope = m.clone();
        for _ in 0..height {
            if (doubled_point.0.clone() - partial_sum.0.clone()) % prime == bigint!(0) {
                return Err(RunnerError::EcOpSameXCoordinate(
                    partial_sum,
                    m.clone(),
                    doubled_point,
                ));
            };
            if slope.clone() & bigint!(1) != bigint!(0) {
                partial_sum = ec_add(partial_sum, doubled_point.clone(), prime);
            }
            doubled_point = ec_double(doubled_point, alpha, prime);
            slope = slope.clone() >> 1_i32;
        }
        Ok(partial_sum)
    }

    pub fn initialize_segments(
        &mut self,
        segments: &mut MemorySegmentManager,
        memory: &mut Memory,
    ) {
        self.base = segments.add(memory).segment_index
    }

    pub fn initial_stack(&self) -> Vec<MaybeRelocatable> {
        if self._included {
            vec![MaybeRelocatable::from((self.base, 0))]
        } else {
            vec![]
        }
    }

    pub fn base(&self) -> isize {
        self.base
    }

    pub fn ratio(&self) -> u32 {
        self.ratio
    }

    pub fn add_validation_rule(&self, _memory: &mut Memory) -> Result<(), RunnerError> {
        Ok(())
    }

    pub fn deduce_memory_cell(
        &self,
        address: &Relocatable,
        memory: &Memory,
    ) -> Result<Option<MaybeRelocatable>, RunnerError> {
        //Constant values declared here
        const EC_POINT_INDICES: [(usize, usize); 3] = [(0, 1), (2, 3), (5, 6)];
        const M_INDEX: usize = 4;
        const OUTPUT_INDICES: (usize, usize) = EC_POINT_INDICES[2];
        let alpha: BigInt = bigint!(1);
        let beta: BigInt = bigint_str!(
            b"3141592653589793238462643383279502884197169399375105820974944592307816406665"
        );
        let field_prime = bigint_str!(
            b"3618502788666131213697322783095070105623107215331596699973092056135872020481"
        );

        let index = address
            .offset
            .mod_floor(&(self.cells_per_instance as usize));
        //Index should be an output cell
        if index != OUTPUT_INDICES.0 && index != OUTPUT_INDICES.1 {
            return Ok(None);
        }
        let instance = MaybeRelocatable::from((address.segment_index, address.offset - index));
        //All input cells should be filled, and be integer values
        //If an input cell is not filled, return None
        let mut input_cells = Vec::<Cow<BigInt>>::with_capacity(self.n_input_cells as usize);
        for i in 0..self.n_input_cells as usize {
            match memory
                .get(&instance.add_usize_mod(i, None))
                .map_err(RunnerError::FailedMemoryGet)?
            {
                None => return Ok(None),
                Some(addr) => {
                    input_cells.push(match addr {
                        Cow::Borrowed(MaybeRelocatable::Int(num)) => Cow::Borrowed(num),
                        Cow::Owned(MaybeRelocatable::Int(num)) => Cow::Owned(num),
                        _ => {
                            return Err(RunnerError::ExpectedInteger(
                                instance.add_usize_mod(i, None),
                            ))
                        }
                    });
                }
            };
        }
        //Assert that m is under the limit defined by scalar_limit.
        if input_cells[M_INDEX].as_ref() >= &self.ec_op_builtin.scalar_limit {
            return Err(RunnerError::EcOpBuiltinScalarLimit(
                self.ec_op_builtin.scalar_limit.clone(),
            ));
        }

        // Assert that if the current address is part of a point, the point is on the curve
        for pair in &EC_POINT_INDICES[0..1] {
            if !EcOpBuiltinRunner::point_on_curve(
                input_cells[pair.0].as_ref(),
                input_cells[pair.1].as_ref(),
                &alpha,
                &beta,
                &field_prime,
            ) {
                return Err(RunnerError::PointNotOnCurve(*pair));
            };
        }
        let result = EcOpBuiltinRunner::ec_op_impl(
            (
                input_cells[0].to_owned().into_owned(),
                input_cells[1].to_owned().into_owned(),
            ),
            (
                input_cells[2].to_owned().into_owned(),
                input_cells[3].to_owned().into_owned(),
            ),
            input_cells[4].as_ref(),
            &alpha,
            &field_prime,
            self.ec_op_builtin.scalar_height,
        )?;
        match index - self.n_input_cells as usize {
            0 => Ok(Some(MaybeRelocatable::Int(result.0))),
            _ => Ok(Some(MaybeRelocatable::Int(result.1))),
            //Default case corresponds to 1, as there are no other possible cases
        }
    }

    pub fn get_allocated_memory_units(&self, vm: &VirtualMachine) -> Result<usize, MemoryError> {
        let value = safe_div_usize(vm.current_step, self.ratio as usize)
            .map_err(|_| MemoryError::ErrorCalculatingMemoryUnits)?;
        Ok(self.cells_per_instance as usize * value)
    }

    pub fn get_memory_segment_addresses(&self) -> (&'static str, (isize, Option<usize>)) {
        ("ec_op", (self.base, self.stop_ptr))
    }

    pub fn get_used_cells(&self, vm: &VirtualMachine) -> Result<usize, MemoryError> {
        let base = self.base();
        vm.segments
            .get_segment_used_size(
                base.try_into()
                    .map_err(|_| MemoryError::AddressInTemporarySegment(base))?,
            )
            .ok_or(MemoryError::MissingSegmentUsedSizes)
    }

    pub fn get_used_cells_and_allocated_size(
        &self,
        vm: &VirtualMachine,
    ) -> Result<(usize, usize), MemoryError> {
        let ratio = self.ratio as usize;
        let cells_per_instance = self.cells_per_instance;
        let min_step = ratio * self.instances_per_component as usize;
        if vm.current_step < min_step {
            Err(MemoryError::InsufficientAllocatedCells)
        } else {
            let used = self.get_used_cells(vm)?;
            let size = cells_per_instance as usize
                * safe_div_usize(vm.current_step, ratio)
                    .map_err(|_| MemoryError::InsufficientAllocatedCells)?;
            if used > size {
                return Err(MemoryError::InsufficientAllocatedCells);
            }
            Ok((used, size))
        }
    }

    pub fn get_used_instances(&self, vm: &VirtualMachine) -> Result<usize, MemoryError> {
        let used_cells = self.get_used_cells(vm)?;
        Ok(div_ceil(used_cells, self.cells_per_instance as usize))
    }

    pub fn final_stack(
        &self,
        vm: &VirtualMachine,
        pointer: Relocatable,
    ) -> Result<(Relocatable, usize), RunnerError> {
        if self._included {
            if let Ok(stop_pointer) = vm
                .get_relocatable(&(pointer.sub(1)).map_err(|_| RunnerError::FinalStack)?)
                .as_deref()
            {
                if self.base() != stop_pointer.segment_index {
                    return Err(RunnerError::InvalidStopPointer("ec_op".to_string()));
                }
                let stop_ptr = stop_pointer.offset;
                let num_instances = self
                    .get_used_instances(vm)
                    .map_err(|_| RunnerError::FinalStack)?;
                let used_cells = num_instances * self.cells_per_instance as usize;
                if stop_ptr != used_cells {
                    return Err(RunnerError::InvalidStopPointer("ec_op".to_string()));
                }

                Ok((
                    pointer.sub(1).map_err(|_| RunnerError::FinalStack)?,
                    stop_ptr,
                ))
            } else {
                Err(RunnerError::FinalStack)
            }
        } else {
            let stop_ptr = self.base() as usize;
            Ok((pointer, stop_ptr))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor;
    use crate::types::program::Program;
    use crate::utils::test_utils::*;
    use crate::vm::runners::cairo_runner::CairoRunner;
    use crate::vm::{
        errors::{memory_errors::MemoryError, runner_errors::RunnerError},
        runners::builtin_runner::BuiltinRunner,
        vm_core::VirtualMachine,
    };
    use num_bigint::Sign;

    #[test]
    fn get_used_instances() {
        let builtin = EcOpBuiltinRunner::new(&EcOpInstanceDef::new(10), true);

        let mut vm = vm!();

        vm.memory = memory![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((2, 0), (0, 0)),
            ((2, 1), (0, 0))
        ];

        vm.segments.segment_used_sizes = Some(vec![1]);

        assert_eq!(builtin.get_used_instances(&vm), Ok(1));
    }

    #[test]
    fn final_stack() {
        let builtin = EcOpBuiltinRunner::new(&EcOpInstanceDef::new(10), true);

        let mut vm = vm!();

        vm.memory = memory![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((2, 0), (0, 0)),
            ((2, 1), (0, 0))
        ];

        vm.segments.segment_used_sizes = Some(vec![0]);

        let pointer = Relocatable::from((2, 2));

        assert_eq!(
            builtin.final_stack(&vm, pointer).unwrap(),
            (Relocatable::from((2, 1)), 0)
        );
    }

    #[test]
    fn final_stack_error_stop_pointer() {
        let builtin = EcOpBuiltinRunner::new(&EcOpInstanceDef::new(10), true);

        let mut vm = vm!();

        vm.memory = memory![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((2, 0), (0, 0)),
            ((2, 1), (0, 0))
        ];

        vm.segments.segment_used_sizes = Some(vec![999]);

        let pointer = Relocatable::from((2, 2));

        assert_eq!(
            builtin.final_stack(&vm, pointer),
            Err(RunnerError::InvalidStopPointer("ec_op".to_string()))
        );
    }

    #[test]
    fn final_stack_error_when_not_included() {
        let builtin = EcOpBuiltinRunner::new(&EcOpInstanceDef::new(10), false);

        let mut vm = vm!();

        vm.memory = memory![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((2, 0), (0, 0)),
            ((2, 1), (0, 0))
        ];

        vm.segments.segment_used_sizes = Some(vec![0]);

        let pointer = Relocatable::from((2, 2));

        assert_eq!(
            builtin.final_stack(&vm, pointer).unwrap(),
            (Relocatable::from((2, 2)), 0)
        );
    }

    #[test]
    fn final_stack_error_non_relocatable() {
        let builtin = EcOpBuiltinRunner::new(&EcOpInstanceDef::new(10), true);

        let mut vm = vm!();

        vm.memory = memory![
            ((0, 0), (0, 0)),
            ((0, 1), (0, 1)),
            ((2, 0), (0, 0)),
            ((2, 1), 2)
        ];

        vm.segments.segment_used_sizes = Some(vec![0]);

        let pointer = Relocatable::from((2, 2));

        assert_eq!(
            builtin.final_stack(&vm, pointer),
            Err(RunnerError::FinalStack)
        );
    }

    #[test]
    fn get_used_cells_and_allocated_size_test() {
        let builtin: BuiltinRunner = EcOpBuiltinRunner::new(&EcOpInstanceDef::new(10), true).into();

        let mut vm = vm!();

        vm.segments.segment_used_sizes = Some(vec![0]);

        let program = program!(
            builtins = vec![String::from("pedersen")],
            data = vec_data!(
                (4612671182993129469_i64),
                (5189976364521848832_i64),
                (18446744073709551615_i128),
                (5199546496550207487_i64),
                (4612389712311386111_i64),
                (5198983563776393216_i64),
                (2),
                (2345108766317314046_i64),
                (5191102247248822272_i64),
                (5189976364521848832_i64),
                (7),
                (1226245742482522112_i64),
                ((
                    b"3618502788666131213697322783095070105623107215331596699973092056135872020470",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(8),
        );
        let mut cairo_runner = cairo_runner!(program);

        let hint_processor = BuiltinHintProcessor::new_empty();

        let address = cairo_runner.initialize(&mut vm).unwrap();

        cairo_runner
            .run_until_pc(address, &mut vm, &hint_processor)
            .unwrap();

        assert_eq!(builtin.get_used_cells_and_allocated_size(&vm), Ok((0, 7)));
    }

    #[test]
    fn get_allocated_memory_units() {
        let builtin = EcOpBuiltinRunner::new(&EcOpInstanceDef::new(10), true);

        let mut vm = vm!();

        let program = program!(
            builtins = vec![String::from("ec_op")],
            data = vec_data!(
                (4612671182993129469_i64),
                (5189976364521848832_i64),
                (18446744073709551615_i128),
                (5199546496550207487_i64),
                (4612389712311386111_i64),
                (5198983563776393216_i64),
                (2),
                (2345108766317314046_i64),
                (5191102247248822272_i64),
                (5189976364521848832_i64),
                (7),
                (1226245742482522112_i64),
                ((
                    b"3618502788666131213697322783095070105623107215331596699973092056135872020470",
                    10
                )),
                (2345108766317314046_i64)
            ),
            main = Some(8),
        );

        let mut cairo_runner = cairo_runner!(program);

        let hint_processor = BuiltinHintProcessor::new_empty();

        let address = cairo_runner.initialize(&mut vm).unwrap();

        cairo_runner
            .run_until_pc(address, &mut vm, &hint_processor)
            .unwrap();

        assert_eq!(builtin.get_allocated_memory_units(&vm), Ok(7));
    }

    #[test]
    fn point_is_on_curve_a() {
        let x = bigint_str!(
            b"874739451078007766457464989774322083649278607533249481151382481072868806602"
        );
        let y = bigint_str!(
            b"152666792071518830868575557812948353041420400780739481342941381225525861407"
        );
        let alpha = bigint!(1);
        let beta = bigint_str!(
            b"3141592653589793238462643383279502884197169399375105820974944592307816406665"
        );
        let prime = bigint_str!(
            b"3618502788666131213697322783095070105623107215331596699973092056135872020481"
        );
        assert!(EcOpBuiltinRunner::point_on_curve(
            &x, &y, &alpha, &beta, &prime
        ));
    }

    #[test]
    fn point_is_on_curve_b() {
        let x = bigint_str!(
            b"3139037544796708144595053687182055617920475701120786241351436619796497072089"
        );
        let y = bigint_str!(
            b"2119589567875935397690285099786081818522144748339117565577200220779667999801"
        );
        let alpha = bigint!(1);
        let beta = bigint_str!(
            b"3141592653589793238462643383279502884197169399375105820974944592307816406665"
        );
        let prime = bigint_str!(
            b"3618502788666131213697322783095070105623107215331596699973092056135872020481"
        );
        assert!(EcOpBuiltinRunner::point_on_curve(
            &x, &y, &alpha, &beta, &prime
        ));
    }

    #[test]
    fn point_is_not_on_curve_a() {
        let x = bigint_str!(
            b"874739454078007766457464989774322083649278607533249481151382481072868806602"
        );
        let y = bigint_str!(
            b"152666792071518830868575557812948353041420400780739481342941381225525861407"
        );
        let alpha = bigint!(1);
        let beta = bigint_str!(
            b"3141592653589793238462643383279502884197169399375105820974944592307816406665"
        );
        let prime = bigint_str!(
            b"3618502788666131213697322783095070105623107215331596699973092056135872020481"
        );
        assert!(!EcOpBuiltinRunner::point_on_curve(
            &x, &y, &alpha, &beta, &prime
        ));
    }

    #[test]
    fn point_is_not_on_curve_b() {
        let x = bigint_str!(
            b"3139037544756708144595053687182055617927475701120786241351436619796497072089"
        );
        let y = bigint_str!(
            b"2119589567875935397690885099786081818522144748339117565577200220779667999801"
        );
        let alpha = bigint!(1);
        let beta = bigint_str!(
            b"3141592653589793238462643383279502884197169399375105820974944592307816406665"
        );
        let prime = bigint_str!(
            b"3618502788666131213697322783095070105623107215331596699973092056135872020481"
        );
        assert!(!EcOpBuiltinRunner::point_on_curve(
            &x, &y, &alpha, &beta, &prime
        ));
    }

    #[test]
    fn compute_ec_op_impl_valid_a() {
        let partial_sum = (
            bigint_str!(
                b"3139037544796708144595053687182055617920475701120786241351436619796497072089"
            ),
            bigint_str!(
                b"2119589567875935397690285099786081818522144748339117565577200220779667999801"
            ),
        );
        let doubled_point = (
            bigint_str!(
                b"874739451078007766457464989774322083649278607533249481151382481072868806602"
            ),
            bigint_str!(
                b"152666792071518830868575557812948353041420400780739481342941381225525861407"
            ),
        );
        let m = bigint!(34);
        let alpha = bigint!(1);
        let height = 256;
        let prime = bigint_str!(
            b"3618502788666131213697322783095070105623107215331596699973092056135872020481"
        );
        let result =
            EcOpBuiltinRunner::ec_op_impl(partial_sum, doubled_point, &m, &alpha, &prime, height);
        assert_eq!(
            result,
            Ok((
                bigint_str!(
                    b"1977874238339000383330315148209250828062304908491266318460063803060754089297"
                ),
                bigint_str!(
                    b"2969386888251099938335087541720168257053975603483053253007176033556822156706"
                )
            ))
        );
    }

    #[test]
    fn compute_ec_op_impl_valid_b() {
        let partial_sum = (
            bigint_str!(
                b"2962412995502985605007699495352191122971573493113767820301112397466445942584"
            ),
            bigint_str!(
                b"214950771763870898744428659242275426967582168179217139798831865603966154129"
            ),
        );
        let doubled_point = (
            bigint_str!(
                b"874739451078007766457464989774322083649278607533249481151382481072868806602"
            ),
            bigint_str!(
                b"152666792071518830868575557812948353041420400780739481342941381225525861407"
            ),
        );
        let m = bigint!(34);
        let alpha = bigint!(1);
        let height = 256;
        let prime = bigint_str!(
            b"3618502788666131213697322783095070105623107215331596699973092056135872020481"
        );
        let result =
            EcOpBuiltinRunner::ec_op_impl(partial_sum, doubled_point, &m, &alpha, &prime, height);
        assert_eq!(
            result,
            Ok((
                bigint_str!(
                    b"2778063437308421278851140253538604815869848682781135193774472480292420096757"
                ),
                bigint_str!(
                    b"3598390311618116577316045819420613574162151407434885460365915347732568210029"
                )
            ))
        );
    }

    #[test]
    fn compute_ec_op_invalid_same_x_coordinate() {
        let partial_sum = (bigint!(1), bigint!(9));
        let doubled_point = (bigint!(1), bigint!(12));
        let m = bigint!(34);
        let alpha = bigint!(1);
        let height = 256;
        let prime = bigint_str!(
            b"3618502788666131213697322783095070105623107215331596699973092056135872020481"
        );
        let result = EcOpBuiltinRunner::ec_op_impl(
            partial_sum.clone(),
            doubled_point.clone(),
            &m,
            &alpha,
            &prime,
            height,
        );
        assert_eq!(
            result,
            Err(RunnerError::EcOpSameXCoordinate(
                partial_sum,
                m,
                doubled_point
            ))
        );
    }

    #[test]
    /* Data taken from this program execution:
       %builtins output ec_op
       from starkware.cairo.common.cairo_builtins import EcOpBuiltin
       from starkware.cairo.common.serialize import serialize_word
       from starkware.cairo.common.ec_point import EcPoint
       from starkware.cairo.common.ec import ec_op

       func main{output_ptr: felt*, ec_op_ptr: EcOpBuiltin*}():
           let x: EcPoint = EcPoint(2089986280348253421170679821480865132823066470938446095505822317253594081284, 1713931329540660377023406109199410414810705867260802078187082345529207694986)

           let y: EcPoint = EcPoint(874739451078007766457464989774322083649278607533249481151382481072868806602,152666792071518830868575557812948353041420400780739481342941381225525861407)
           let z: EcPoint = ec_op(x,34, y)
           serialize_word(z.x)
           return()
           end
    */
    fn deduce_memory_cell_ec_op_for_preset_memory_valid() {
        let memory = memory![
            (
                (3, 0),
                (
                    b"2962412995502985605007699495352191122971573493113767820301112397466445942584",
                    10
                )
            ),
            (
                (3, 1),
                (
                    b"214950771763870898744428659242275426967582168179217139798831865603966154129",
                    10
                )
            ),
            (
                (3, 2),
                (
                    b"874739451078007766457464989774322083649278607533249481151382481072868806602",
                    10
                )
            ),
            (
                (3, 3),
                (
                    b"152666792071518830868575557812948353041420400780739481342941381225525861407",
                    10
                )
            ),
            ((3, 4), 34),
            (
                (3, 5),
                (
                    b"2778063437308421278851140253538604815869848682781135193774472480292420096757",
                    10
                )
            )
        ];
        let builtin = EcOpBuiltinRunner::new(&EcOpInstanceDef::default(), true);

        let result = builtin.deduce_memory_cell(&Relocatable::from((3, 6)), &memory);
        assert_eq!(
            result,
            Ok(Some(MaybeRelocatable::from(bigint_str!(
                b"3598390311618116577316045819420613574162151407434885460365915347732568210029"
            ))))
        );
    }

    #[test]
    fn deduce_memory_cell_ec_op_for_preset_memory_unfilled_input_cells() {
        let memory = memory![
            (
                (3, 1),
                (
                    b"214950771763870898744428659242275426967582168179217139798831865603966154129",
                    10
                )
            ),
            (
                (3, 2),
                (
                    b"874739451078007766457464989774322083649278607533249481151382481072868806602",
                    10
                )
            ),
            (
                (3, 3),
                (
                    b"152666792071518830868575557812948353041420400780739481342941381225525861407",
                    10
                )
            ),
            ((3, 4), 34),
            (
                (3, 5),
                (
                    b"2778063437308421278851140253538604815869848682781135193774472480292420096757",
                    10
                )
            )
        ];

        let builtin = EcOpBuiltinRunner::new(&EcOpInstanceDef::default(), true);
        let result = builtin.deduce_memory_cell(&Relocatable::from((3, 6)), &memory);
        assert_eq!(result, Ok(None));
    }

    #[test]
    fn deduce_memory_cell_ec_op_for_preset_memory_addr_not_an_output_cell() {
        let memory = memory![
            (
                (3, 0),
                (
                    b"2962412995502985605007699495352191122971573493113767820301112397466445942584",
                    10
                )
            ),
            (
                (3, 1),
                (
                    b"214950771763870898744428659242275426967582168179217139798831865603966154129",
                    10
                )
            ),
            (
                (3, 2),
                (
                    b"874739451078007766457464989774322083649278607533249481151382481072868806602",
                    10
                )
            ),
            (
                (3, 3),
                (
                    b"152666792071518830868575557812948353041420400780739481342941381225525861407",
                    10
                )
            ),
            ((3, 4), 34),
            (
                (3, 5),
                (
                    b"2778063437308421278851140253538604815869848682781135193774472480292420096757",
                    10
                )
            )
        ];
        let builtin = EcOpBuiltinRunner::new(&EcOpInstanceDef::default(), true);

        let result = builtin.deduce_memory_cell(&Relocatable::from((3, 3)), &memory);
        assert_eq!(result, Ok(None));
    }

    #[test]
    fn deduce_memory_cell_ec_op_for_preset_memory_non_integer_input() {
        let memory = memory![
            (
                (3, 0),
                (
                    b"2962412995502985605007699495352191122971573493113767820301112397466445942584",
                    10
                )
            ),
            (
                (3, 1),
                (
                    b"214950771763870898744428659242275426967582168179217139798831865603966154129",
                    10
                )
            ),
            (
                (3, 2),
                (
                    b"874739451078007766457464989774322083649278607533249481151382481072868806602",
                    10
                )
            ),
            ((3, 3), (1, 2)),
            ((3, 4), 34),
            (
                (3, 5),
                (
                    b"2778063437308421278851140253538604815869848682781135193774472480292420096757",
                    10
                )
            )
        ];
        let builtin = EcOpBuiltinRunner::new(&EcOpInstanceDef::default(), true);

        assert_eq!(
            builtin.deduce_memory_cell(&Relocatable::from((3, 6)), &memory),
            Err(RunnerError::ExpectedInteger(MaybeRelocatable::from((3, 3))))
        );
    }

    #[test]
    fn deduce_memory_cell_ec_op_for_preset_memory_m_over_scalar_limit() {
        let memory = memory![
            (
                (3, 0),
                (
                    b"2962412995502985605007699495352191122971573493113767820301112397466445942584",
                    10
                )
            ),
            (
                (3, 1),
                (
                    b"214950771763870898744428659242275426967582168179217139798831865603966154129",
                    10
                )
            ),
            (
                (3, 2),
                (
                    b"874739451078007766457464989774322083649278607533249481151382481072868806602",
                    10
                )
            ),
            (
                (3, 3),
                (
                    b"152666792071518830868575557812948353041420400780739481342941381225525861407",
                    10
                )
            ),
            //Scalar Limit + 1
            (
                (3, 4),
                (
                    b"3618502788666131213697322783095070105623107215331596699973092056135872020482",
                    10
                )
            ),
            (
                (3, 5),
                (
                    b"2778063437308421278851140253538604815869848682781135193774472480292420096757",
                    10
                )
            )
        ];
        let builtin = EcOpBuiltinRunner::new(&EcOpInstanceDef::default(), true);

        let error = builtin.deduce_memory_cell(&Relocatable::from((3, 6)), &memory);
        assert_eq!(
            error,
            Err(RunnerError::EcOpBuiltinScalarLimit(
                builtin.ec_op_builtin.scalar_limit.clone()
            ))
        );
    }

    #[test]
    fn get_memory_segment_addresses() {
        let builtin = EcOpBuiltinRunner::new(&EcOpInstanceDef::default(), true);

        assert_eq!(builtin.get_memory_segment_addresses(), ("ec_op", (0, None)));
    }

    #[test]
    fn get_memory_accesses_missing_segment_used_sizes() {
        let builtin =
            BuiltinRunner::EcOp(EcOpBuiltinRunner::new(&EcOpInstanceDef::default(), true));
        let vm = vm!();

        assert_eq!(
            builtin.get_memory_accesses(&vm),
            Err(MemoryError::MissingSegmentUsedSizes),
        );
    }

    #[test]
    fn get_memory_accesses_empty() {
        let builtin =
            BuiltinRunner::EcOp(EcOpBuiltinRunner::new(&EcOpInstanceDef::default(), true));
        let mut vm = vm!();

        vm.segments.segment_used_sizes = Some(vec![0]);
        assert_eq!(builtin.get_memory_accesses(&vm), Ok(vec![]));
    }

    #[test]
    fn get_memory_accesses() {
        let builtin =
            BuiltinRunner::EcOp(EcOpBuiltinRunner::new(&EcOpInstanceDef::default(), true));
        let mut vm = vm!();

        vm.segments.segment_used_sizes = Some(vec![4]);
        assert_eq!(
            builtin.get_memory_accesses(&vm),
            Ok(vec![
                (builtin.base(), 0).into(),
                (builtin.base(), 1).into(),
                (builtin.base(), 2).into(),
                (builtin.base(), 3).into(),
            ]),
        );
    }

    #[test]
    fn get_used_cells_missing_segment_used_sizes() {
        let builtin =
            BuiltinRunner::EcOp(EcOpBuiltinRunner::new(&EcOpInstanceDef::default(), true));
        let vm = vm!();

        assert_eq!(
            builtin.get_used_cells(&vm),
            Err(MemoryError::MissingSegmentUsedSizes)
        );
    }

    #[test]
    fn get_used_cells_empty() {
        let builtin =
            BuiltinRunner::EcOp(EcOpBuiltinRunner::new(&EcOpInstanceDef::default(), true));
        let mut vm = vm!();

        vm.segments.segment_used_sizes = Some(vec![0]);
        assert_eq!(builtin.get_used_cells(&vm), Ok(0));
    }

    #[test]
    fn get_used_cells() {
        let builtin =
            BuiltinRunner::EcOp(EcOpBuiltinRunner::new(&EcOpInstanceDef::default(), true));
        let mut vm = vm!();

        vm.segments.segment_used_sizes = Some(vec![4]);
        assert_eq!(builtin.get_used_cells(&vm), Ok(4));
    }

    #[test]
    fn initial_stack_included_test() {
        let ec_op_builtin = EcOpBuiltinRunner::new(&EcOpInstanceDef::default(), true);
        assert_eq!(ec_op_builtin.initial_stack(), vec![mayberelocatable!(0, 0)])
    }

    #[test]
    fn initial_stack_not_included_test() {
        let ec_op_builtin = EcOpBuiltinRunner::new(&EcOpInstanceDef::default(), false);
        assert_eq!(ec_op_builtin.initial_stack(), Vec::new())
    }
}
