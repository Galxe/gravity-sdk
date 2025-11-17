// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! PVSS (Publicly Verifiable Secret Sharing) tests

#![allow(clippy::needless_borrow)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::let_and_return)]

use gaptos::aptos_crypto::hash::CryptoHash;
use gaptos::aptos_dkg::{
    constants::{G1_PROJ_NUM_BYTES, G2_PROJ_NUM_BYTES},
    pvss::{
        das,
        das::unweighted_protocol,
        insecure_field,
        test_utils::{
            get_threshold_configs_for_benchmarking, get_threshold_configs_for_testing,
            get_weighted_configs_for_benchmarking, get_weighted_configs_for_testing,
            reconstruct_dealt_secret_key_randomly, NoAux,
        },
        traits::{transcript::Transcript, SecretSharingConfig},
        GenericWeighting, ThresholdConfig, WeightedConfig,
    },
    utils::random::random_scalar,
};
use rand::{rngs::StdRng, thread_rng};

#[test]
fn test_pvss_all_unweighted() {
    let mut rng = thread_rng();

    //
    // Unweighted PVSS tests
    //
    let tcs = get_threshold_configs_for_testing();
    for tc in tcs {
        println!("\nTesting {tc} PVSS");

        let seed = random_scalar(&mut rng);

        // Das
        pvss_deal_verify_and_reconstruct::<das::Transcript>(&tc, seed.to_bytes_le());

        // Insecure testing-only field-element PVSS
        pvss_deal_verify_and_reconstruct::<insecure_field::Transcript>(&tc, seed.to_bytes_le());
    }
}

#[test]
fn test_pvss_all_weighted() {
    let mut rng = thread_rng();

    //
    // PVSS weighted tests
    //
    let wcs = get_weighted_configs_for_testing();

    for wc in wcs {
        println!("\nTesting {wc} PVSS");
        let seed = random_scalar(&mut rng);

        // Generically-weighted Das
        // WARNING: Insecure, due to encrypting different shares with the same randomness, do not use!
        pvss_deal_verify_and_reconstruct::<GenericWeighting<das::Transcript>>(
            &wc,
            seed.to_bytes_le(),
        );

        // Generically-weighted field-element PVSS
        // WARNING: Insecure, reveals the dealt secret and its shares.
        pvss_deal_verify_and_reconstruct::<GenericWeighting<insecure_field::Transcript>>(
            &wc,
            seed.to_bytes_le(),
        );

        // Provably-secure Das PVSS
        pvss_deal_verify_and_reconstruct::<das::WeightedTranscript>(&wc, seed.to_bytes_le());
    }
}

#[test]
fn test_pvss_transcript_size() {
    for sc in get_threshold_configs_for_benchmarking() {
        println!();
        let expected_size = expected_transcript_size::<das::Transcript>(&sc);
        let actual_size = actual_transcript_size::<das::Transcript>(&sc);

        print_transcript_size::<das::Transcript>("Expected", &sc, expected_size);
        print_transcript_size::<das::Transcript>("Actual", &sc, actual_size);
    }

    for wc in get_weighted_configs_for_benchmarking() {
        let actual_size = actual_transcript_size::<das::Transcript>(wc.get_threshold_config());
        print_transcript_size::<das::Transcript>("Actual", wc.get_threshold_config(), actual_size);

        let actual_size = actual_transcript_size::<das::WeightedTranscript>(&wc);
        print_transcript_size::<das::WeightedTranscript>("Actual", &wc, actual_size);
    }
}

fn print_transcript_size<T: Transcript>(size_type: &str, sc: &T::SecretSharingConfig, size: usize) {
    let name = T::scheme_name();
    println!("{size_type:8} transcript size for {sc} {name}: {size} bytes");
}

//
// Helper functions
//

