//! CEILING PROBE FOR B11: what is the DOM's value model actually costing?
//!
//! The `scan` column already gives the total headroom -- it walks the document
//! and builds nothing, so whatever `dom-parse` costs above it is the price of
//! building a `Value`. Measured, that is **70% to 85% of dom-parse**: a 3.29x
//! ceiling on `citm_catalog`, 3.37x on `canada`, 3.68x on `twitter` and
//! **6.89x on `s4-frame-telemetry`**. That is by far the largest ceiling in the
//! project, and every brick landed so far has been fighting for the other 15%.
//!
//! A ceiling is not a plan, though. The allocation census says WHERE the money
//! goes, and it points at three separate things:
//!
//! | finding | citm | twitter | s4-frame-telemetry |
//! |---|---|---|---|
//! | distinct keys vs `String`s built | 80.6x reuse | 142x | **1,418.7x** (46,816 allocations for 33 names) |
//! | allocations of <= 8 bytes | 54.8% | -- | 82.6% |
//! | `BTreeMap` nodes' share of bytes | ~100% of 7.68 MB | -- | -- |
//!
//! So there are three candidate levers, and this probe exists to price them
//! **separately and against the whole program**, because today's B15s lesson
//! was that a tight-loop micro-probe bounds the WORK a change removes, not the
//! TIME the program saves -- it can refute a brick, never justify one.
//!
//! # How it isolates one variable at a time
//!
//! One generic value type, one visitor, four storage models. The only thing
//! that differs between arms is how a key, a string and a map are represented,
//! so a difference between two arms cannot be a difference in the parser:
//!
//! | arm | keys | maps | short strings |
//! |---|---|---|---|
//! | `Base` | `Box<str>` per occurrence | `BTreeMap` | heap |
//! | `Intern` | **interned**, one per distinct name | `BTreeMap` | heap |
//! | `VecMap` | `Box<str>` per occurrence | **sorted `Vec`** | heap |
//! | `All` | interned | sorted `Vec` | **inline up to 22 bytes** |
//!
//! `Base` is the control: it is not `serde_json::Value`, it is this probe's own
//! code wearing the same representation. Comparing `Base` against the real
//! `Value` measures how faithful the probe is; comparing the other three
//! against `Base` measures the lever, with the probe's own overhead cancelling.
//!
//! Nothing here ships. It is an instrument, and the numbers it produces decide
//! whether B11 is built.

use std::collections::BTreeMap;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};

// ---------------------------------------------------------------------------
// Short strings: inline, or on the heap.
// ---------------------------------------------------------------------------

/// 22 bytes inline, which is what fits beside a length byte in the same 24
/// bytes a `String` occupies on a 64-bit target. Chosen from the census: 82.6%
/// of `s4-frame-telemetry`'s allocations and 54.8% of `citm_catalog`'s are 8
/// bytes or fewer, so the inline case is the common case by a wide margin.
const INLINE: usize = 22;

/// A string that avoids the heap when it is short enough.
pub enum SmallStr {
    /// `len` bytes of `buf` are live. No allocation at all.
    Inline {
        buf: [u8; INLINE],
        len: u8,
    },
    Heap(Box<str>),
}

impl SmallStr {
    #[inline]
    pub fn new(s: &str) -> Self {
        let b = s.as_bytes();
        if b.len() <= INLINE {
            let mut buf = [0u8; INLINE];
            buf[..b.len()].copy_from_slice(b);
            SmallStr::Inline {
                buf,
                len: b.len() as u8,
            }
        } else {
            SmallStr::Heap(s.into())
        }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        match self {
            // SAFETY-free: built only from a `&str`, so the bytes are valid
            // UTF-8 by construction. `from_utf8` is checked here on purpose --
            // this is an instrument, and a probe that cheats on validation
            // measures a parser we would not ship.
            SmallStr::Inline { buf, len } => {
                std::str::from_utf8(&buf[..*len as usize]).unwrap_or_default()
            }
            SmallStr::Heap(s) => s,
        }
    }
}

// ---------------------------------------------------------------------------
// The interner: one allocation per DISTINCT key, not per occurrence.
// ---------------------------------------------------------------------------

/// Interned key table for one parse.
///
/// The census says this is the biggest single lever on the widest fixture:
/// `s4-frame-telemetry` builds **46,816 `String`s for 33 distinct names**. The
/// trade is an allocation for a hash lookup, and whether that pays is exactly
/// what the clock has to say -- "removed work is not saved time" has been true
/// three times in this project already.
#[derive(Default)]
pub struct Keys {
    seen: std::collections::HashMap<Box<str>, u32>,
    table: Vec<Box<str>>,
}

