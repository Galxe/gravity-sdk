use gaptos::aptos_dkg::{
    pvss::{
        das::{self, unweighted_protocol},
        insecure_field,
        test_utils::{
            get_threshold_configs_for_testing, get_weighted_configs_for_testing,
            reconstruct_dealt_secret_key_randomly, setup_dealing, DealingArgs, NoAux,
        },
        traits::{SecretSharingConfig, Transcript},
        GenericWeighting,
    },
    utils::random::random_scalar,
};
use gaptos::aptos_crypto::hash::CryptoHash;
use rand::{rngs::StdRng, thread_rng, SeedableRng};
use rand_core::SeedableRng as RandCoreSeedableRng;

#[test]
fn test_dkg_all_unweighted() {
    let mut rng = thread_rng();
    let tcs = get_threshold_configs_for_testing();
    let seed = random_scalar(&mut rng);

    aggregatable_dkg::<unweighted_protocol::Transcript>(tcs.last().unwrap(), seed.to_bytes_le());
    aggregatable_dkg::<insecure_field::Transcript>(tcs.last().unwrap(), seed.to_bytes_le());
}

#[test]
fn test_dkg_all_weighted() {
    let mut rng = thread_rng();
    let wcs = get_weighted_configs_for_testing();
    let seed = random_scalar(&mut rng);

    aggregatable_dkg::<GenericWeighting<unweighted_protocol::Transcript>>(
        wcs.last().unwrap(),
        seed.to_bytes_le(),
    );
    aggregatable_dkg::<GenericWeighting<das::Transcript>>(wcs.last().unwrap(), seed.to_bytes_le());
    aggregatable_dkg::<das::WeightedTranscript>(wcs.last().unwrap(), seed.to_bytes_le());
}

/// Deals `n` times, aggregates all transcripts, and attempts to reconstruct the secret dealt in this
/// aggregated transcript.
fn aggregatable_dkg<T: Transcript + CryptoHash>(sc: &T::SecretSharingConfig, seed_bytes: [u8; 32]) {
    let mut rng = StdRng::from_seed(seed_bytes);

    let d = setup_dealing::<T, StdRng>(sc, &mut rng);

    let mut trxs = vec![];

    // Deal `n` transcripts
    let total_players = SecretSharingConfig::get_total_num_players(sc);
    for i in 0..total_players {
        trxs.push(T::deal(
            sc,
            &d.pp,
            &d.ssks[i],
            &d.eks,
            &d.iss[i],
            &NoAux,
            &SecretSharingConfig::get_player(sc, i),
            &mut rng,
        ));
    }

    // Aggregate all `n` transcripts
    let trx = T::aggregate(sc, trxs).unwrap();

    // Verify the aggregated transcript
    trx.verify(
        sc,
        &d.pp,
        &d.spks,
        &d.eks,
        &(0..total_players)
            .map(|_| NoAux)
            .collect::<Vec<NoAux>>(),
    )
    .expect("aggregated PVSS transcript failed verification");

    if d.dsk != reconstruct_dealt_secret_key_randomly::<StdRng, T>(sc, &mut rng, &d.dks, trx) {
        panic!("Reconstructed SK did not match");
    }
}