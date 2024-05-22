use bitvm::treepp::*;
use rust_bitcoin_m31::{
    qm31_add, qm31_copy, qm31_double, qm31_equalverify, qm31_fromaltstack, qm31_mul, qm31_roll,
    qm31_sub, qm31_toaltstack,
};
use std::ops::{Add, Mul, Neg};

use crate::{
    channel::ChannelGadget,
    math::{Field, QM31},
};

pub struct CirclePointSecureGadget;

impl CirclePointSecureGadget {
    // Rationale: cos(2*theta) = 2*cos(theta)^2-1
    //
    // input:
    //  x (QM31)
    //
    // output:
    //  2*x^2-1 (QM31)
    pub fn double_x() -> Script {
        script! {
            { qm31_copy(0) }
            qm31_mul
            qm31_double
            { 0 }
            { 0 }
            { 0 }
            { 1 }
            qm31_sub
        }
    }

    // Samples a random point over the projective line, see Lemma 1 in https://eprint.iacr.org/2024/278.pdf
    //
    // input:
    //  qm31 hint (5 elements) --- as hints
    //  y - hint such that 2*t = y*(1+t^2)
    //  x - hint such that 1-t^2 = x*(1+t^2)
    //      where t is extracted from channel
    //  channel
    //
    // output:
    //  y
    //  x
    //      where (x,y) - random point on C(QM31) satisfying x^2+y^2=1 (8 elements)
    //  channel'=sha256(channel)
    //
    // WARNING!!: y & x are read using u31ext_copy(), and not via the hint convention of repeating `OP_DEPTH OP_1SUB OP_ROLL`
    //
    pub fn verify_sampling_random_point() -> Script {
        script! {
            { ChannelGadget::squeeze_element_using_hint() } //stack: y,x,channel',t
            4 OP_ROLL //stack: y,x,t,channel'
            OP_TOALTSTACK //stack: y,x,t; altstack: channel'
            { qm31_copy(1) } //stack: y,x,t,x; altstack: channel'
            qm31_toaltstack //stack: y,x,t; altstack: channel',x
            { qm31_copy(2) } //stack: y,x,t,y; altstack: channel',x
            qm31_toaltstack  //stack: y,x,t; altstack: channel',x,y
            { qm31_copy(0) }  //stack: y,x,t,t; altstack: channel'
            qm31_double //stack: y,x,t,2*t; altstack: channel'
            qm31_toaltstack  //stack: y,x,t; altstack: channel',x,y,2*t
            { qm31_copy(0) } //stack: y,x,t,t; altstack: channel',x,y,2*t
            qm31_mul //stack: y,x,t^2; altstack: channel',x,y,2*t
            { qm31_copy(0) } //stack: y,x,t^2,t^2; altstack: channel',x,y,2*t
            { 0 as u32 }
            { 0 as u32 }
            { 0 as u32 }
            { 1 as u32 } //stack: y,x,t^2,t^2,1; altstack: channel',x,y,2*t
            { qm31_roll(1) } //stack: y,x,t^2,1,t^2; altstack: channel',x,y,2*t
            qm31_sub //stack: y,x,t^2,1-t^2; altstack: channel',x,y,2*t
            qm31_toaltstack //stack: y,x,t^2; altstack: channel',x,y,2*t,1-t^2
            { 0 as u32 }
            { 0 as u32 }
            { 0 as u32 }
            { 1 as u32 }
            qm31_add //stack: y,x,1+t^2; altstack: channel',x,y,2*t,1-t^2
            { qm31_copy(0) } //stack: y,x,1+t^2,1+t^2; altstack: channel',x,y,2*t,1-t^2
            { qm31_roll(2) } //stack: y,1+t^2,1+t^2,x; altstack: channel',x,y,2*t,1-t^2
            qm31_mul //stack: y,1+t^2,(1+t^2)*x; altstack: channel',x,y,2*t,1-t^2
            qm31_fromaltstack //stack: y,1+t^2,(1+t^2)*x,1-t^2; altstack: channel',x,y,2*t
            qm31_equalverify //stack: y,1+t^2; altstack: channel',x,y,2*t
            qm31_mul //stack: y*(1+t^2); altstack: channel',x,y,2*t,1-t^2
            qm31_fromaltstack //stack: y*(1+t^2),1-t^2; altstack: channel',x,y,2*t
            qm31_equalverify //stack: ; altstack: channel',x,y
            OP_FROMALTSTACK qm31_fromaltstack qm31_fromaltstack //stack: y,x,channel'
        }
    }

    // input:
    //  NONE - this function does not update the channel, only peeks at its value
    //
    // output:
    //  y
    //  x
    pub fn push_random_point_hint(t: QM31) -> Script {
        let oneplustsquaredinv = t.square().add(QM31::one()).inverse(); //(1+t^2)^-1

        script! {
            { t.double().mul(oneplustsquaredinv) }
            { QM31::one().add(t.square().neg()).mul(oneplustsquaredinv) }
        }
    }
}

#[cfg(test)]
mod test {
    use std::ops::{Add, Mul, Neg};

    use bitvm::treepp::*;
    use rand::{Rng, RngCore, SeedableRng};
    use rand_chacha::ChaCha20Rng;
    use rust_bitcoin_m31::qm31_equalverify;

    use crate::{
        channel::Channel,
        channel_extract::ExtractorGadget,
        circle_secure::bitcoin_script::CirclePointSecureGadget,
        math::{Field, M31, QM31},
    };

    #[test]
    fn test_double_x() {
        let double_x_script = CirclePointSecureGadget::double_x();
        println!(
            "CirclePointSecure.double_x() = {} bytes",
            double_x_script.len()
        );

        for seed in 0..20 {
            let mut prng = ChaCha20Rng::seed_from_u64(seed);

            let a = QM31::from_m31(
                M31::reduce(prng.next_u64()),
                M31::reduce(prng.next_u64()),
                M31::reduce(prng.next_u64()),
                M31::reduce(prng.next_u64()),
            );
            let double_a = a.square().double().add(QM31::one().neg());

            let script = script! {
                { a }
                { double_x_script.clone() }
                { double_a }
                qm31_equalverify
                OP_TRUE
            };
            let exec_result = execute_script(script);
            assert!(exec_result.success);
        }
    }

    #[test]
    fn test_get_random_point() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        let get_random_point_script = CirclePointSecureGadget::verify_sampling_random_point();
        println!(
            "CirclePointSecure.get_random_point() = {} bytes",
            get_random_point_script.len()
        );

        let mut a = [0u8; 32];
        a.iter_mut().for_each(|v| *v = prng.gen());

        let mut channel = Channel::new(a);
        let (t, hint_t) = channel.draw_element();

        let c = channel.state;

        let x = t
            .square()
            .add(QM31::one())
            .inverse()
            .mul(QM31::one().add(t.square().neg())); //(1+t^2)^-1 * (1-t^2)
        let y = t
            .square()
            .add(QM31::one())
            .inverse()
            .mul(QM31::one().double().mul(t)); // (1+t^2)^-1 * 2 * t

        let script = script! {
            { ExtractorGadget::push_hint_qm31(&hint_t) }
            { CirclePointSecureGadget::push_random_point_hint(t.clone()) }
            { a.to_vec() }

            { get_random_point_script.clone() }
            { c.to_vec() } //check channel'
            OP_EQUALVERIFY // checking that indeed channel' = sha256(channel)
            { x } //check x
            qm31_equalverify
            { y } //check y
            qm31_equalverify
            OP_TRUE
        };
        let exec_result = execute_script(script);
        assert!(exec_result.success);
    }
}
