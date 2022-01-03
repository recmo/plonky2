use std::marker::PhantomData;

use crate::field::extension_field::Extendable;
use crate::field::field_types::RichField;
use crate::gates::binary_arithmetic::BinaryArithmeticGate;
use crate::gates::binary_subtraction::BinarySubtractionGate;
use crate::iop::generator::{SimpleGenerator, GeneratedValues};
use crate::iop::target::Target;
use crate::iop::witness::{PartitionWitness, Witness};
use crate::plonk::circuit_builder::CircuitBuilder;

#[derive(Clone, Copy, Debug)]
pub struct BinaryTarget<const BITS: usize>(pub Target);

impl<F: RichField + Extendable<D>, const D: usize> CircuitBuilder<F, D> {
    pub fn add_virtual_binary_target<const BITS: usize>(&mut self) -> BinaryTarget<BITS> {
        BinaryTarget(self.add_virtual_target())
    }

    pub fn add_virtual_binary_targets<const BITS: usize>(&mut self, n: usize) -> Vec<BinaryTarget<BITS>> {
        self.add_virtual_targets(n)
            .into_iter()
            .map(BinaryTarget)
            .collect()
    }

    pub fn zero_binary<const BITS: usize>(&mut self) -> BinaryTarget<BITS> {
        BinaryTarget(self.zero())
    }

    pub fn one_binary<const BITS: usize>(&mut self) -> BinaryTarget<BITS> {
        BinaryTarget(self.one())
    }

    pub fn connect_binary<const BITS: usize>(&mut self, x: BinaryTarget<BITS>, y: BinaryTarget<BITS>) {
        self.connect(x.0, y.0)
    }

    pub fn assert_zero_binary<const BITS: usize>(&mut self, x: BinaryTarget<BITS>) {
        self.assert_zero(x.0)
    }

    /// Checks for special cases where the value of
    /// `x * y + z`
    /// can be determined without adding a `BinaryArithmeticGate`.
    pub fn arithmetic_binary_special_cases<const BITS: usize>(
        &mut self,
        x: BinaryTarget<BITS>,
        y: BinaryTarget<BITS>,
        z: BinaryTarget<BITS>,
    ) -> Option<(BinaryTarget<BITS>, BinaryTarget<BITS>)> {
        let x_const = self.target_as_constant(x.0);
        let y_const = self.target_as_constant(y.0);
        let z_const = self.target_as_constant(z.0);

        // If both terms are constant, return their (constant) sum.
        let first_term_const = if let (Some(xx), Some(yy)) = (x_const, y_const) {
            Some(xx * yy)
        } else {
            None
        };

        if let (Some(a), Some(b)) = (first_term_const, z_const) {
            let sum = (a + b).to_canonical_u64();
            let base = 1u64 << BITS;
            let (low, high) = (sum % base, (sum >> BITS) % base);
            let low_F = F::from_canonical_u64(low);
            let high_F = F::from_canonical_u64(high);
            
            return Some((
                BinaryTarget::<BITS>(self.constant(low_F)),
                BinaryTarget::<BITS>(self.constant(high_F)),
            ));
        }

        None
    }

    // Returns x * y + z.
    pub fn mul_add_binary<const BITS: usize>(
        &mut self,
        x: BinaryTarget<BITS>,
        y: BinaryTarget<BITS>,
        z: BinaryTarget<BITS>,
    ) -> (BinaryTarget<BITS>, BinaryTarget<BITS>) {
        if let Some(result) = self.arithmetic_binary_special_cases(x, y, z) {
            return result;
        }

        let gate = BinaryArithmeticGate::<F, D, BITS>::new_from_config(&self.config);
        let (gate_index, copy) = self.find_binary_arithmetic_gate::<BITS>();

        self.connect(
            Target::wire(gate_index, gate.wire_ith_multiplicand_0(copy)),
            x.0,
        );
        self.connect(
            Target::wire(gate_index, gate.wire_ith_multiplicand_1(copy)),
            y.0,
        );
        self.connect(Target::wire(gate_index, gate.wire_ith_addend(copy)), z.0);

        let output_low = BinaryTarget(Target::wire(
            gate_index,
            gate.wire_ith_output_low_half(copy),
        ));
        let output_high = BinaryTarget(Target::wire(
            gate_index,
            gate.wire_ith_output_high_half(copy),
        ));

        (output_low, output_high)
    }

