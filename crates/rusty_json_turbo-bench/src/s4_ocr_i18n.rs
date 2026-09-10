#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// An FFAI result bundle of 30 OCR pages, 30 ASR transcripts and 30 captions:
/// the escape path in both directions, since its `String` fields carry raw
/// UTF-8, `\uXXXX` escapes and surrogate pairs in the same value; the item
/// shapes ride an internally-tagged enum on `kind`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OcrI18n {
    pub bundle: String,
    pub engines: Engines,
    pub note: String,
    pub items: Vec<Item>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Engines {
    pub ocr: String,
    pub asr: String,
    pub vlm: String,
    pub detect: String,
}

/// One result, internally tagged on `kind`.
///
/// An enum, because `kind` determines the key set exactly: 30 `ocr.page`
/// objects share one 9-key tuple, 30 `asr.transcript` objects one 7-key tuple
/// and 30 `vlm.caption` objects one 10-key tuple, overlapping only in `engine`
/// and `source`. The tag values carry a dot, so each variant is renamed.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind"))]
pub enum Item {
    #[cfg_attr(feature = "serde", serde(rename = "ocr.page"))]
    OcrPage(OcrPage),
    #[cfg_attr(feature = "serde", serde(rename = "asr.transcript"))]
    AsrTranscript(AsrTranscript),
    #[cfg_attr(feature = "serde", serde(rename = "vlm.caption"))]
    VlmCaption(VlmCaption),
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OcrPage {
    pub engine: String,
    pub source: String,
    pub detector: String,
    pub recognizer: String,
    pub reading_order: String,
    pub page: PageGeometry,
    pub lines: Vec<OcrLine>,
    pub churn: u32,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PageGeometry {
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
    pub rotation: u32,
}

/// One recognised line. All 689 of them carry the same seven keys, so this is
/// one struct and not a second enum: only `kind` above wanted one.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OcrLine {
    pub index: u32,
    /// The escape-heavy field: raw UTF-8 and `\uXXXX` in one value.
    pub text: String,
    pub conf: f64,
    pub bbox: BBox,
    pub lang: String,
    pub rtl: bool,
    /// Signed.
    pub baseline_skew: f64,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AsrTranscript {
    pub engine: String,
    pub source: String,
    pub model: String,
    pub language: String,
    pub wer_holdout: f64,
    pub segments: Vec<Segment>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Segment {
    pub id: u32,
    pub start: f64,
    pub end: f64,
    pub text: String,
    /// Present on every segment, null on 145 of 492.
    pub speaker: Option<String>,
    /// Signed.
    pub avg_logprob: f64,
    pub no_speech_prob: f64,
    pub words: Vec<Word>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Word {
    pub w: String,
    pub s: f64,
    pub e: f64,
    pub p: f64,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct VlmCaption {
    pub engine: String,
    pub source: String,
    pub model: String,
    pub prompt: String,
    pub caption: String,
    pub alternates: Vec<String>,
    pub tokens: u32,
    pub byte_identical_to_reference: bool,
    pub detections: Vec<Detection>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Detection {
    pub label: String,
    pub conf: f64,
    pub xyxy: BBox,
    pub track_id: u32,
}

/// A fixed-length pixel box, as a tuple, the way `twitter::Indices` is.
pub type BBox = (u32, u32, u32, u32);
