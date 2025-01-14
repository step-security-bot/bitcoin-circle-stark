//! The bitcoin-circle-stark crate implements a number of Bitcoin script gadgets for
//! a stwo proof verifier.

#![deny(missing_docs)]

use crate::treepp::pushable::{Builder, Pushable};
use stwo_prover::core::fields::m31::M31;
use stwo_prover::core::fields::qm31::QM31;
use stwo_prover::core::vcs::bws_sha256_hash::BWSSha256Hash;

/// Module for absorbing and squeezing of the channel.
pub mod channel;
/// Module for the circle curve over the qm31 field.
pub mod circle;
/// Module for constraints over the circle curve
pub mod constraints;
/// Module for Fibonacci end-to-end test.
pub mod fibonacci;
/// Module for FRI.
pub mod fri;
/// Module for the Merkle tree.
pub mod merkle_tree;
/// Module for out-of-domain sampling.
pub mod oods;
/// Module for PoW.
pub mod pow;
/// Module for test utils.
pub mod tests_utils;
/// Module for the twiddle Merkle tree.
pub mod twiddle_merkle_tree;
/// Module for utility functions.
pub mod utils;

pub(crate) mod treepp {
    pub use bitcoin_script::{define_pushable, script};
    #[cfg(test)]
    pub use bitcoin_scriptexec::{convert_to_witness, execute_script};

    define_pushable!();
    pub use bitcoin::ScriptBuf as Script;
}

impl Pushable for M31 {
    fn bitcoin_script_push(self, builder: Builder) -> Builder {
        self.0.bitcoin_script_push(builder)
    }
}

impl Pushable for QM31 {
    fn bitcoin_script_push(self, builder: Builder) -> Builder {
        let mut builder = self.1 .1.bitcoin_script_push(builder);
        builder = self.1 .0.bitcoin_script_push(builder);
        builder = self.0 .1.bitcoin_script_push(builder);
        self.0 .0.bitcoin_script_push(builder)
    }
}

impl Pushable for BWSSha256Hash {
    fn bitcoin_script_push(self, builder: Builder) -> Builder {
        self.as_ref().to_vec().bitcoin_script_push(builder)
    }
}

#[cfg(test)]
mod test {
    use crate::channel::Sha256Channel;
    use crate::fri;
    use crate::treepp::{
        pushable::{Builder, Pushable},
        *,
    };
    use crate::twiddle_merkle_tree::TWIDDLE_MERKLE_TREE_ROOT_4;
    use crate::utils::permute_eval;
    use num_traits::One;
    use rand::{Rng, RngCore, SeedableRng};
    use rand_chacha::ChaCha20Rng;
    use stwo_prover::core::channel::Channel;
    use stwo_prover::core::circle::CirclePointIndex;
    use stwo_prover::core::fields::m31::M31;
    use stwo_prover::core::fields::qm31::QM31;
    use stwo_prover::core::fields::FieldExpOps;
    use stwo_prover::core::vcs::bws_sha256_hash::BWSSha256Hash;

    #[test]
    fn test_pushable() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        // m31
        let m31 = M31::reduce(prng.next_u64());
        let qm31 = QM31::from_m31(
            M31::reduce(prng.next_u64()),
            M31::reduce(prng.next_u64()),
            M31::reduce(prng.next_u64()),
            M31::reduce(prng.next_u64()),
        );

        let mut builder = Builder::new();
        builder = m31.bitcoin_script_push(builder);
        assert_eq!(script! { {m31} }.as_bytes(), builder.as_bytes());

        let mut builder = Builder::new();
        builder = qm31.bitcoin_script_push(builder);
        assert_eq!(script! { {qm31} }.as_bytes(), builder.as_bytes());
    }

    #[test]
    fn test_cfri_main() {
        // Prepare a low degree evaluation
        let logn = 5;
        let p = CirclePointIndex::subgroup_gen(logn as u32 + 1).to_point();

        let mut prng = ChaCha20Rng::seed_from_u64(0);

        let mut channel_init_state = [0u8; 32];
        channel_init_state.iter_mut().for_each(|v| *v = prng.gen());

        let channel_init_state = BWSSha256Hash::from(channel_init_state.to_vec());

        // Note: Add another .square() to make the proof fail.
        let evaluation = (0..(1 << logn))
            .map(|i| (p.mul(i * 2 + 1).x.square().square() + M31::one()).into())
            .collect::<Vec<QM31>>();
        let evaluation = permute_eval(evaluation);

        // FRI.
        let proof = fri::fri_prove(&mut Sha256Channel::new(channel_init_state), evaluation);
        fri::fri_verify(
            &mut Sha256Channel::new(channel_init_state),
            logn,
            proof,
            TWIDDLE_MERKLE_TREE_ROOT_4,
        );
    }
}
