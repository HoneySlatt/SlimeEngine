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
}
