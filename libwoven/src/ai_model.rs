//! Bounded local-provider inference primitive for WovenAI Stage 23.2.
//!
//! This is deliberately a small, deterministic model contract rather than a
//! claim of general-purpose neural inference. It uses fixed integer weights,
//! no allocation, and a bounded feature vocabulary so the broker can account
//! for every byte and operation. Larger tensor/model formats belong to later
//! provider work.

pub const MODEL_ABI: u64 = 0x0002_0000;
pub const LOCAL_PROVIDER_FLAG: u64 = 1 << 32;
pub const CLASS_GREETING: u8 = 1;
pub const CLASS_STATUS: u8 = 2;
pub const CLASS_HELP: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inference {
    pub class: u8,
    pub score: u16,
}

/// Run the fixed-weight local model. `input` is already UTF-8 validated by
/// the WAI protocol; non-ASCII bytes are ignored by the ASCII feature map.
pub fn infer(input: &[u8]) -> Inference {
    let features = [
        contains(input, b"hello") || contains(input, b"hi"),
        contains(input, b"status") || contains(input, b"state"),
        contains(input, b"help") || contains(input, b"how"),
        contains(input, b"thanks") || contains(input, b"thank"),
    ];
    let scores = [
        score(features, [6, 0, 0, 2]),
        score(features, [0, 6, 0, 0]),
        score(features, [0, 0, 6, 0]),
    ];
    let mut best = 0usize;
    for index in 1..scores.len() {
        if scores[index] > scores[best] {
            best = index;
        }
    }
    Inference {
        class: (best + 1) as u8,
        score: scores[best],
    }
}

/// Pack the bounded class and confidence into the WAI response value.
pub const fn encode_result(inference: Inference) -> u64 {
    inference.class as u64 | ((inference.score as u64) << 32)
}

fn score(features: [bool; 4], weights: [u16; 4]) -> u16 {
    let mut total = 0u16;
    let mut index = 0;
    while index < features.len() {
        if features[index] {
            total = total.saturating_add(weights[index]);
        }
        index += 1;
    }
    total
}

fn contains(input: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > input.len() {
        return false;
    }
    input.windows(needle.len()).any(|window| {
        window
            .iter()
            .zip(needle)
            .all(|(byte, expected)| byte.to_ascii_lowercase() == *expected)
    })
}

#[cfg(test)]
mod tests {
    use super::{encode_result, infer, CLASS_GREETING, CLASS_HELP, CLASS_STATUS};

    #[test]
    fn fixed_model_classifies_bounded_features() {
        assert_eq!(infer(b"Hello there").class, CLASS_GREETING);
        assert_eq!(infer(b"what is the status").class, CLASS_STATUS);
        assert_eq!(infer(b"how do I get help").class, CLASS_HELP);
    }

    #[test]
    fn result_encoding_is_stable_and_bounded() {
        let result = infer(b"hello");
        assert_eq!(encode_result(result), 1 | (6u64 << 32));
    }
}