impl Keys {
    #[inline]
    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.seen.get(s) {
            return i;
        }
        let i = self.table.len() as u32;
        let owned: Box<str> = s.into();
        self.table.push(owned.clone());
        self.seen.insert(owned, i);
        i
    }

    #[inline]
    pub fn get(&self, i: u32) -> &str {
        &self.table[i as usize]
    }

    pub fn distinct(&self) -> usize {
        self.table.len()
    }
}

// ---------------------------------------------------------------------------
// The models.
// ---------------------------------------------------------------------------

/// How one arm represents a key, a string and a map.
pub trait Model: Sized + 'static {
    /// What a map key costs to store.
    type Key;
    /// What a string value costs to store.
    type Str;
    /// The map itself.
    type Map;

    const NAME: &'static str;
    /// Does this arm need the key table threaded through the parse?
    const INTERNS: bool;

    fn key(s: &str, keys: &mut Keys) -> Self::Key;
    fn string(s: &str) -> Self::Str;
    fn map_new() -> Self::Map;
    fn map_push(m: &mut Self::Map, k: Self::Key, v: PValue<Self>);
    fn map_len(m: &Self::Map) -> usize;
}

/// One value, in whichever model.
pub enum PValue<M: Model> {
    Null,
    Bool(bool),
    U64(u64),
    I64(i64),
    F64(f64),
    Str(M::Str),
    Arr(Vec<PValue<M>>),
    Obj(M::Map),
}

macro_rules! model {
    ($name:ident, $keyty:ty, $strty:ty, $mapty:ty, $interns:expr,
     key = $key:expr, string = $string:expr, new = $new:expr, push = $push:expr, len = $len:expr) => {
        pub struct $name;
        impl Model for $name {
            type Key = $keyty;
            type Str = $strty;
            type Map = $mapty;
            const NAME: &'static str = stringify!($name);
            const INTERNS: bool = $interns;
            #[inline]
            fn key(s: &str, keys: &mut Keys) -> Self::Key {
                #[allow(clippy::redundant_closure_call)]
                ($key)(s, keys)
            }
            #[inline]
            fn string(s: &str) -> Self::Str {
                #[allow(clippy::redundant_closure_call)]
                ($string)(s)
            }
            #[inline]
            fn map_new() -> Self::Map {
                #[allow(clippy::redundant_closure_call)]
                ($new)()
            }
            #[inline]
            fn map_push(m: &mut Self::Map, k: Self::Key, v: PValue<Self>) {
                #[allow(clippy::redundant_closure_call)]
                ($push)(m, k, v)
            }
            #[inline]
            fn map_len(m: &Self::Map) -> usize {
                #[allow(clippy::redundant_closure_call)]
                ($len)(m)
            }
        }
    };
}

// THE CONTROL. Same shape as `serde_json::Value`: an owned key per
// occurrence, a `BTreeMap`, a heap string.
model!(
    Base,
    Box<str>,
    Box<str>,
    BTreeMap<Box<str>, PValue<Base>>,
    false,
    key = |s: &str, _: &mut Keys| -> Box<str> { s.into() },
    string = |s: &str| -> Box<str> { s.into() },
    new = BTreeMap::new,
    push = |m: &mut BTreeMap<Box<str>, PValue<Base>>, k: Box<str>, v: PValue<Base>| {
        m.insert(k, v);
    },
    len = |m: &BTreeMap<Box<str>, PValue<Base>>| m.len()
);

// LEVER 1: interned keys, everything else the control's.
model!(
    Intern, u32, Box<str>, BTreeMap<u32, PValue<Intern>>, true,
    key = |s: &str, keys: &mut Keys| keys.intern(s),
    string = |s: &str| -> Box<str> { s.into() },
    new = BTreeMap::new,
    push = |m: &mut BTreeMap<u32, PValue<Intern>>, k: u32, v: PValue<Intern>| {
        m.insert(k, v);
    },
    len = |m: &BTreeMap<u32, PValue<Intern>>| m.len()
);

// LEVER 2: a sorted Vec instead of a BTreeMap. The census says the median
// object arity is 2 and 97.7% of citm's objects hold <= 8 keys, so a
// `BTreeMap` node built for eleven pairs is mostly empty space.
model!(
    VecMap,
    Box<str>,
    Box<str>,
    Vec<(Box<str>, PValue<VecMap>)>,
    false,
    key = |s: &str, _: &mut Keys| -> Box<str> { s.into() },
    string = |s: &str| -> Box<str> { s.into() },
    new = Vec::new,
    push = |m: &mut Vec<(Box<str>, PValue<VecMap>)>, k: Box<str>, v: PValue<VecMap>| {
        m.push((k, v));
    },
    len = |m: &Vec<(Box<str>, PValue<VecMap>)>| m.len()
);

