// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

// DKG runtime tests are temporarily disabled due to API changes in gaptos
#[cfg(test)]
mod dkg_runtime_tests;

#[cfg(test)]
mod pvss_tests;

#[cfg(test)]
mod crypto_tests;

#[cfg(test)]
mod weighted_vuf_tests;

#[cfg(test)]
mod secret_sharing_config_tests;

#[cfg(test)]
mod fft_tests;

#[cfg(test)]
mod dkg_tests;

#[cfg(test)]
mod accumulator_tests;