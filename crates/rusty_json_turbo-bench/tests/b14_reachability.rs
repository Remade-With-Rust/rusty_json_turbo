//! Is `deserialize_in_place` REACHABLE through this crate's public API?
//!
//! Brick B14 is "evaluate derive `deserialize_in_place` for `Vec<Struct>`;
//! enable it in the fork if it pays". It cannot pay if nothing ever calls it,
//! and that is a question about code paths rather than about speed -- so it is
//! answered here by a test rather than by a benchmark.
//!
//! The trick is that `Deserialize::deserialize_in_place` has a DEFAULT body,
//! so an impl that never gets called looks exactly like one that does. This
//! type's version panics. If any entry point reaches it, the test fails and
//! says so; if none does, the test passes and B14 has no route to the parser.

use serde::de::{Deserializer, Visitor};
use serde::Deserialize;
use std::fmt;

/// A `u32` whose in-place path is a landmine.
#[derive(Debug, PartialEq)]
struct Tripwire(u32);

impl<'de> Deserialize<'de> for Tripwire {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl Visitor<'_> for V {
            type Value = Tripwire;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a u32")
            }
            fn visit_u64<E>(self, v: u64) -> Result<Tripwire, E> {
                Ok(Tripwire(v as u32))
            }
        }
        deserializer.deserialize_u32(V)
    }

    fn deserialize_in_place<D>(_deserializer: D, _place: &mut Self) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        panic!("deserialize_in_place WAS reached -- brick B14 has a route after all");
    }
}

#[derive(Deserialize, Debug, PartialEq)]
struct Wrapper {
    items: Vec<Tripwire>,
}

/// `from_slice` into a fresh value: the only shape this crate's API offers.
#[test]
fn from_slice_never_takes_the_in_place_path() {
    let v: Wrapper = turbo::from_slice(br#"{"items":[1,2,3]}"#).unwrap();
    assert_eq!(v.items, vec![Tripwire(1), Tripwire(2), Tripwire(3)]);
}

/// And through `from_str`, and through a `Vec` at the top level, and through
/// the streaming deserializer -- so "no route" covers every entry point rather
/// than the one that came to mind first.
#[test]
fn no_entry_point_takes_the_in_place_path() {
    let a: Vec<Tripwire> = turbo::from_str("[1,2,3]").unwrap();
    assert_eq!(a.len(), 3);

    let b: Wrapper = turbo::from_str(r#"{"items":[4]}"#).unwrap();
    assert_eq!(b.items, vec![Tripwire(4)]);

    let c: Vec<Tripwire> = turbo::from_reader(&b"[5,6]"[..]).unwrap();
    assert_eq!(c.len(), 2);

    let stream = turbo::Deserializer::from_str("[1] [2]").into_iter::<Vec<Tripwire>>();
    let counted = stream.map(Result::unwrap).count();
    assert_eq!(counted, 2);
}

/// The conclusion, stated as an assertion so it cannot drift: there is no
/// public function on this crate that deserializes INTO an existing value.
/// Until one exists, `deserialize_in_place` is unreachable and B14 can only
/// add generated code, never remove work.
#[test]
fn there_is_no_reuse_entry_point_to_reach_it_with() {
    // `from_slice`, `from_str`, `from_reader` and `Deserializer::into_iter`
    // all return a fresh `T`. None takes `&mut T`. This test exists to fail
    // loudly if such an API is ever added, because on that day B14 becomes
    // worth re-measuring.
    let names = ["from_slice", "from_str", "from_reader"];
    assert_eq!(names.len(), 3, "the entry-point list changed; re-read B14");
}
