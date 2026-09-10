//! THE B6 CEILING PROBE: what would key dispatch cost if it were free?
//!
//! Brick B6 proposes matching object keys on the JSON side, and B6h proposes
//! handing a field INDEX across the serde seam instead of a string. The
//! mission plan gates both on one measurement -- "stub key matching with an
//! index lookup on S1 struct parse; this probe also decides B6h" -- and this is
//! that stub.
//!
//! # What is compared
//!
//! [`FrameTelemetryFast`] has the same fields, the same types and the same wire
//! names as [`crate::s4_frame_telemetry::FrameTelemetry`]. The ONLY difference
//! is how a key becomes a field: the derive emits a `match` on `&str`, which
//! lowers to a length switch and then a chain of `memcmp` calls; this one
//! dispatches on the length and one or two individual bytes, with no string
//! comparison anywhere.
//!
//! `s4-frame-telemetry.json` is the right document for it: 2,600 objects
//! sharing one exact 18-key tuple, 46,816 keys at 80.3 keys/KB, and 33
//! distinct names reused 1,418 times each. It is the highest key density in
//! the corpus by a wide margin, so if key dispatch cannot be shown to matter
//! here it cannot matter anywhere.
//!
//! # THIS IS A CEILING, NOT A CANDIDATE, AND IT IS NOT SHIPPABLE
//!
//! The fast matcher does not verify the whole key. `"qp"` and `"qz"` both have
//! length 2 and first byte `q`, so both map to the `qp` field. That is fine for
//! an upper bound on a document whose keys are known, and it is exactly why
//! this lives in the harness and never in the library: a real brick would have
//! to keep the full comparison, or verify some other way, and would therefore
//! be SLOWER than this number. Read it as "key dispatch cannot be worth more
//! than X", never as "a brick would gain X".
//!
//! To keep the comparison honest about work done rather than only about time,
//! the probe asserts the two paths produce equal values on the real document
//! before it times anything.

#![allow(clippy::doc_markdown)]

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;

use crate::s4_frame_telemetry::{FrameType, Summary};

/// Mirrors `FrameTelemetry`, but its frames use the fast field dispatch.
#[derive(Deserialize)]
pub struct FrameTelemetryFast {
    pub encoder: String,
    pub clip: String,
    pub preset: String,
    pub tune: Option<String>,
    pub crf: f64,
    pub threads: u32,
    pub started_at: String,
    pub frames: Vec<FrameFast>,
    pub summary: Summary,
}

/// Mirrors `Frame` exactly. Only `Deserialize` differs.
pub struct FrameFast {
    pub n: u32,
    pub pts: u64,
    pub dts: u64,
    pub t: FrameType,
    pub qp: f64,
    pub bits: u32,
    pub psnr_y: f64,
    pub psnr_u: f64,
    pub psnr_v: f64,
    pub ssim: f64,
    pub mb_i: u32,
    pub mb_p: u32,
    pub mb_b: u32,
    pub mb_skip: u32,
    pub satd: u32,
    pub refs: Vec<u32>,
    pub cpb: f64,
    pub ms: f64,
}

/// The eighteen fields, as indices.
///
/// The names separate perfectly on length plus one or two bytes:
///
/// ```text
/// len 1  n t
/// len 2  qp ms                      -> byte 0
/// len 3  pts dts cpb ref            -> byte 0
/// len 4  bits ssim satd mb_i mb_p mb_b
///                                  -> byte 0, then byte 1 for s*, byte 3 for mb_*
/// len 6  psnr_y psnr_u psnr_v       -> byte 5
/// len 7  mb_skip                    -> unique
/// ```
///
/// So no `memcmp` is needed at all. That is the whole point: this is the
/// cheapest dispatch the field set physically admits, and therefore the
/// ceiling.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    N,
    Pts,
    Dts,
    T,
    Qp,
    Bits,
    PsnrY,
    PsnrU,
    PsnrV,
    Ssim,
    MbI,
    MbP,
    MbB,
    MbSkip,
    Satd,
    Refs,
    Cpb,
    Ms,
    Unknown,
}

/// Length plus one or two bytes, no string comparison.
#[inline(always)]
fn dispatch(key: &[u8]) -> Field {
    match key.len() {
        1 => match key[0] {
            b'n' => Field::N,
            b't' => Field::T,
            _ => Field::Unknown,
        },
        2 => match key[0] {
            b'q' => Field::Qp,
            b'm' => Field::Ms,
            _ => Field::Unknown,
        },
        3 => match key[0] {
            b'p' => Field::Pts,
            b'd' => Field::Dts,
            b'c' => Field::Cpb,
            b'r' => Field::Refs,
            _ => Field::Unknown,
        },
        4 => match key[0] {
            b'b' => Field::Bits,
            // `ssim` and `satd` share a first byte; byte 1 splits them.
            b's' => match key[1] {
                b's' => Field::Ssim,
                b'a' => Field::Satd,
                _ => Field::Unknown,
            },
            // `mb_i` / `mb_p` / `mb_b` differ only at byte 3.
            b'm' => match key[3] {
                b'i' => Field::MbI,
                b'p' => Field::MbP,
                b'b' => Field::MbB,
                _ => Field::Unknown,
            },
            _ => Field::Unknown,
        },
        // `psnr_y` / `psnr_u` / `psnr_v` differ only at byte 5.
        6 => match key[5] {
            b'y' => Field::PsnrY,
            b'u' => Field::PsnrU,
            b'v' => Field::PsnrV,
            _ => Field::Unknown,
        },
        7 => Field::MbSkip,
        _ => Field::Unknown,
    }
}

struct FieldKey(Field);

