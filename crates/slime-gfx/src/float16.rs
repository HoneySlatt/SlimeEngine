//! Half-precision floats (IEEE 754 binary16), the channels of `Rgba16Float` textures, converted by
//! hand rather than with a crate (`docs/PLAN.md`, Stage 3b: no new dependency).

/// The value of the half-precision float whose bits are `bits`, exactly: every half-precision
/// value, subnormals, infinities and NaN included, has an exact single-precision equivalent.
pub(crate) fn f16_to_f32(bits: u16) -> f32 {
    let negative = bits & 0x8000 != 0;
    let exponent = u32::from((bits >> 10) & 0x1f);
    let mantissa = u32::from(bits & 0x3ff);
    let magnitude = match exponent {
        // Zero and the subnormals: the mantissa counts units of 2^-24, exactly representable.
        0 => mantissa as f32 * (1.0 / 16_777_216.0),
        // Infinities and NaN, keeping the NaN's payload.
        0x1f => f32::from_bits(0x7f80_0000 | (mantissa << 13)),
        // Normal numbers: the exponent moves from a bias of 15 to one of 127.
        _ => f32::from_bits(((exponent + 127 - 15) << 23) | (mantissa << 13)),
    };
    if negative { -magnitude } else { magnitude }
}

/// The bits of the half-precision float nearest to `value`, as IEEE 754 rounds by default: to
/// the nearest, and to the one whose last bit is 0 (even) when `value` lies exactly halfway
/// between two. Magnitudes from 65 520 up, halfway between the largest finite half, 65 504, and
/// the next power of two, become infinities; magnitudes below 2^-14 become subnormals, and those
/// at most 2^-25 zeros, keeping their sign. A NaN stays a NaN (a quiet one, keeping the high bits
/// of its payload). For the `Rgba16Float` data of [`Gpu::create_texture_with_levels`] and
/// [`Gpu::create_cube_texture`](crate::Gpu::create_cube_texture), whose texels are four such
/// halves, little-endian.
///
/// [`Gpu::create_texture_with_levels`]: crate::Gpu::create_texture_with_levels
pub fn f32_to_f16(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = (bits >> 23) & 0xff;
    let mantissa = bits & 0x7f_ffff;
    if exponent == 0xff {
        // Infinities, and NaN made quiet so that dropping low payload bits cannot make it one.
        let nan = if mantissa == 0 {
            0
        } else {
            0x200 | (mantissa >> 13) as u16
        };
        return sign | 0x7c00 | nan;
    }
    // The exponent with a bias of 15 instead of 127; 0 or less is below the normal halves.
    let half_exponent = exponent as i32 - 127 + 15;
    if half_exponent >= 0x1f {
        return sign | 0x7c00;
    }
    // The significand with its leading 1, and how many of its low bits half precision drops: 13
    // for normal halves, more for subnormals, whose leading bit moves right. Below 2^-25 all 24
    // go, and the value rounds to zero.
    let (significand, dropped, base) = if half_exponent > 0 {
        (mantissa, 13, (half_exponent as u32) << 10)
    } else if half_exponent >= -10 {
        (mantissa | 0x80_0000, (14 - half_exponent) as u32, 0)
    } else {
        return sign;
    };
    let kept = significand >> dropped;
    let remainder = significand & ((1 << dropped) - 1);
    let halfway = 1 << (dropped - 1);
    let rounded_up = remainder > halfway || (remainder == halfway && kept & 1 == 1);
    // A carry out of the mantissa moves to the next exponent, and from the largest finite half to
    // infinity, as it should.
    sign | (base + kept + u32::from(rounded_up)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values_convert_exactly() {
        for (bits, value) in [
            (0x0000, 0.0),
            (0x3c00, 1.0),
            (0x3800, 0.5),
            (0x4000, 2.0),
            (0xc000, -2.0),
            (0x3555, 0.25 * (1.0 + 341.0 / 1024.0)),
            (0x7bff, 65_504.0),
            // The smallest normal number, then the smallest and the largest subnormals.
            (0x0400, 1.0 / 16_384.0),
            (0x0001, 1.0 / 16_777_216.0),
            (0x03ff, 1023.0 / 16_777_216.0),
        ] {
            assert_eq!(f16_to_f32(bits), value, "{bits:#06x}");
            assert_eq!(f32_to_f16(value), bits, "{value}");
        }
        assert!(f16_to_f32(0x8000) == 0.0 && f16_to_f32(0x8000).is_sign_negative());
        assert_eq!(f16_to_f32(0x7c00), f32::INFINITY);
        assert_eq!(f16_to_f32(0xfc00), f32::NEG_INFINITY);
        assert!(f16_to_f32(0x7e00).is_nan());
        assert!(f16_to_f32(0xfe01).is_nan());
    }

    #[test]
    fn every_finite_value_is_in_order_and_symmetric() {
        // Positive halves are ordered like their bits, from zero to the largest finite one, and
        // each negative half is the negation of its positive counterpart.
        let mut previous = f16_to_f32(0);
        for bits in 1..0x7c00_u16 {
            let value = f16_to_f32(bits);
            assert!(
                value > previous,
                "{bits:#06x} gives {value}, not above {previous}"
            );
            assert_eq!(f16_to_f32(bits | 0x8000), -value, "{bits:#06x}");
            previous = value;
        }
    }

    #[test]
    fn every_half_converts_back_to_itself() {
        for bits in 0..=u16::MAX {
            let value = f16_to_f32(bits);
            if value.is_nan() {
                assert!(f16_to_f32(f32_to_f16(value)).is_nan(), "{bits:#06x}");
            } else {
                assert_eq!(f32_to_f16(value), bits, "{bits:#06x}");
            }
        }
    }

    #[test]
    fn values_between_two_halves_round_to_the_nearest_and_ties_to_even() {
        // Every pair of neighbouring finite halves, normal and subnormal, positive and negative:
        // their midpoint, exact in f32, goes to the one with an even last bit; one f32 step above
        // or below it goes to the nearer one.
        for bits in 0..0x7bff_u16 {
            for sign in [0, 0x8000] {
                let (low, high) = (bits | sign, (bits + 1) | sign);
                let (a, b) = (f16_to_f32(low), f16_to_f32(high));
                let middle = (a + b) / 2.0;
                let even = if low & 1 == 0 { low } else { high };
                assert_eq!(
                    f32_to_f16(middle),
                    even,
                    "between {low:#06x} and {high:#06x}"
                );
                let towards_a = f32::from_bits(middle.to_bits() - 1);
                let towards_b = f32::from_bits(middle.to_bits() + 1);
                assert_eq!(f32_to_f16(towards_a), low, "just towards {low:#06x}");
                assert_eq!(f32_to_f16(towards_b), high, "just towards {high:#06x}");
            }
        }
    }

    #[test]
    fn magnitudes_beyond_the_largest_half_become_infinities_and_tiny_ones_zeros() {
        // 65 520 lies halfway between 65 504, whose last bit is 1, and 65 536, which half
        // precision does not have: it rounds to infinity, and anything below it to 65 504.
        assert_eq!(f32_to_f16(65_520.0), 0x7c00);
        assert_eq!(f32_to_f16(-65_520.0), 0xfc00);
        assert_eq!(f32_to_f16(65_519.996), 0x7bff);
        // From 65 536 up, beyond any half's exponent.
        assert_eq!(f32_to_f16(65_536.0), 0x7c00);
        assert_eq!(f32_to_f16(1e5), 0x7c00);
        assert_eq!(f32_to_f16(1e10), 0x7c00);
        assert_eq!(f32_to_f16(f32::MAX), 0x7c00);
        assert_eq!(f32_to_f16(f32::INFINITY), 0x7c00);
        assert_eq!(f32_to_f16(f32::NEG_INFINITY), 0xfc00);
        // 2^-25 is halfway between 0 and the smallest subnormal, 2^-24: it rounds to 0, which
        // is even; anything above it to 2^-24.
        let smallest = 1.0 / 16_777_216.0_f32;
        assert_eq!(f32_to_f16(smallest / 2.0), 0x0000);
        assert_eq!(f32_to_f16(-smallest / 2.0), 0x8000);
        assert_eq!(
            f32_to_f16(f32::from_bits((smallest / 2.0).to_bits() + 1)),
            0x0001
        );
        assert_eq!(f32_to_f16(1e-30), 0x0000);
        assert_eq!(f32_to_f16(f32::from_bits(1)), 0x0000);
        assert_eq!(f32_to_f16(-0.0), 0x8000);
    }

    #[test]
    fn a_nan_stays_a_nan() {
        for nan in [
            f32::NAN,
            -f32::NAN,
            f32::from_bits(0x7f80_0001),
            f32::from_bits(0xffc0_1234),
        ] {
            let bits = f32_to_f16(nan);
            assert!(
                f16_to_f32(bits).is_nan(),
                "{:#010x} gave {bits:#06x}",
                nan.to_bits()
            );
            assert_eq!(
                bits & 0x8000,
                ((nan.to_bits() >> 16) & 0x8000) as u16,
                "its sign"
            );
        }
    }
}
