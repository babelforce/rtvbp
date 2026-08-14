pub(super) fn encode_pcmu(pcm: &[u8]) -> Vec<u8> {
    pcm.chunks_exact(2)
        .map(|sample| linear_to_mu_law(i16::from_le_bytes([sample[0], sample[1]])))
        .collect()
}

pub(super) fn decode_pcmu(encoded: &[u8]) -> Vec<u8> {
    encoded
        .iter()
        .flat_map(|sample| mu_law_to_linear(*sample).to_le_bytes())
        .collect()
}

fn linear_to_mu_law(sample: i16) -> u8 {
    const BIAS: i32 = 0x84;
    const CLIP: i32 = 32_635;
    let mut value = i32::from(sample);
    let sign = if value < 0 {
        value = -value;
        0x80
    } else {
        0
    };
    value = value.min(CLIP) + BIAS;
    let mut exponent = 7_u8;
    let mut mask = 0x4000;
    while exponent > 0 && value & mask == 0 {
        exponent -= 1;
        mask >>= 1;
    }
    let mantissa = u8::try_from((value >> (u32::from(exponent) + 3)) & 0x0f).unwrap_or(0);
    !(sign | (exponent << 4) | mantissa)
}

fn mu_law_to_linear(encoded: u8) -> i16 {
    const BIAS: i32 = 0x84;
    let value = !encoded;
    let mut magnitude = (i32::from(value & 0x0f) << 3) + BIAS;
    magnitude <<= (value & 0x70) >> 4;
    let linear = if value & 0x80 != 0 {
        BIAS - magnitude
    } else {
        magnitude - BIAS
    };
    i16::try_from(linear).unwrap_or(if linear < 0 { i16::MIN } else { i16::MAX })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_matches_g711_reference_points_and_is_little_endian() {
        let samples = [i16::MIN, -10_000, -1_000, 0, 1_000, 10_000, i16::MAX];
        let pcm: Vec<_> = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let encoded = encode_pcmu(&pcm);
        assert_eq!(encoded, [0, 28, 78, 255, 206, 156, 128]);
        let decoded = decode_pcmu(&encoded);
        let round_trip: Vec<_> = decoded
            .chunks_exact(2)
            .map(|sample| i16::from_le_bytes([sample[0], sample[1]]))
            .collect();
        assert_eq!(round_trip, [-32_124, -9_852, -988, 0, 988, 9_852, 32_124]);
    }
}