/// Basic viability test for a PVSS transcript (weighted or unweighted):
///  1. Deals a secret, creating a transcript
///  2. Verifies the transcript.
///  3. Ensures the a sufficiently-large random subset of the players can recover the dealt secret
fn pvss_deal_verify_and_reconstruct<T: Transcript + CryptoHash>(
    sc: &T::SecretSharingConfig,
    seed_bytes: [u8; 32],
) {
    // println!();
    // println!("Seed: {}", hex::encode(seed_bytes.as_slice()));
    use rand::SeedableRng;
    let mut rng = StdRng::from_seed(seed_bytes);

    let d = gaptos::aptos_dkg::pvss::test_utils::setup_dealing::<T, StdRng>(sc, &mut rng);

    // Test dealing
    let trx = T::deal(
        &sc,
        &d.pp,
        &d.ssks[0],
        &d.eks,
        &d.s,
        &NoAux,
        &sc.get_player(0),
        &mut rng,
    );
    trx.verify(&sc, &d.pp, &vec![d.spks[0].clone()], &d.eks, &vec![NoAux])
        .expect("PVSS transcript failed verification");

    // Test transcript (de)serialization
    let trx_deserialized = T::try_from(trx.to_bytes().as_slice())
        .expect("serialized transcript should deserialize correctly");

    assert_eq!(trx, trx_deserialized);
    if d.dsk != reconstruct_dealt_secret_key_randomly::<StdRng, T>(sc, &mut rng, &d.dks, trx) {
        panic!("Reconstructed SK did not match");
    }
}

fn actual_transcript_size<T: Transcript>(sc: &T::SecretSharingConfig) -> usize {
    let mut rng = thread_rng();

    let trx = T::generate(&sc, &mut rng);
    let actual_size = trx.to_bytes().len();

    actual_size
}

