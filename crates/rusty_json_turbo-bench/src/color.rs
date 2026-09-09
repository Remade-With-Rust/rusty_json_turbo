//! Twitter's `profile_*_color` fields: six upper-case hex digits.
//!
//! A safe rewrite of json-benchmark's fixture (which used raw pointer copies
//! into an uninitialised buffer); same wire format, same visitor.

#[cfg(feature = "serde")]
use std::fmt;

#[cfg(feature = "serde")]
use serde::de::{self, Deserialize, Deserializer, Unexpected};
#[cfg(feature = "serde")]
use serde::ser::{Serialize, Serializer};

#[derive(Clone, Copy)]
pub struct Color(u32);

const HEX: &[u8; 16] = b"0123456789ABCDEF";

impl Color {
    fn as_str(self, buf: &mut [u8; 6]) -> &str {
        for (i, slot) in buf.iter_mut().enumerate() {
            let shift = 20 - 4 * i;
            *slot = HEX[((self.0 >> shift) & 0xF) as usize];
        }
        // Every byte came from HEX, so this is ASCII by construction.
        std::str::from_utf8(buf).expect("hex digits are ASCII")
    }
}

#[cfg(feature = "serde")]
impl Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut buf = [0u8; 6];
        serializer.serialize_str(self.as_str(&mut buf))
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Color;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("color string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Color, E>
            where
                E: de::Error,
            {
                match u32::from_str_radix(value, 16) {
                    Ok(hex) => Ok(Color(hex)),
                    Err(_) => Err(E::invalid_value(Unexpected::Str(value), &self)),
                }
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

#[test]
fn test_color() {
    let mut buf = [0u8; 6];
    assert_eq!(Color(0xA0A0A0).as_str(&mut buf), "A0A0A0");
    assert_eq!(Color(0x00_12_AB).as_str(&mut buf), "0012AB");
}
