use anyhow::{anyhow, Result};
use codepage_437::CP437_WINGDINGS;

// Serialize multi-line content with CR-terminated lines
pub fn encode_multiline(input: &str) -> Result<Vec<u8>> {
    input
        .chars()
        .map(|c| {
            if c == '\n' {
                Ok('\r' as u8)
            } else {
                CP437_WINGDINGS
                    .encode(c)
                    .ok_or(anyhow!("Couldn't encode char: {}", c))
            }
        })
        .collect()
}

pub fn decode_multiline(input: &[u8]) -> String {
    input
        .iter()
        .map(|&x| {
            if x == '\r' as u8 {
                '\n'
            } else {
                CP437_WINGDINGS.decode(x)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use insta::assert_debug_snapshot;

    use crate::encoding::{decode_multiline, encode_multiline};

    fn serialize_code(input: &str) -> Vec<u8> {
        encode_multiline(input).expect("Error in test")
    }

    #[test]
    fn roundtrip_multiline() {
        let bytes: Vec<u8> = (0..=255).collect();
        assert_eq!(bytes, serialize_code(&decode_multiline(&bytes)))
    }

    #[test]
    fn newlines() {
        assert_debug_snapshot!(serialize_code("ABC\nDEF"), @r###"
        [
            65,
            66,
            67,
            13,
            68,
            69,
            70,
        ]
        "###)
    }

    #[test]
    fn wingdings() {
        assert_debug_snapshot!(decode_multiline(&(0..32).collect::<Vec<_>>()), @r###""\0☺☻♥♦♣♠•◘○◙♂♀\n♫☼►◄↕‼¶§▬↨↑↓→←∟↔▲▼""###);
        assert_debug_snapshot!(decode_multiline(&(112..144).collect::<Vec<_>>()), @r###""pqrstuvwxyz{|}~⌂ÇüéâäàåçêëèïîìÄÅ""###);
        assert_debug_snapshot!(decode_multiline(&(224..=255).collect::<Vec<_>>()), @r###""αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{a0}""###);
    }
}
