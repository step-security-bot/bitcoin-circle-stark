use crate::channel::DrawHints;
use crate::treepp::*;
use crate::utils::{hash_felt_gadget, trim_m31_gadget};

/// Gadget for a channel.
pub struct Sha256ChannelGadget;

impl Sha256ChannelGadget {
    /// Absorb a commitment.
    pub fn mix_digest() -> Script {
        script! {
            OP_CAT OP_SHA256
        }
    }

    /// Absorb a qm31 element.
    pub fn mix_felt() -> Script {
        script! {
            OP_TOALTSTACK
            hash_felt_gadget
            OP_FROMALTSTACK OP_CAT OP_SHA256
        }
    }

    /// Squeeze a qm31 element using hints.
    pub fn draw_felt_with_hint() -> Script {
        script! {
            OP_DUP OP_SHA256 OP_SWAP
            OP_PUSHBYTES_1 OP_PUSHBYTES_0 OP_CAT OP_SHA256
            { Self::unpack_multi_m31::<4>() }
        }
    }

    /// Squeeze queries from the channel, each of logn bits, using hints.
    pub fn draw_5numbers_with_hint(logn: usize) -> Script {
        script! {
            OP_DUP OP_SHA256 OP_SWAP
            OP_PUSHBYTES_1 OP_PUSHBYTES_0 OP_CAT OP_SHA256
            { Self::unpack_multi_m31::<5>() }
            { trim_m31_gadget(logn) }
            OP_SWAP { trim_m31_gadget(logn) }
            OP_2SWAP { trim_m31_gadget(logn) }
            OP_SWAP { trim_m31_gadget(logn) }
            4 OP_ROLL { trim_m31_gadget(logn) }
        }
    }

    /// Push the hint for drawing m31 elements from a hash.
    pub fn push_draw_hint<const N: usize>(e: &DrawHints<N>) -> Script {
        if N % 8 == 0 {
            assert!(e.1.is_empty());
        } else {
            assert_eq!(e.1.len(), 32 - (N % 8) * 4);
        }
        script! {
            for i in 0..N {
                { e.0[i] }
            }
            if N % 8 != 0 {
                { e.1.clone() }
            }
        }
    }

    /// Reconstruct a 4-byte representation from a Bitcoin integer.
    ///
    /// Idea: extract the positive/negative symbol and pad it accordingly.
    fn reconstruct() -> Script {
        script! {
            // handle 0x80 specially---it is the "negative zero", but most arithmetic opcodes refuse to work with it.
            OP_DUP OP_PUSHBYTES_1 OP_LEFT OP_EQUAL
            OP_IF
                OP_DROP
                OP_PUSHBYTES_0 OP_TOALTSTACK
                OP_PUSHBYTES_4 OP_PUSHBYTES_0 OP_PUSHBYTES_0 OP_PUSHBYTES_0 OP_LEFT
            OP_ELSE
                OP_DUP OP_ABS
                OP_DUP OP_TOALTSTACK

                OP_SIZE 4 OP_LESSTHAN
                OP_IF
                    OP_DUP OP_ROT
                    OP_EQUAL OP_TOALTSTACK

                    // stack: abs(a)
                    // altstack: abs(a), is_positive

                    OP_SIZE 2 OP_LESSTHAN OP_IF OP_PUSHBYTES_2 OP_PUSHBYTES_0 OP_PUSHBYTES_0 OP_CAT OP_ENDIF
                    OP_SIZE 3 OP_LESSTHAN OP_IF OP_PUSHBYTES_1 OP_PUSHBYTES_0 OP_CAT OP_ENDIF

                    OP_FROMALTSTACK
                    OP_IF
                        OP_PUSHBYTES_1 OP_PUSHBYTES_0
                    OP_ELSE
                        OP_PUSHBYTES_1 OP_LEFT
                    OP_ENDIF
                    OP_CAT
                OP_ELSE
                    OP_DROP
                OP_ENDIF
            OP_ENDIF
        }
    }

