use std::collections::BTreeMap;
use std::ops::{Deref,DerefMut};

use quote::ToTokens;
use syn;


/// Return camel-cased version of provided ident.
pub fn to_camel_ident(ident: &syn::Ident) -> syn::Ident {
    let ident = ident.to_string();
    let mut out = String::with_capacity(ident.len());

    let mut iter = ident.chars();

    out.push(iter.next().unwrap().to_uppercase().next().unwrap());

    while let Some(c) = iter.next() {
        out.push(match c {
            '_' => match iter.next() {
                Some(c) => c.to_uppercase().next().unwrap(),
                None => break,
            },
            _ => c
        });
    }

    syn::parse_str::<syn::Ident>(&out).unwrap()
}


/// Run over attributes with the provided function, removing attribute when `func` returns `true`.
/// Return the count of removed attributes.
pub fn drain_attrs(attrs: &mut Vec<syn::Attribute>, mut func: impl FnMut(&syn::Attribute) -> bool) -> usize
{
    let (mut i, mut count) = (0,0);
    while i != attrs.len() {
        if func(&attrs[i]) {
            attrs.remove(i);
            count += 1;
        }
        else { i += 1 }
    }
    count
}


pub type AttributesMap = BTreeMap<String, Option<String>>;


/// Read syn::Attribute list into key-values.
///
/// Attribute accepted format:
/// - #[prefix(key=value,key=value)
/// - #[prefix("key",...)]
/// - #[prefix(key)]
///
pub struct Attributes {
    pub attrs: AttributesMap,
}

impl Attributes {
    pub fn new() -> Self {
        Self { attrs: AttributesMap::new() }
    }

    /// Set `default` value for attribute key when not declared or None.
    pub fn set_default<K: Into<String>, D: Into<String>>(&mut self, key: K, default: D) -> String {
        let key = key.into();
        if let Some(Some(v)) = self.attrs.get(&key) {
            return v.clone();
        }
        self.attrs.insert(key.clone(), Some(default.into()));
        self.attrs.get(&key).unwrap().as_ref().unwrap().clone()
    }

    /// Parse attribute into syn entity.
    pub fn get_as<K: Into<String>,T: syn::parse::Parse>(&self, key: K) -> Option<T> {
        match self.attrs.get(&key.into()) {
            Some(Some(v)) => syn::parse_str(&v).ok(),
            _ => None
        }
    }

    /// Create new Attributes reading from provided `syn::Attribute`s
    pub fn from_attrs(prefix: &str, attrs: &mut Vec<syn::Attribute>) -> Self {
        let mut this = Self::new();
        this.read_attrs(prefix, attrs);
        this
    }

    /// Read attributes draining them when attribute has provided prefix.
    pub fn read_attrs(&mut self, prefix: &str, attrs: &mut Vec<syn::Attribute>) {
        drain_attrs(attrs, |attr| {
            if !attr.path.is_ident(prefix) {
                return false;
            }

            match attr.parse_meta() {
                Ok(syn::Meta::List(meta)) => {
                    for nested in meta.nested.iter() {
                        self.insert_nested(nested)
                    }
                    true
                },
                _ => false,
            }
        });
    }

    /// Add attribute from `syn::NestedMeta`.
    fn insert_nested(&mut self, meta: &syn::NestedMeta) {
        match meta {
            syn::NestedMeta::Meta(syn::Meta::Path(path)) => {
                let key = match path.get_ident() {
                    Some(ident) => ident.to_string(),
                    _ => return,
                };
                self.insert(key, None);
            },
            syn::NestedMeta::Meta(syn::Meta::NameValue(m)) => {
                let key = match m.path.get_ident() {
                    Some(ident) => ident.to_string(),
                    _ => return
                };
                let value = match &m.lit {
                    syn::Lit::Str(lit) => lit.value(),
                    _ => m.lit.to_token_stream().to_string(),
                };

                self.insert(key, Some(value));
            },
            syn::NestedMeta::Lit(m) => {
                self.insert(m.to_token_stream().to_string(), None); },
            _ => (),
        }
    }
}


impl Deref for Attributes {
    type Target = AttributesMap;

    fn deref(&self) -> &Self::Target {
        &self.attrs
    }
}

impl DerefMut for Attributes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.attrs
    }
}