    pub fn add_binary<const BITS: usize>(&mut self, a: BinaryTarget<BITS>, b: BinaryTarget<BITS>) -> (BinaryTarget<BITS>, BinaryTarget<BITS>) {
        let one = self.one_binary();
        self.mul_add_binary(a, one, b)
    }

    pub fn add_many_binary<const BITS: usize>(&mut self, to_add: &[BinaryTarget<BITS>]) -> (BinaryTarget<BITS>, BinaryTarget<BITS>) {
        match to_add.len() {
            0 => (self.zero_binary(), self.zero_binary()),
            1 => (to_add[0], self.zero_binary()),
            2 => self.add_binary(to_add[0], to_add[1]),
            _ => {
                let (mut low, mut carry) = self.add_binary(to_add[0], to_add[1]);
                for i in 2..to_add.len() {
                    let (new_low, new_carry) = self.add_binary(to_add[i], low);
                    let (combined_carry, _zero) = self.add_binary(carry, new_carry);
                    low = new_low;
                    carry = combined_carry;
                }
                (low, carry)
            }
        }
    }

    pub fn mul_binary<const BITS: usize>(&mut self, a: BinaryTarget<BITS>, b: BinaryTarget<BITS>) -> (BinaryTarget<BITS>, BinaryTarget<BITS>) {
        let zero = self.zero_binary();
        self.mul_add_binary(a, b, zero)
    }

    // Returns x - y - borrow, as a pair (result, borrow), where borrow is 0 or 1 depending on whether borrowing from the next digit is required (iff y + borrow > x).
    pub fn sub_binary<const BITS: usize>(
        &mut self,
        x: BinaryTarget<BITS>,
        y: BinaryTarget<BITS>,
        borrow: BinaryTarget<BITS>,
    ) -> (BinaryTarget<BITS>, BinaryTarget<BITS>) {
        let gate = BinarySubtractionGate::<F, D, BITS>::new_from_config(&self.config);
        let (gate_index, copy) = self.find_binary_subtraction_gate::<BITS>();

        self.connect(Target::wire(gate_index, gate.wire_ith_input_x(copy)), x.0);
        self.connect(Target::wire(gate_index, gate.wire_ith_input_y(copy)), y.0);
        self.connect(
            Target::wire(gate_index, gate.wire_ith_input_borrow(copy)),
            borrow.0,
        );

        let output_result = BinaryTarget(Target::wire(gate_index, gate.wire_ith_output_result(copy)));
        let output_borrow = BinaryTarget(Target::wire(gate_index, gate.wire_ith_output_borrow(copy)));

        (output_result, output_borrow)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    
    use rand::{thread_rng, Rng};

    use crate::field::goldilocks_field::GoldilocksField;
    use crate::field::field_types::Field;
    use crate::iop::witness::PartialWitness;
    use crate::plonk::circuit_builder::CircuitBuilder;
    use crate::plonk::circuit_data::CircuitConfig;
    use crate::plonk::verifier::verify;

    #[test]
    pub fn test_add_many_binarys() -> Result<()> {
        type F = GoldilocksField;
        const D: usize = 4;
        const BITS: usize = 30;

        let config = CircuitConfig::standard_recursion_config();

        let pw = PartialWitness::new();
        let mut builder = CircuitBuilder::<F, D>::new(config);

        let mut rng = thread_rng();
        let mut to_add = Vec::new();
        for _ in 0..10 {
            let c = rng.gen::<u64>() % (1 << BITS);
            to_add.push(builder.constant_binary::<BITS>(F::from_canonical_u64(c)));
        }
        let _ = builder.add_many_binary(&to_add);

        let data = builder.build();
        let proof = data.prove(pw).unwrap();
        verify(proof, &data.verifier_only, &data.common)
    }
}