use crate::circuits::Tensor;
use stwo_prover::core::backend::Column;
use stwo_prover::core::{
    backend::{
        simd::{m31::LOG_N_LANES, SimdBackend},
        Col,
    },
    fields::m31::BaseField,
    poly::{
        circle::{CanonicCoset, CircleEvaluation},
        BitReversedOrder,
    },
    ColumnVec,
};

pub fn generate_trace(
    log_size: u32,
    a: Tensor,
    b: Tensor,
) -> (
    ColumnVec<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
    Tensor,
) {
    assert!(a.is_broadcastable_with(&b), "Tensors must be broadcastable");

    // Calculate required trace size
    let max_size = a.size().max(b.size());
    assert!(log_size >= LOG_N_LANES);

    // Initialize trace columns

    let trace_size = 1 << log_size;
    let mut trace: Vec<Col<SimdBackend, BaseField>> = Vec::with_capacity(3);
    let mut c_data = Vec::with_capacity((max_size + (1 << LOG_N_LANES) - 1) >> LOG_N_LANES);
    for _ in 0..3 {
        trace.push(Col::<SimdBackend, BaseField>::zeros(trace_size));
    }

    // Calculate number of SIMD-packed rows needed for each tensor
    let n_rows = 1 << (log_size - LOG_N_LANES);
    let a_packed_size = (a.size() + (1 << LOG_N_LANES) - 1) >> LOG_N_LANES;
    let b_packed_size = (b.size() + (1 << LOG_N_LANES) - 1) >> LOG_N_LANES;

    // Fill trace with tensor data
    // Process in chunks for better cache utilization
    const CHUNK_SIZE: usize = 64;
    for chunk in (0..n_rows).step_by(CHUNK_SIZE) {
        let end = (chunk + CHUNK_SIZE).min(n_rows);

        for vec_row in chunk..end {
            if vec_row < max_size {
                // Calculate the packed indices with broadcasting
                let a_idx = vec_row % a_packed_size;
                let b_idx = vec_row % b_packed_size;

                let sum = a.data[a_idx] + b.data[b_idx];

                trace[0].data[vec_row] = a.data[a_idx];
                trace[1].data[vec_row] = b.data[b_idx];
                trace[2].data[vec_row] = sum;

                c_data.push(sum);
            }
        }
    }

    // Create output tensor C
    let c = Tensor {
        data: c_data,
        dims: if a.size() > b.size() {
            a.dims.clone()
        } else {
            b.dims.clone()
        },
        stride: Tensor::compute_stride(if a.size() > b.size() {
            &a.dims
        } else {
            &b.dims
        }),
    };

    let domain = CanonicCoset::new(log_size).circle_domain();

    (
        trace
            .into_iter()
            .map(|eval| CircleEvaluation::new(domain, eval))
            .collect(),
        c,
    )
}