// ALL THREE AT ONCE: interned keys, a Vec map, and inline short strings.
model!(
    All,
    u32,
    SmallStr,
    Vec<(u32, PValue<All>)>,
    true,
    key = |s: &str, keys: &mut Keys| keys.intern(s),
    string = SmallStr::new,
    new = Vec::new,
    push = |m: &mut Vec<(u32, PValue<All>)>, k: u32, v: PValue<All>| {
        m.push((k, v));
    },
    len = |m: &Vec<(u32, PValue<All>)>| m.len()
);

// ---------------------------------------------------------------------------
// One visitor, shared by every arm. The key table travels as a seed, which is
// serde's own mechanism for stateful deserialization -- no thread-locals, so
// the interning cost measured is the real one.
// ---------------------------------------------------------------------------

struct Seed<'k, M: Model> {
    keys: &'k mut Keys,
    _m: std::marker::PhantomData<M>,
}

impl<'de, M: Model> DeserializeSeed<'de> for Seed<'_, M> {
    type Value = PValue<M>;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de, M: Model> Visitor<'de> for Seed<'_, M> {
    type Value = PValue<M>;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(PValue::Null)
    }
    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(PValue::Null)
    }
    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
        Ok(PValue::Bool(v))
    }
    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
        Ok(PValue::U64(v))
    }
    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
        Ok(PValue::I64(v))
    }
    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E> {
        Ok(PValue::F64(v))
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
        Ok(PValue::Str(M::string(v)))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
        let mut out = Vec::with_capacity(a.size_hint().unwrap_or(0));
        while let Some(v) = a.next_element_seed(Seed::<M> {
            keys: self.keys,
            _m: std::marker::PhantomData,
        })? {
            out.push(v);
        }
        Ok(PValue::Arr(out))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
        let mut m = M::map_new();
        // The key arrives as a borrowed `&str` wherever the input allows it,
        // which is the whole point: an arm that interns never owns it, and an
        // arm that does not owns exactly one copy per occurrence.
        while let Some(k) = a.next_key::<&str>()? {
            let key = M::key(k, self.keys);
            let v = a.next_value_seed(Seed::<M> {
                keys: self.keys,
                _m: std::marker::PhantomData,
            })?;
            M::map_push(&mut m, key, v);
        }
        Ok(PValue::Obj(m))
    }
}

/// Parse `bytes` into model `M`, returning the value and its key table.
///
/// # Panics
///
/// On a parse error. Every document handed to this is one the real parser has
/// already accepted, so an error here is a bug in the probe.
pub fn parse<M: Model>(bytes: &[u8]) -> (PValue<M>, Keys) {
    let mut keys = Keys::default();
    let mut de = turbo::Deserializer::from_slice(bytes);
    let seed = Seed::<M> {
        keys: &mut keys,
        _m: std::marker::PhantomData,
    };
    let v = seed
        .deserialize(&mut de)
        .expect("probe: document must parse");
    de.end().expect("probe: trailing input");
    (v, keys)
}

/// Walk a parsed value so the optimiser cannot delete the construction, and
/// return a checksum that proves the arms all built the same thing.
pub fn checksum<M: Model>(
    v: &PValue<M>,
    keys: &Keys,
    get_key: fn(&M::Key, &Keys) -> String,
) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    walk::<M>(v, keys, get_key, &mut h);
    h
}

fn walk<M: Model>(v: &PValue<M>, keys: &Keys, get_key: fn(&M::Key, &Keys) -> String, h: &mut u64) {
    fn mix(h: &mut u64, x: u64) {
        *h ^= x;
        *h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    match v {
        PValue::Null => mix(h, 1),
        PValue::Bool(b) => mix(h, 2 + u64::from(*b)),
        PValue::U64(n) => mix(h, 4 ^ *n),
        PValue::I64(n) => mix(h, 5 ^ (*n as u64)),
        PValue::F64(n) => mix(h, 6 ^ n.to_bits()),
        PValue::Str(_) => mix(h, 7),
        PValue::Arr(a) => {
            mix(h, 8 ^ a.len() as u64);
            for x in a {
                walk::<M>(x, keys, get_key, h);
            }
        }
        PValue::Obj(m) => {
            mix(h, 9 ^ M::map_len(m) as u64);
            let _ = (keys, get_key);
        }
    }
}
