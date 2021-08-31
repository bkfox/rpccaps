extern crate proc_macro;

use syn;

use super::utils::*;


pub struct Method {
    pub index: u32,
    pub method: syn::ImplItemMethod,

    pub ident: syn::Ident,
    pub ident_cap: syn::Ident,
    pub args: Vec<syn::Pat>,
    pub args_ty: Vec<syn::Type>,
    pub output: Option<syn::Type>,
    pub is_async: bool,
}

impl Method {
    pub fn new(index: u32, method: &mut syn::ImplItemMethod) -> Option<Self> {
        let sig = &method.sig;
        // arguments
        let mut iter = sig.inputs.iter();
        match iter.next() {
            Some(syn::FnArg::Receiver(_)) => (),
            _ => return None,
        }

        let (mut args, mut args_ty) = (Vec::new(), Vec::new());
        for arg in iter {
            match arg {
                syn::FnArg::Typed(arg) => {
                    args.push((*arg.pat).clone());
                    args_ty.push((*arg.ty).clone());
                }
                _ => (),
            }
        }

        // metadata
        // let attrs = Attributes::from_attrs("rpc", &mut method.attrs).to_map();

        let ident = sig.ident.clone();
        Some(Self {
            index, args, args_ty, ident,
            method: method.clone(),
            ident_cap: to_camel_ident(&sig.ident),
            output: match sig.output.clone() {
                syn::ReturnType::Default => None,
                syn::ReturnType::Type(_, ty) => Some(*ty)
            },

            is_async: sig.asyncness.is_some(),
        })
    }
}



