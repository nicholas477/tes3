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

/// A single property's value
#[cfg(feature = "inspect")]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Property {
    pub type_name: String,
    pub name: &'static str,
    pub value: String,
}

#[cfg(feature = "inspect")]
impl Property {
    /// The name of this property's Rust type, e.g. `"u32"` or `"NiNode"`.
    pub fn type_name(&self) -> &str {
        self.type_name.as_str()
    }

    /// Actual name of the property
    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

/// Implemented by every `Ni*` type via `#[derive(Meta)]` to support generic reflection over
/// their fields, including those declared on base types, for use by tools like inspectors.
#[cfg(feature = "inspect")]
pub trait Inspect {
    /// This struct's own fields as `(field name, value)` pairs, in declaration order.
    fn properties(&self) -> Vec<Property>;
}

#[cfg(feature = "inspect")]
impl Inspect for Property {
    #[inline]
    fn properties(&self) -> Vec<Property> {
        vec![self.clone()]
    }
}

/// Links are just indices into the file's object table, not useful to show in an inspector.
// #[cfg(feature = "inspect")]
// impl<T> Inspect for NiLink<T> {
//     #[inline]
//     fn properties(&self) -> Vec<Property> {
//         vec![Property {
//             type_name: "NiLink",
//             name: "",
//             value: String::new(),
//         }]
//     }
// }

// #[cfg(feature = "inspect")]
// impl<T> Inspect for Option<NiLink<T>> {
//     #[inline]
//     fn properties(&self) -> Vec<Property> {
//         Vec::new()
//     }
// }

// #[cfg(feature = "inspect")]
// impl<T> Inspect for Vec<NiLink<T>> {
//     #[inline]
//     fn properties(&self) -> Vec<Property> {
//         Vec::new()
//     }
// }

/// Wraps a field's name and value so the resulting `Property`/`Vec<Property>` can be chosen
/// based on whether the field's type implements `Inspect`, via autoref-based specialization.
#[cfg(feature = "inspect")]
#[doc(hidden)]
pub struct Wrap<'a, T>(pub &'static str, pub &'a T);

#[cfg(feature = "inspect")]
impl<T> std::fmt::Debug for Wrap<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Wrap").field(&self.0).finish()
    }
}

/// Matched first: fields whose type implements `Inspect` contribute their own properties.
#[cfg(feature = "inspect")]
#[doc(hidden)]
pub trait ToPropertiesAsObject {
    fn to_properties(&self) -> Vec<Property>;
}

#[cfg(feature = "inspect")]
impl<T: Inspect> ToPropertiesAsObject for Wrap<'_, T> {
    #[inline]
    fn to_properties(&self) -> Vec<Property> {
        self.1.properties()
    }
}

/// Matched as a fallback: anything else becomes a single leaf property via `Debug`.
#[cfg(feature = "inspect")]
#[doc(hidden)]
pub trait ToPropertiesAsDebug {
    fn to_properties(&self) -> Vec<Property>;
}

#[cfg(feature = "inspect")]
impl<T> ToPropertiesAsDebug for Wrap<'_, Vec<T>> {
    #[inline]
    fn to_properties(&self) -> Vec<Property> {
        vec![Property {
            type_name: clean_nested_type_pure(std::any::type_name::<Vec<T>>()),
            name: self.0,
            value: self.1.len().to_string(),
        }]
    }
}

#[cfg(feature = "inspect")]
fn clean_nested_type_pure(input: &str) -> String {
    let mut result = String::new();
    let mut current_segment = String::new();

    for c in input.chars() {
        match c {
            '<' | '>' | ',' | ' ' => {
                // Clean the segment built up so far and add it to the result
                if let Some(last_part) = current_segment.split("::").last() {
                    result.push_str(last_part);
                }
                current_segment.clear();
                result.push(c);
            }
            _ => {
                current_segment.push(c);
            }
        }
    }

    // Catch any remaining text if the string doesn't end with a delimiter
    if !current_segment.is_empty() {
        if let Some(last_part) = current_segment.split("::").last() {
            result.push_str(last_part);
        }
    }

    result
}

#[cfg(feature = "inspect")]
impl<T: std::fmt::Debug> ToPropertiesAsDebug for &Wrap<'_, T> {
    #[inline]
    fn to_properties(&self) -> Vec<Property> {
        vec![Property {
            type_name: clean_nested_type_pure(std::any::type_name::<T>()),
            name: self.0,
            value: format!("{:?}", self.1),
        }]
    }
}

#[cfg(all(test, feature = "inspect"))]
mod tests {
    use super::*;

    #[test]
    fn vector_property_displays_its_element_count() {
        let values = vec![10, 20, 30];
        let properties = (&Wrap("values", &values)).to_properties();

        assert_eq!(
            properties,
            vec![Property {
                type_name: "Vec<i32>".to_owned(),
                name: "values",
                value: "3".to_owned(),
            }]
        );
    }
}
