#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// An encoder's per-frame statistics log, 2,600 frames of 1080p30 under one
/// summary: the high-arity field matcher, because every frame object carries
/// the same exact 18-key tuple, so the derive's `__Field` compare chain runs
/// 46,800 times over 18 names.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FrameTelemetry {
    pub encoder: String,
    pub clip: String,
    pub preset: String,
    pub tune: Option<String>,
    pub crf: f64,
    pub threads: u32,
    pub started_at: String,
    pub frames: Vec<Frame>,
    pub summary: Summary,
}

/// One frame. Eighteen fields, no `Option`, no absent key: the arity is the
/// point, so this is one struct and not a per-slice-type enum.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Frame {
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
    // `ref` is a keyword; the wire name is unchanged.
    #[cfg_attr(feature = "serde", serde(rename = "ref"))]
    pub refs: Vec<u32>,
    pub cpb: f64,
    pub ms: f64,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Summary {
    pub frames: u32,
    pub kbps: f64,
    pub bytes: u64,
    pub psnr_avg: f64,
    pub ssim_avg: f64,
    pub encode_s: f64,
    pub fps: f64,
}

enum_str!(FrameType {
    I("I"),
    P("P"),
    B("B"),
});
