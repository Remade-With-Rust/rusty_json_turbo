#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap as Map;

use crate::empty;

/// One `rffprobe -show_streams -show_format -show_chapters -print_format json`
/// run over an ingest directory, 75 files and 177 streams: an internally-tagged
/// enum on `codec_type` whose video variant is a 40-field struct, which is the
/// widest field matcher in the corpus, plus `String`-carried numerics.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MediaProbe {
    pub probe_version: String,
    pub probe_flags: Vec<String>,
    pub root: String,
    pub scanned_at: String,
    pub files: Vec<ProbedFile>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ProbedFile {
    pub programs: empty::Array,
    pub streams: Vec<Stream>,
    pub chapters: Vec<Chapter>,
    pub format: Format,
}

/// One stream, internally tagged on `codec_type`.
///
/// An enum and not one struct full of `Option`s, because `codec_type`
/// determines the key set exactly: 75 video objects carry the same 41 keys, 75
/// audio objects the same 26, 27 subtitle objects the same 17, with no
/// intersection beyond the 16 they all share and no key present-in-one-and-
/// absent-in-another within a variant. One struct with `Option`s would need
/// `skip_serializing_if` on 25 fields to round-trip, and that attribute would
/// silently also erase the fields that are present-but-`null` (`profile`,
/// `initial_padding`). The enum keeps absent and null distinct, and it puts
/// serde's `TaggedContentVisitor` buffering under measurement, which is a path
/// S1-S3 never enter.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "codec_type", rename_all = "snake_case")
)]
// The variants differ in size by design: they are the shapes on the wire.
// Boxing to equalise them would add one allocation per stream to the
// struct-parse column, and this fixture exists to measure the derive.
#[allow(clippy::large_enum_variant)]
pub enum Stream {
    Video(VideoStream),
    Audio(AudioStream),
    Subtitle(SubtitleStream),
}

/// Forty fields. `twitter.json`'s 40-field `User` reproduced in a house shape,
/// without twitter's non-ASCII confounding the dispatch measurement.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct VideoStream {
    pub index: u32,
    pub codec_name: String,
    pub codec_long_name: String,
    pub profile: String,
    pub codec_tag_string: String,
    pub codec_tag: String,
    pub width: u32,
    pub height: u32,
    pub coded_width: u32,
    pub coded_height: u32,
    pub closed_captions: u32,
    pub film_grain: u32,
    pub has_b_frames: u32,
    pub sample_aspect_ratio: String,
    pub display_aspect_ratio: String,
    pub pix_fmt: String,
    pub level: u32,
    pub color_range: String,
    pub color_space: String,
    pub color_transfer: String,
    pub color_primaries: String,
    pub chroma_location: String,
    pub field_order: String,
    pub refs: u32,
    /// ffprobe carries this bool as a string; so does the fixture.
    pub is_avc: String,
    pub nal_length_size: String,
    pub id: String,
    pub r_frame_rate: String,
    pub avg_frame_rate: String,
    pub time_base: String,
    /// Signed: the corpus carries start times down to -1105 ticks.
    pub start_pts: i64,
    pub start_time: String,
    pub duration_ts: u32,
    pub duration: String,
    pub bit_rate: String,
    pub bits_per_raw_sample: String,
    pub nb_frames: String,
    pub extradata_size: u32,
    pub disposition: Disposition,
    pub tags: VideoTags,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AudioStream {
    pub index: u32,
    pub codec_name: String,
    pub codec_long_name: String,
    /// Present on every audio stream, null on 49 of 75.
    pub profile: Option<String>,
    pub codec_tag_string: String,
    pub codec_tag: String,
    pub sample_fmt: String,
    pub sample_rate: String,
    pub channels: u32,
    pub channel_layout: String,
    pub bits_per_sample: u32,
    /// Present on every audio stream, null on 56 of 75.
    pub initial_padding: Option<u32>,
    pub id: String,
    pub r_frame_rate: String,
    pub avg_frame_rate: String,
    pub time_base: String,
    pub start_pts: i64,
    pub start_time: String,
    pub duration_ts: u32,
    pub duration: String,
    pub bit_rate: String,
    pub nb_frames: String,
    pub extradata_size: u32,
    pub disposition: Disposition,
    pub tags: AudioTags,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SubtitleStream {
    pub index: u32,
    pub codec_name: String,
    pub codec_long_name: String,
    pub codec_tag_string: String,
    pub codec_tag: String,
    pub id: String,
    pub r_frame_rate: String,
    pub avg_frame_rate: String,
    pub time_base: String,
    pub start_pts: i64,
    pub start_time: String,
    pub duration_ts: u32,
    pub duration: String,
    pub extradata_size: u32,
    pub disposition: Disposition,
    pub tags: SubtitleTags,
}

/// Seventeen integer flags on every stream, whatever its codec type.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Disposition {
    pub default: u32,
    pub dub: u32,
    pub original: u32,
    pub comment: u32,
    pub lyrics: u32,
    pub karaoke: u32,
    pub forced: u32,
    pub hearing_impaired: u32,
    pub visual_impaired: u32,
    pub clean_effects: u32,
    pub attached_pic: u32,
    pub timed_thumbnails: u32,
    pub non_diegetic: u32,
    pub captions: u32,
    pub descriptions: u32,
    pub dependent: u32,
    pub still_image: u32,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct VideoTags {
    pub language: String,
    pub handler_name: String,
    pub vendor_id: String,
    pub encoder: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AudioTags {
    pub language: String,
    pub handler_name: String,
    pub vendor_id: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SubtitleTags {
    pub language: String,
    pub title: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Chapter {
    pub id: u32,
    pub time_base: String,
    pub start: u32,
    pub start_time: String,
    pub end: u32,
    pub end_time: String,
    pub tags: ChapterTags,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ChapterTags {
    pub title: String,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Format {
    pub filename: String,
    pub nb_streams: u32,
    pub nb_programs: u32,
    pub format_name: String,
    pub format_long_name: String,
    pub start_time: String,
    pub duration: String,
    pub size: String,
    pub bit_rate: String,
    pub probe_score: u32,
    /// A container tag bag: twelve keys here, but the wire contract is a map of
    /// arbitrary names to `String` values, so it stays a map.
    pub tags: Map<String, String>,
}