    /// Unpack multiple m31 and put them on the stack.
    pub fn unpack_multi_m31<const N: usize>() -> Script {
        script! {
            for _ in 0..N {
                OP_DEPTH OP_1SUB OP_ROLL
            }

            for _ in 0..N {
                { N - 1 } OP_ROLL
                { Self::reconstruct() }
            }

            for _ in 0..N-1 {
                OP_CAT
            }

            if N % 8 != 0 {
                OP_DEPTH OP_1SUB OP_ROLL OP_CAT
            }

            OP_EQUALVERIFY

            for _ in 0..N {
                OP_FROMALTSTACK

                // Reduce the number from [0, 2^31-1] to [0, 2^31-2] by subtracting 1 from any element that is not zero.
                // This is because 2^31-1 is the modulus and a reduced element should be smaller than it.
                // The sampling, therefore, has a small bias.
                OP_DUP OP_NOT OP_NOTIF OP_1SUB OP_ENDIF
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::channel::{generate_hints, ChannelWithHint, Sha256Channel, Sha256ChannelGadget};
    use crate::tests_utils::report::report_bitcoin_script_size;
    use crate::treepp::*;
    use crate::utils::{hash_felt_gadget, hash_qm31};
    use bitcoin_script::script;
    use rand::{Rng, RngCore, SeedableRng};
    use rand_chacha::ChaCha20Rng;
    use rust_bitcoin_m31::qm31_equalverify;
    use stwo_prover::core::channel::Channel;
    use stwo_prover::core::fields::cm31::CM31;
    use stwo_prover::core::fields::m31::M31;
    use stwo_prover::core::fields::qm31::QM31;
    use stwo_prover::core::vcs::bws_sha256_hash::BWSSha256Hash;

    #[test]
    fn test_mix_digest() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        let channel_script = Sha256ChannelGadget::mix_digest();
        report_bitcoin_script_size("Channel", "mix_digest", channel_script.len());

        let mut init_state = [0u8; 32];
        init_state.iter_mut().for_each(|v| *v = prng.gen());
        let init_state = BWSSha256Hash::from(init_state.to_vec());

        let mut elem = [0u8; 32];
        elem.iter_mut().for_each(|v| *v = prng.gen());
        let elem = BWSSha256Hash::from(elem.to_vec());

        let mut channel = Sha256Channel::new(init_state);
        channel.mix_digest(elem);

        let final_state = channel.digest;

        let script = script! {
            { elem }
            { init_state }
            { channel_script.clone() }
            { final_state }
            OP_EQUAL
        };
        let exec_result = execute_script(script);
        assert!(exec_result.success);
    }

    #[test]
    fn test_mix_felt() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        let channel_script = Sha256ChannelGadget::mix_felt();
        report_bitcoin_script_size("Channel", "mix_felt", channel_script.len());

        let mut init_state = [0u8; 32];
        init_state.iter_mut().for_each(|v| *v = prng.gen());
        let init_state = BWSSha256Hash::from(init_state.to_vec());

        let elem = QM31(
            CM31(M31::reduce(prng.next_u64()), M31::reduce(prng.next_u64())),
            CM31(M31::reduce(prng.next_u64()), M31::reduce(prng.next_u64())),
        );

        let mut channel = Sha256Channel::new(init_state);
        channel.mix_felts(&[elem]);

        let final_state = channel.digest;

        let script = script! {
            { elem }
            { init_state }
            { channel_script.clone() }
            { final_state }
            OP_EQUAL
        };
        let exec_result = execute_script(script);
        assert!(exec_result.success);
    }

    #[test]
    fn test_draw_8_elements() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        for _ in 0..100 {
            let mut a = [0u8; 32];
            a.iter_mut().for_each(|v| *v = prng.gen());
            let a = BWSSha256Hash::from(a.to_vec());

            let mut channel = Sha256Channel::new(a);
            let (b, hint) = channel.draw_m31_and_hints::<8>();

            let c = channel.digest;

            let script = script! {
                { Sha256ChannelGadget::push_draw_hint(&hint) }
                { a }
                OP_DUP OP_SHA256 OP_SWAP
                OP_PUSHBYTES_1 OP_PUSHBYTES_0 OP_CAT OP_SHA256
                { Sha256ChannelGadget::unpack_multi_m31::<8>() }
                for i in 0..8 {
                    { b[i] }
                    OP_EQUALVERIFY
                }
                { c }
                OP_EQUAL
            };
            let exec_result = execute_script(script);
            assert!(exec_result.success);
        }
    }

