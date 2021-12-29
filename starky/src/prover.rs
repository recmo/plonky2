use plonky2::field::extension_field::Extendable;
use plonky2::field::polynomial::PolynomialValues;
use plonky2::fri::commitment::PolynomialBatchCommitment;
use plonky2::fri::prover::fri_proof;
use plonky2::hash::hash_types::RichField;
use plonky2::plonk::config::GenericConfig;
use plonky2::timed;
use plonky2::util::timing::TimingTree;
use plonky2::util::transpose;
use rayon::prelude::*;

use crate::proof::StarkProof;
use crate::stark::Stark;

pub fn prove<
    F: RichField + Extendable<D>,
    C: GenericConfig<D, F = F>,
    S: Stark<F, D>,
    const D: usize,
>(
    timing: &mut TimingTree,
) -> StarkProof<F, C, D> {
    let witness_row_major: Vec<Vec<F>> = todo!();
    let witness_col_major: Vec<Vec<F>> = transpose(&witness_row_major);

    let trace_poly_values: Vec<PolynomialValues<F>> = timed!(
        timing,
        "compute trace polynomials",
        witness_col_major
            .par_iter()
            .map(|column| PolynomialValues::new(column.clone()))
            .collect()
    );

    let rate_bits = todo!();
    let cap_height = todo!();
    let trace_commitment = timed!(
        timing,
        "compute trace commitment",
        PolynomialBatchCommitment::<F, C, D>::from_values(
            trace_poly_values,
            rate_bits,
            false,
            cap_height,
            timing,
            None,
        )
    );

    let trace_cap = trace_commitment.merkle_tree.cap;
    let openings = todo!();

    let initial_merkle_trees = todo!();
    let lde_polynomial_coeffs = todo!();
    let lde_polynomial_values = todo!();
    let challenger = todo!();
    let fri_config = todo!();

    let opening_proof = fri_proof::<F, C, D>(
        initial_merkle_trees,
        lde_polynomial_coeffs,
        lde_polynomial_values,
        challenger,
        fri_config,
        timing,
    );

    StarkProof {
        trace_cap,
        openings,
        opening_proof,
    }
}
