// rust std imports
use std::marker::PhantomData;

// external imports
use slotmap::{Key, KeyData};

// internal imports
use crate::prelude::*;

#[derive(Debug, Default, Eq, PartialEq)]
pub struct NiLink<T> {
    pub key: NiKey,
    phantom: PhantomData<fn() -> T>,
}

impl<T> NiLink<T> {
    #[inline]
    pub const fn new(key: NiKey) -> Self {
        Self {
            key,
            phantom: PhantomData,
        }
    }

    #[inline]
    pub fn null() -> Self {
        Self::new(NiKey::null())
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.key.is_null()
    }

    #[inline]
    pub const fn cast<U>(&self) -> NiLink<U> {
        NiLink::new(self.key)
    }
}

impl<T> Clone for NiLink<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for NiLink<T> {}

impl<T> Load for NiLink<T>
where
    T: Load,
{
    #[allow(clippy::cast_sign_loss)]
    fn load(stream: &mut Reader<'_>) -> io::Result<Self> {
        let idx: i32 = stream.load()?;
        let key = match idx {
            i if (i < 0) => NiKey::null(),
            i => KeyData::from_ffi((1 << 32) | (i as u64 + 1)).into(),
        };
        Ok(Self::new(key))
    }
}

impl<T> Save for NiLink<T>
where
    T: Save,
{
    fn save(&self, stream: &mut Writer) -> io::Result<()> {
        if self.is_null() {
            stream.save(&-1i32)
        } else {
            let key = self.key.data().as_ffi();
            let Some(index) = stream.context.get(&key) else {
                return Writer::error("NiLink target not found");
            };
            stream.save_as::<i32>(*index)
        }
    }
}

//
// Visitor
//

pub trait Visitor {
    fn visitor<F>(&self, f: &mut F)
    where
        F: FnMut(NiKey);

    fn remap_links(&mut self, remap: &HashMap<NiKey, NiKey>);
}

impl<T> Visitor for &T {
    #[inline]
    fn visitor<F>(&self, _: &mut F)
    where
        F: FnMut(NiKey),
    {
    }

    #[inline]
    fn remap_links(&mut self, _: &HashMap<NiKey, NiKey>) {}
}

impl<T> Visitor for &mut T {
    #[inline]
    fn visitor<F>(&self, _: &mut F)
    where
        F: FnMut(NiKey),
    {
    }

    #[inline]
    fn remap_links(&mut self, _: &HashMap<NiKey, NiKey>) {}
}

impl<V: Visitor> Visitor for Option<V> {
    #[inline]
    fn visitor<F>(&self, f: &mut F)
    where
        F: FnMut(NiKey),
    {
        if let Some(inner) = self {
            inner.visitor(f);
        }
    }

    #[inline]
    fn remap_links(&mut self, remap: &HashMap<NiKey, NiKey>) {
        if let Some(inner) = self {
            inner.remap_links(remap);
        }
    }
}

impl<V: Visitor> Visitor for Vec<V> {
    #[inline]
    fn visitor<F>(&self, f: &mut F)
    where
        F: FnMut(NiKey),
    {
        for item in self.iter().rev() {
            item.visitor(f);
        }
    }

    #[inline]
    fn remap_links(&mut self, remap: &HashMap<NiKey, NiKey>) {
        for item in self.iter_mut() {
            item.remap_links(remap);
        }
    }
}

impl<T> Visitor for NiLink<T> {
    #[inline]
    fn visitor<F>(&self, f: &mut F)
    where
        F: FnMut(NiKey),
    {
        f(self.key);
    }

    #[inline]
    fn remap_links(&mut self, remap: &HashMap<NiKey, NiKey>) {
        if let Some(key) = remap.get(&self.key) {
            self.key = *key;
        }
    }
}

impl Visitor for TextureMap {
    #[inline]
    fn visitor<F>(&self, f: &mut F)
    where
        F: FnMut(NiKey),
    {
        match self {
            TextureMap::Map(inner) => inner.visitor(f),
            TextureMap::BumpMap(inner) => inner.visitor(f),
        }
    }

    #[inline]
    fn remap_links(&mut self, remap: &HashMap<NiKey, NiKey>) {
        match self {
            TextureMap::Map(inner) => inner.remap_links(remap),
            TextureMap::BumpMap(inner) => inner.remap_links(remap),
        }
    }
}

impl Visitor for TextureSource {
    #[inline]
    fn visitor<F>(&self, f: &mut F)
    where
        F: FnMut(NiKey),
    {
        match self {
            TextureSource::External(inner) => inner.visitor(f),
            TextureSource::Internal(inner) => inner.visitor(f),
        }
    }

    #[inline]
    fn remap_links(&mut self, remap: &HashMap<NiKey, NiKey>) {
        match self {
            TextureSource::External(_) => {}
            TextureSource::Internal(inner) => inner.remap_links(remap),
        }
    }
}

//
// Inspect
//

/// Implemented by every `Ni*` type via `#[derive(Meta)]` to support generic reflection over
/// their fields, including those declared on base types, for use by tools like inspectors.
#[cfg(feature = "inspect")]
pub trait Inspect {
    /// The name of this struct, e.g. `"NiNode"`.
    fn struct_name(&self) -> &'static str;

    /// This struct's own fields (not including any inherited from `base`), as
    /// `(field name, formatted value)` pairs, in declaration order.
    fn own_properties(&self) -> Vec<(&'static str, String)>;

    /// The `base` object one level up the inheritance chain, if any.
    fn base_object(&self) -> Option<&dyn Inspect>;
}

#[cfg(feature = "inspect")]
impl dyn Inspect + '_ {
    /// All properties of this object and its bases, most-derived first, as
    /// `(struct name, field name, formatted value)` tuples.
    pub fn all_properties(&self) -> Vec<(&'static str, &'static str, String)> {
        let mut out = Vec::new();
        let mut cur: Option<&dyn Inspect> = Some(self);
        while let Some(obj) = cur {
            let struct_name = obj.struct_name();
            out.extend(
                obj.own_properties()
                    .into_iter()
                    .map(|(name, value)| (struct_name, name, value)),
            );
            cur = obj.base_object();
        }
        out
    }
}