    #[test]
    fn test_draw_felt_with_hint() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        let channel_script = Sha256ChannelGadget::draw_felt_with_hint();
        report_bitcoin_script_size("Channel", "draw_felt_with_hint", channel_script.len());

        for _ in 0..100 {
            let mut a = [0u8; 32];
            a.iter_mut().for_each(|v| *v = prng.gen());
            let a = BWSSha256Hash::from(a.to_vec());

            let mut channel = Sha256Channel::new(a);
            let (b, hint) = channel.draw_felt_and_hints();

            let c = channel.digest;

            let script = script! {
                { Sha256ChannelGadget::push_draw_hint(&hint) }
                { a }
                { channel_script.clone() }
                { b }
                qm31_equalverify
                { c }
                OP_EQUAL
            };
            let exec_result = execute_script(script);
            assert!(exec_result.success);
        }
    }

    #[test]
    fn test_draw_5numbers_with_hint() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        let channel_script = Sha256ChannelGadget::draw_5numbers_with_hint(15);

        report_bitcoin_script_size("Channel", "draw_5numbers_with_hint", channel_script.len());

        for _ in 0..100 {
            let mut a = [0u8; 32];
            a.iter_mut().for_each(|v| *v = prng.gen());
            let a = BWSSha256Hash::from(a.to_vec());

            let mut channel = Sha256Channel::new(a);
            let (b, hint) = channel.draw_5queries(15);

            let c = channel.digest;

            let script = script! {
                { Sha256ChannelGadget::push_draw_hint(&hint) }
                { a }
                { channel_script.clone() }
                { b[4] } OP_EQUALVERIFY
                { b[3] } OP_EQUALVERIFY
                { b[2] } OP_EQUALVERIFY
                { b[1] } OP_EQUALVERIFY
                { b[0] } OP_EQUALVERIFY
                { c }
                OP_EQUAL
            };
            let exec_result = execute_script(script);
            assert!(exec_result.success);
        }
    }

    #[test]
    fn test_hash_felt() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        let commit_script = hash_felt_gadget();
        report_bitcoin_script_size("QM31", "hash", commit_script.len());

        for _ in 0..100 {
            let a = QM31(
                CM31(M31::reduce(prng.next_u64()), M31::reduce(prng.next_u64())),
                CM31(M31::reduce(prng.next_u64()), M31::reduce(prng.next_u64())),
            );
            let b = hash_qm31(&a);

            let script = script! {
                { a }
                { commit_script.clone() }
                { b.to_vec() }
                OP_EQUAL
            };
            let exec_result = execute_script(script);
            assert!(exec_result.success);
        }

        // make sure OP_CAT is not OP_SUCCESS
        let script = script! {
            OP_CAT
            OP_RETURN
        };
        let exec_result = execute_script(script);
        assert!(!exec_result.success);
    }

    #[test]
    fn test_corner_case() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        let mut h = [0u8; 32];
        h[3] = 0x80;
        for elem in h.iter_mut().skip(4) {
            *elem = prng.gen();
        }

        let (_, hint) = generate_hints::<1>(&h);

        let script = script! {
            { Sha256ChannelGadget::push_draw_hint(&hint) }
            { Sha256ChannelGadget::unpack_multi_m31::<1>() }
            OP_NOT
        };
        let exec_result = execute_script(script);
        assert!(!exec_result.success);
    }
}
