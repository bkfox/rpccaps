use std::collections::BTreeMap;
use std::iter::FromIterator;

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
pub fn drain_attrs(attrs: &mut Vec<syn::Attribute>, mut func: impl FnMut(&syn::Attribute) -> bool)
{
    let mut i = 0;
    while i != attrs.len() {
        if func(&attrs[i]) {
            attrs.remove(i);
        }
        else { i += 1 }
    }
}


/// Read syn::Attribute list into key-values.
///
/// Attribute accepted format:
/// - #[prefix(key=value,key=value)
/// - #[prefix("key",...)]
/// - #[prefix(key)]
///
pub struct Attributes {
    pub items: Vec<(String, Option<String>)>,
}

impl Attributes {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn from_attrs(prefix: &str, attrs: &mut Vec<syn::Attribute>) -> Self {
        let mut this = Self::new();
        this.read_attrs(prefix, attrs);
        this
    }

    pub fn read_attrs(&mut self, prefix: &str, attrs: &mut Vec<syn::Attribute>) {
        drain_attrs(attrs, |attr| {
            if !attr.path.is_ident(prefix) {
                return false;
            }

            match attr.parse_meta() {
                Ok(syn::Meta::List(meta)) => {
                    for nested in meta.nested.iter() {
                        self.push_nested(nested)
                    }
                    true
                },
                _ => false,
            }
        })
    }

    fn push_nested(&mut self, meta: &syn::NestedMeta) {
        match meta {
            syn::NestedMeta::Meta(syn::Meta::Path(path)) => {
                let key = match path.get_ident() {
                    Some(ident) => ident.to_string(),
                    _ => return,
                };
                self.push(key, None);
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

                self.push(key, Some(value));
            },
            syn::NestedMeta::Lit(m) => self.push(m.to_token_stream().to_string(), None),
            _ => (),
        }
    }

    /// Add metadata from Expr.
    pub fn push(&mut self, key: String, value: Option<String>) {
        self.items.push((key, value));
    }

    /// Return a map of key values from items
    pub fn to_map(self) -> BTreeMap<String, Option<String>> {
        BTreeMap::from_iter(self.items.into_iter())
    }
}


