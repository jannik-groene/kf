use crate::nnue::layers::LinearLayer;

#[cfg(target_arch="x86_64")]
#[target_feature(enable="avx")]
#[target_feature(enable="avx2")]
#[inline]
pub unsafe fn affine_transform<const INPUTS: usize, const OUTPUTS: usize>(ll: &LinearLayer<INPUTS, OUTPUTS>, inputs: &[i8; INPUTS]) -> [i32; OUTPUTS] {
    assert!(INPUTS % 32 == 0);
    assert!(OUTPUTS % 4 == 0);
    use core::arch::x86_64::*;
    use std::mem::transmute;
    let mut outputs = [0; OUTPUTS];

    let ones: __m256i = _mm256_set1_epi16(1);

    for j in 0..OUTPUTS/4 {

        let mut sum0: __m256i = _mm256_setzero_si256();
        let mut sum1: __m256i = _mm256_setzero_si256();
        let mut sum2: __m256i = _mm256_setzero_si256();
        let mut sum3: __m256i = _mm256_setzero_si256();

        for i in 0..INPUTS/32 {
            let inputs_slice = _mm256_loadu_si256(transmute(inputs[32*i..].as_ptr()));
            let weight_slice = _mm256_loadu_si256(transmute(ll.weights[j*4][32*i..].as_ptr()));
            let prod = _mm256_maddubs_epi16(inputs_slice, weight_slice);
            let prod = _mm256_madd_epi16(prod, ones);
            sum0 = _mm256_add_epi32(prod, sum0);
            let weight_slice = _mm256_loadu_si256(transmute(ll.weights[j*4+1][32*i..].as_ptr()));
            let prod = _mm256_maddubs_epi16(inputs_slice, weight_slice);
            let prod = _mm256_madd_epi16(prod, ones);
            sum1 = _mm256_add_epi32(prod, sum1);
            let weight_slice = _mm256_loadu_si256(transmute(ll.weights[j*4+2][32*i..].as_ptr()));
            let prod = _mm256_maddubs_epi16(inputs_slice, weight_slice);
            let prod = _mm256_madd_epi16(prod, ones);
            sum2 = _mm256_add_epi32(prod, sum2);
            let weight_slice = _mm256_loadu_si256(transmute(ll.weights[j*4+3][32*i..].as_ptr()));
            let prod = _mm256_maddubs_epi16(inputs_slice, weight_slice);
            let prod = _mm256_madd_epi16(prod, ones);
            sum3 = _mm256_add_epi32(prod, sum3);
        }

        let bias = _mm_loadu_si128(transmute(ll.biases[j*4..].as_ptr()));
        sum0 = _mm256_hadd_epi32(sum0, sum1);
        sum2 = _mm256_hadd_epi32(sum2, sum3);
        sum0 = _mm256_hadd_epi32(sum0, sum2);
        let sumlo = _mm256_castsi256_si128(sum0);
        let sumhi = _mm256_extracti128_si256::<1>(sum0);
        let res = _mm_add_epi32(_mm_add_epi32(sumlo, sumhi), bias);
        //let res = _mm_srai_epi32::<6>(sum);
        _mm_storeu_si128(transmute(outputs[j*4..].as_mut_ptr()), res);
    }

    outputs
}