fn expected_transcript_size<T: Transcript<SecretSharingConfig = ThresholdConfig>>(
    sc: &ThresholdConfig,
) -> usize {
    if T::scheme_name() == unweighted_protocol::DAS_SK_IN_G1 {
        G2_PROJ_NUM_BYTES
            + (sc.get_total_num_players() + 1) * (G2_PROJ_NUM_BYTES + G1_PROJ_NUM_BYTES)
    } else {
        panic!("Did not implement support for '{}' yet", T::scheme_name())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn create_many_configs() {
        let mut _tcs = vec![];

        for t in 1..100 {
            for n in t..100 {
                _tcs.push(ThresholdConfig::new(t, n).unwrap())
            }
        }
    }

    #[test]
    fn bvt() {
        // 1-out-of-1 weighted
        let wc = WeightedConfig::new(1, vec![1]).unwrap();
        assert_eq!(wc.get_virtual_player(&wc.get_player(0), 0).id, 0);

        // 1-out-of-2, weights 2
        let wc = WeightedConfig::new(1, vec![2]).unwrap();
        assert_eq!(wc.get_virtual_player(&wc.get_player(0), 0).id, 0);
        assert_eq!(wc.get_virtual_player(&wc.get_player(0), 1).id, 1);

        // 1-out-of-2, weights 1, 1
        let wc = WeightedConfig::new(1, vec![1, 1]).unwrap();
        assert_eq!(wc.get_virtual_player(&wc.get_player(0), 0).id, 0);
        assert_eq!(wc.get_virtual_player(&wc.get_player(1), 0).id, 1);

        // 3-out-of-5, some weights are 0.
        let wc = WeightedConfig::new(1, vec![0, 0, 0, 2, 2, 2, 0, 0, 0, 3, 3, 3, 0, 0, 0]).unwrap();
        assert_eq!(wc.get_virtual_player(&wc.get_player(3), 0).id, 0);
        assert_eq!(wc.get_virtual_player(&wc.get_player(3), 1).id, 1);
        assert_eq!(wc.get_virtual_player(&wc.get_player(4), 0).id, 2);
        assert_eq!(wc.get_virtual_player(&wc.get_player(4), 1).id, 3);
        assert_eq!(wc.get_virtual_player(&wc.get_player(5), 0).id, 4);
        assert_eq!(wc.get_virtual_player(&wc.get_player(5), 1).id, 5);
        assert_eq!(wc.get_virtual_player(&wc.get_player(9), 0).id, 6);
        assert_eq!(wc.get_virtual_player(&wc.get_player(9), 1).id, 7);
        assert_eq!(wc.get_virtual_player(&wc.get_player(9), 2).id, 8);
        assert_eq!(wc.get_virtual_player(&wc.get_player(10), 0).id, 9);
        assert_eq!(wc.get_virtual_player(&wc.get_player(10), 1).id, 10);
        assert_eq!(wc.get_virtual_player(&wc.get_player(10), 2).id, 11);
        assert_eq!(wc.get_virtual_player(&wc.get_player(11), 0).id, 12);
        assert_eq!(wc.get_virtual_player(&wc.get_player(11), 1).id, 13);
        assert_eq!(wc.get_virtual_player(&wc.get_player(11), 2).id, 14);
    }

    #[test]
    fn test_ldt_correctness() {
        use gaptos::aptos_dkg::{
            algebra::{evaluation_domain::BatchEvaluationDomain, fft::fft_assign},
            pvss::{test_utils, LowDegreeTest},
            utils::random::random_scalars,
        };
        use blstrs::Scalar;
        use rand::{prelude::ThreadRng, thread_rng};

        let mut rng = thread_rng();

        for sc in test_utils::get_threshold_configs_for_testing() {
            // A degree t-1 polynomial p(X)
            let (p_0, batch_dom, mut evals) = random_polynomial_evals(&mut rng, &sc);

            let t = sc.get_threshold();
            let n = sc.get_total_num_players();

            // Test deg(p) < t, given evals at roots of unity
            let ldt = LowDegreeTest::random(&mut rng, t, n, false, &batch_dom);
            assert!(ldt.low_degree_test(&evals).is_ok());

            if t < n {
                // Test deg(p) < t + 1, given evals at roots of unity
                let ldt = LowDegreeTest::random(&mut rng, t + 1, n, false, &batch_dom);
                assert!(ldt.low_degree_test(&evals).is_ok());
            }

            // Test deg(p) < t, given evals at roots of unity and given p(0)
            evals.push(p_0);
            let ldt = LowDegreeTest::random(&mut rng, t, n + 1, true, &batch_dom);
            assert!(ldt.low_degree_test(&evals).is_ok());
        }

        fn random_polynomial_evals(
            mut rng: &mut ThreadRng,
            sc: &ThresholdConfig,
        ) -> (Scalar, BatchEvaluationDomain, Vec<Scalar>) {
            let t = sc.get_threshold();
            let n = sc.get_total_num_players();
            let p = random_scalars(t, &mut rng);
            let p_0 = p[0];
            let batch_dom = BatchEvaluationDomain::new(n);

            // Compute p(\omega^i) for all i's
            // (e.g., in SCRAPE we will be given A_i = g^{p(\omega^i)})
            let mut p_evals = p;
            fft_assign(&mut p_evals, &batch_dom.get_subdomain(n));
            p_evals.truncate(n);
            (p_0, batch_dom, p_evals)
        }
    }

    /// Test the soundness of the LDT: a polynomial of degree > t - 1 should not pass the check.
    #[test]
    fn test_ldt_soundness() {
        use gaptos::aptos_dkg::{
            algebra::{evaluation_domain::BatchEvaluationDomain, fft::fft_assign},
            pvss::LowDegreeTest,
            utils::random::random_scalars,
        };
        use blstrs::Scalar;
        use rand::{prelude::ThreadRng, thread_rng};

        let mut rng = thread_rng();

        for t in 1..8 {
            for n in (t + 1)..(3 * t + 1) {
                let sc = ThresholdConfig::new(t, n).unwrap();
                let sc_higher_degree = ThresholdConfig::new(sc.get_threshold() + 1, sc.get_total_num_players()).unwrap();

                // A degree t polynomial p(X), higher by 1 than what the LDT expects
                let (p_0, batch_dom, mut evals) =
                    random_polynomial_evals(&mut rng, &sc_higher_degree);

                let t_val = sc.get_threshold();
                let n_val = sc.get_total_num_players();

                // Test deg(p) < t, given evals at roots of unity
                // This should fail, since deg(p) = t
                let ldt = LowDegreeTest::random(&mut rng, t_val, n_val, false, &batch_dom);
                assert!(ldt.low_degree_test(&evals).is_err());

                // Test deg(p) < t, given evals at roots of unity and given p(0)
                // This should fail, since deg(p) = t
                evals.push(p_0);
                let ldt = LowDegreeTest::random(&mut rng, t_val, n_val + 1, true, &batch_dom);
                assert!(ldt.low_degree_test(&evals).is_err());
            }
        }

        fn random_polynomial_evals(
            mut rng: &mut ThreadRng,
            sc: &ThresholdConfig,
        ) -> (Scalar, BatchEvaluationDomain, Vec<Scalar>) {
            let t = sc.get_threshold();
            let n = sc.get_total_num_players();
            let p = random_scalars(t, &mut rng);
            let p_0 = p[0];
            let batch_dom = BatchEvaluationDomain::new(n);

            // Compute p(\omega^i) for all i's
            // (e.g., in SCRAPE we will be given A_i = g^{p(\omega^i)})
            let mut p_evals = p;
            fft_assign(&mut p_evals, &batch_dom.get_subdomain(n));
            p_evals.truncate(n);
            (p_0, batch_dom, p_evals)
        }
    }
}