impl<'de> Deserialize<'de> for FieldKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = FieldKey;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a frame field name")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<FieldKey, E> {
                Ok(FieldKey(dispatch(v.as_bytes())))
            }
            fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<FieldKey, E> {
                Ok(FieldKey(dispatch(v)))
            }
        }
        deserializer.deserialize_identifier(V)
    }
}

impl<'de> Deserialize<'de> for FrameFast {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FrameVisitor;

        impl<'de> Visitor<'de> for FrameVisitor {
            type Value = FrameFast;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("struct FrameFast")
            }

            fn visit_map<A>(self, mut map: A) -> Result<FrameFast, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut n = None;
                let mut pts = None;
                let mut dts = None;
                let mut t = None;
                let mut qp = None;
                let mut bits = None;
                let mut psnr_y = None;
                let mut psnr_u = None;
                let mut psnr_v = None;
                let mut ssim = None;
                let mut mb_i = None;
                let mut mb_p = None;
                let mut mb_b = None;
                let mut mb_skip = None;
                let mut satd = None;
                let mut refs = None;
                let mut cpb = None;
                let mut ms = None;

                while let Some(FieldKey(key)) = map.next_key::<FieldKey>()? {
                    match key {
                        Field::N => n = Some(map.next_value()?),
                        Field::Pts => pts = Some(map.next_value()?),
                        Field::Dts => dts = Some(map.next_value()?),
                        Field::T => t = Some(map.next_value()?),
                        Field::Qp => qp = Some(map.next_value()?),
                        Field::Bits => bits = Some(map.next_value()?),
                        Field::PsnrY => psnr_y = Some(map.next_value()?),
                        Field::PsnrU => psnr_u = Some(map.next_value()?),
                        Field::PsnrV => psnr_v = Some(map.next_value()?),
                        Field::Ssim => ssim = Some(map.next_value()?),
                        Field::MbI => mb_i = Some(map.next_value()?),
                        Field::MbP => mb_p = Some(map.next_value()?),
                        Field::MbB => mb_b = Some(map.next_value()?),
                        Field::MbSkip => mb_skip = Some(map.next_value()?),
                        Field::Satd => satd = Some(map.next_value()?),
                        Field::Refs => refs = Some(map.next_value()?),
                        Field::Cpb => cpb = Some(map.next_value()?),
                        Field::Ms => ms = Some(map.next_value()?),
                        Field::Unknown => {
                            let _ = map.next_value::<de::IgnoredAny>()?;
                        }
                    }
                }

                macro_rules! need {
                    ($v:ident) => {
                        $v.ok_or_else(|| de::Error::missing_field(stringify!($v)))?
                    };
                }
                Ok(FrameFast {
                    n: need!(n),
                    pts: need!(pts),
                    dts: need!(dts),
                    t: need!(t),
                    qp: need!(qp),
                    bits: need!(bits),
                    psnr_y: need!(psnr_y),
                    psnr_u: need!(psnr_u),
                    psnr_v: need!(psnr_v),
                    ssim: need!(ssim),
                    mb_i: need!(mb_i),
                    mb_p: need!(mb_p),
                    mb_b: need!(mb_b),
                    mb_skip: need!(mb_skip),
                    satd: need!(satd),
                    refs: need!(refs),
                    cpb: need!(cpb),
                    ms: need!(ms),
                })
            }

            /// Not reached by this corpus, but a `Visitor` that only handles
            /// maps would be a different shape from the derive's and could
            /// skew the comparison.
            fn visit_seq<A>(self, _seq: A) -> Result<FrameFast, A::Error>
            where
                A: SeqAccess<'de>,
            {
                Err(de::Error::custom("FrameFast expects a map"))
            }
        }

        deserializer.deserialize_struct("FrameFast", FIELDS, FrameVisitor)
    }
}

/// The same list, in the same order, that the derive would pass. Handed to
/// `deserialize_struct` so the JSON side sees an identical call.
const FIELDS: &[&str] = &[
    "n", "pts", "dts", "t", "qp", "bits", "psnr_y", "psnr_u", "psnr_v", "ssim", "mb_i", "mb_p",
    "mb_b", "mb_skip", "satd", "ref", "cpb", "ms",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The dispatch must agree with the real names, and must not claim a name
    /// it was not given. The second half is what makes the ceiling honest
    /// about which keys it would get WRONG.
    #[test]
    fn dispatch_maps_every_real_field() {
        let expected = [
            ("n", Field::N),
            ("pts", Field::Pts),
            ("dts", Field::Dts),
            ("t", Field::T),
            ("qp", Field::Qp),
            ("bits", Field::Bits),
            ("psnr_y", Field::PsnrY),
            ("psnr_u", Field::PsnrU),
            ("psnr_v", Field::PsnrV),
            ("ssim", Field::Ssim),
            ("mb_i", Field::MbI),
            ("mb_p", Field::MbP),
            ("mb_b", Field::MbB),
            ("mb_skip", Field::MbSkip),
            ("satd", Field::Satd),
            ("ref", Field::Refs),
            ("cpb", Field::Cpb),
            ("ms", Field::Ms),
        ];
        for (name, want) in expected {
            assert_eq!(dispatch(name.as_bytes()), want, "dispatch failed on {name}");
        }
        assert_eq!(expected.len(), FIELDS.len());
    }

    /// The ceiling is a ceiling BECAUSE it is wrong here, and that is recorded
    /// rather than hidden: a key it never saw can collide onto a real field.
    /// If this test ever fails because the dispatch became exact, the probe
    /// has turned into a candidate and should be re-read as one.
    #[test]
    fn the_ceiling_is_deliberately_not_exact() {
        assert_eq!(dispatch(b"qz"), Field::Qp, "expected the known collision");
        assert_eq!(dispatch(b"pXX"), Field::Pts, "expected the known collision");
    }
}
