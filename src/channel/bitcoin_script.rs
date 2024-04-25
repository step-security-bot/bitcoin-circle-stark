use crate::channel_commit::CommitmentGadget;
use crate::channel_extract::ExtractorGadget;
use crate::utils::trim_m31_gadget;
use bitvm::treepp::*;

pub struct ChannelGadget;

impl ChannelGadget {
    pub fn new(hash: [u8; 32]) -> Script {
        script! {
            { hash.to_vec() }
        }
    }

    pub fn absorb_commitment() -> Script {
        script! {
            OP_CAT OP_SHA256
        }
    }

    pub fn absorb_qm31() -> Script {
        script! {
            { CommitmentGadget::commit_qm31() }
            OP_CAT OP_SHA256
        }
    }

    pub fn squeeze_element_using_hint() -> Script {
        script! {
            OP_DUP OP_SHA256 OP_SWAP
            OP_PUSHBYTES_1 OP_PUSHBYTES_0 OP_CAT OP_SHA256
            { ExtractorGadget::unpack_qm31() }
        }
    }

    pub fn squeeze_5queries_using_hint(logn: usize) -> Script {
        script! {
            OP_DUP OP_SHA256 OP_SWAP
            OP_PUSHBYTES_1 OP_PUSHBYTES_0 OP_CAT OP_SHA256
            { ExtractorGadget::unpack_5m31() }
            { trim_m31_gadget(logn) } OP_TOALTSTACK
            { trim_m31_gadget(logn) } OP_TOALTSTACK
            { trim_m31_gadget(logn) } OP_TOALTSTACK
            { trim_m31_gadget(logn) } OP_TOALTSTACK
            { trim_m31_gadget(logn) }
            OP_FROMALTSTACK OP_FROMALTSTACK OP_FROMALTSTACK OP_FROMALTSTACK
        }
    }
}

#[cfg(test)]
mod test {
    use crate::channel::{Channel, ChannelGadget};
    use crate::channel_commit::Commitment;
    use crate::channel_extract::ExtractorGadget;
    use crate::fields::{CM31, M31, QM31};
    use bitcoin_script::script;
    use bitvm::treepp::*;
    use rand::{Rng, RngCore, SeedableRng};
    use rand_chacha::ChaCha20Rng;
    use rust_bitcoin_u31_or_u30::{u31ext_equalverify, QM31 as QM31Gadget};

    #[test]
    fn test_absorb_commitment() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        let channel_script = ChannelGadget::absorb_commitment();
        println!(
            "Channel.absorb_commitment() = {} bytes",
            channel_script.len()
        );

        let mut a = [0u8; 32];
        a.iter_mut().for_each(|v| *v = prng.gen());

        let mut b = [0u8; 32];
        b.iter_mut().for_each(|v| *v = prng.gen());

        let mut channel = Channel::new(a);
        channel.absorb_commitment(&Commitment(b));

        let c = channel.state;

        let script = script! {
            { a.to_vec() }
            { b.to_vec() }
            { channel_script.clone() }
            { c.to_vec() }
            OP_EQUAL
        };
        let exec_result = execute_script(script);
        assert!(exec_result.success);
    }

    #[test]
    fn test_absorb_qm31() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        let channel_script = ChannelGadget::absorb_qm31();
        println!("Channel.absorb_qm31() = {} bytes", channel_script.len());

        let mut a = [0u8; 32];
        a.iter_mut().for_each(|v| *v = prng.gen());

        let b = QM31(
            CM31(M31::reduce(prng.next_u64()), M31::reduce(prng.next_u64())),
            CM31(M31::reduce(prng.next_u64()), M31::reduce(prng.next_u64())),
        );

        let mut channel = Channel::new(a);
        channel.absorb_qm31(&b);

        let c = channel.state;

        let script = script! {
            { a.to_vec() }
            { b }
            { channel_script.clone() }
            { c.to_vec() }
            OP_EQUAL
        };
        let exec_result = execute_script(script);
        assert!(exec_result.success);
    }

    #[test]
    fn test_squeeze_element_using_hint() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        let channel_script = ChannelGadget::squeeze_element_using_hint();
        println!(
            "Channel.squeeze_element_using_hint() = {} bytes",
            channel_script.len()
        );

        let mut a = [0u8; 32];
        a.iter_mut().for_each(|v| *v = prng.gen());

        let mut channel = Channel::new(a);
        let (b, hint) = channel.draw_element();

        let c = channel.state;

        let script = script! {
            { ExtractorGadget::push_hint_qm31(&hint) }
            { a.to_vec() }
            { channel_script.clone() }
            { b }
            { u31ext_equalverify::<QM31Gadget>() }
            { c.to_vec() }
            OP_EQUAL
        };
        let exec_result = execute_script(script);
        assert!(exec_result.success);
    }

    #[test]
    fn test_squeeze_5queries_using_hint() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        let channel_script = ChannelGadget::squeeze_5queries_using_hint(15);
        println!(
            "Channel.squeeze_5queries_using_hint() = {} bytes",
            channel_script.len()
        );

        let mut a = [0u8; 32];
        a.iter_mut().for_each(|v| *v = prng.gen());

        let mut channel = Channel::new(a);
        let (b, hint) = channel.draw_5queries(15);

        let c = channel.state;

        let script = script! {
            { ExtractorGadget::push_hint_5m31(&hint) }
            { a.to_vec() }
            { channel_script.clone() }
            { b[4] } OP_EQUALVERIFY
            { b[3] } OP_EQUALVERIFY
            { b[2] } OP_EQUALVERIFY
            { b[1] } OP_EQUALVERIFY
            { b[0] } OP_EQUALVERIFY
            { c.to_vec() }
            OP_EQUAL
        };
        let exec_result = execute_script(script);
        assert!(exec_result.success);
    }
}