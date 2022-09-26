extern crate proc_macro;

use syn;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;


use super::method::Method;
use super::utils::*;




pub struct Service<'a> {
    pub ast: &'a mut syn::ItemImpl,
    pub methods: Vec<Method>,
    pub meta: Attributes,
}

impl<'a> Service<'a> {
    pub fn new(ast: &'a mut syn::ItemImpl) -> Self {
        let methods = ast.items.iter_mut().enumerate()
            .filter_map(|(index, mut item)| match &mut item {
                syn::ImplItem::Method(ref mut method) => Method::new(index as u32, method),
                _ => None
            }).collect::<Vec<_>>();

        assert!(methods.len() <= 64, "a maximum 64 rpc methods is allowed");

        let mut meta = Attributes::from_attrs("service", &mut ast.attrs);
        meta.read_attrs("meta", &mut ast.attrs);

        Self { ast, methods, meta }
    }

    pub fn generate(&self) -> TokenStream {
        let ast = &self.ast;
        let (types, service, client) = (self.types(), self.service(), self.client());

        (quote!{
            #ast

            use super::*;
            use std::collections::BTreeMap;
            use std::marker::PhantomData;
            use futures::prelude::*;
            use futures::future::{Future,FutureExt,ok,err};

            use async_trait::async_trait;
            use serde::{Deserialize,Serialize};

            use rpccaps::data::Capability;
            use rpccaps::rpc::service::{Service as RPCService_};
            use rpccaps::data::{signature as sig};

            #types
            #service
            #client
        }).into()
    }

    fn types(&self) -> TokenStream2 {
        // let ty = &*self.ast.self_ty;
        let (_impl_generics, ty_generics, where_clause) = self.ast.generics.split_for_impl();

        let requests = self.methods.iter().map(|Method { ident_cap, args_ty, .. }| {
            quote! { #ident_cap(#(#args_ty),*) }
        });
        let responses = self.methods.iter().map(|Method { ident_cap, output, .. }| {
            match output {
                Some(output) => quote! { #ident_cap(#output) },
                None => quote! { #ident_cap },
            }
        });
        /*let cap_ops = self.methods.iter().map(|Method { ident_cap, index, args_ty, .. }| {
            let args_ty = args_ty.iter().map(|_| quote!{ _ });
            let ops = 1u64.rotate_left(*index);
            quote!{ Request::#ident_cap(#(#args_ty),*) => Capability::new(#ops, 0u64) }
        });*/

        // we need phantom variant for handling generics cases: R, R<A>, R<A,B>.
        let phantom = quote! { _Phantom(PhantomData<Request #ty_generics>) };

        quote! {
            #[derive(Serialize,Deserialize)]
            pub enum Request #ty_generics #where_clause {
                #(#requests,)*
                #phantom
            }

            #[derive(Clone,Serialize,Deserialize)]
            pub enum Response #ty_generics #where_clause {
                #(#responses,)*
                #phantom
            }
        }
            /*
            impl #impl_generics Into<Capability> for &Request #ty_generics #where_clause {
                /// Get the index of the Request method.
                fn into(self) -> Capability {
                    match self {
                        #(#cap_ops,)*
                        _ => Capability::empty(),
                    }
                }
            }
        }*/
    }

    fn service(&self) -> TokenStream2 {
        let ty = &*self.ast.self_ty;
        let (impl_generics, ty_generics, where_clause) = self.ast.generics.split_for_impl();

        let metas = self.meta.iter().map(|(k,v)| match v {
            None => quote! { (#k, "") },
            Some(v) => quote! { (#k, #v) },
        }).collect::<Vec<_>>();
        let metas_len = metas.len();

        let variants = self.methods.iter().map(|method| self.service_dispatch_variant(method));

        quote! {
            #[async_trait]
            impl #impl_generics RPCService_ for #ty #ty_generics #where_clause {
                type Request = Request<#ty_generics>;
                type Response = Response<#ty_generics>;

                fn metas() -> &'static [(&'static str, &'static str)] {
                    static metas : [(&'static str, &'static str); #metas_len] = [#(#metas),*];
                    &metas
                }

                fn is_alive(&self) -> bool {
                    true
                }

                async fn dispatch(&mut self, request: Self::Request) -> Option<Self::Response> {
                    match request {
                        #(#variants,)*
                        _ => None,
                    }
                }
            }
        }
    }

    fn service_dispatch_variant(&self, method: &Method) -> TokenStream2 {
        let Method { ident_cap, ident, args, is_async, output, .. } = method;
        let invoke = match is_async {
            false => quote! { self.#ident(#(#args),*) },
            true => quote! { self.#ident(#(#args),*).await },
        };
        let invoke = match output {
            None => quote! { { #invoke; None } },
            Some(_) => quote! { Some(Response::#ident_cap(#invoke)) }
        };
        quote! { Request::#ident_cap(#(#args),*) => #invoke }
    }

    fn client(&self) -> TokenStream2 {
        let ty = &*self.ast.self_ty;
        let mut generics = self.ast.generics.clone();
        generics.params.push(syn::parse_str::<syn::GenericParam>(r"SinkError: Unpin+Send").unwrap());
        generics.params.push(syn::parse_str::<syn::GenericParam>(&format!(
            r"Transport: Stream<Item=Response>+Sink<Request,Error=SinkError>+Unpin+Send"
        )).unwrap());

        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
        let methods = self.methods.iter().map(|m| self.client_method(m));

        quote! {
            pub struct Client #impl_generics #where_clause {
                transport: Transport,
            }

            impl #impl_generics Client #ty_generics #where_clause {
                pub fn new(transport: Transport) -> Self {
                    Self { transport }
                }

                #(#methods)*
            }
        }
    }

    fn client_method(&self, method: &Method) -> TokenStream2 {
        let Method { ident, ident_cap, args, args_ty, output, .. } = method;
        match output {
            None => quote! {
                pub async fn #ident(&mut self, #(#args: #args_ty),*) {
                    self.transport.send(Request::#ident_cap(#(#args),*)).await;
                }
            },
            Some(out) => {
                quote! {
                    pub async fn #ident(&mut self, #(#args: #args_ty),*) -> Result<#out,()> {
                        self.transport.send(Request::#ident_cap(#(#args),*)).await;
                        match self.transport.next().await {
                            Some(Response::#ident_cap(out)) => Ok(out),
                            _ => Err(()),
                        }
                    }
                }
            }
        }
    }

}


