extern crate proc_macro;

use syn;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote,ToTokens};

use super::utils::*;


struct Method {
    index: u32,
    ident: syn::Ident,
    ident_cap: syn::Ident,
    args: Vec<syn::Pat>,
    args_ty: Vec<syn::Type>,
    output: Option<syn::Type>,

    is_async: bool,
}

impl Method {
    fn new(index: u32, method: &mut syn::ImplItemMethod) -> Option<Self> {
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
        Some(Self { index, args, args_ty, ident,
            ident_cap: to_camel_ident(&sig.ident),
            output: match sig.output.clone() {
                syn::ReturnType::Default => None,
                syn::ReturnType::Type(_, ty) => Some(*ty)
            },

            is_async: sig.asyncness.is_some(),
        })
    }

    fn get_dispatch_variant(&self) -> TokenStream2 {
        let call = self.get_dispatch_call();
        let Self { ident_cap, args, .. } = self;
        quote! { Request::#ident_cap(#(#args),*) => { #call } }
    }

    fn get_dispatch_call(&self) -> TokenStream2 {
        let Self { ident_cap, ident, args, is_async, .. } = self;
        let invoke = match is_async {
            false => quote! { self.#ident(#(#args),*) },
            true => quote! { self.#ident(#(#args),*).await },
        };

        match self.output {
            None => quote! { #invoke; None },
            Some(_) => quote! { Some(Response::#ident_cap(#invoke)) }
        }
    }

    fn get_client_method(&self) -> TokenStream2 {
        let Self { ident, ident_cap, args, args_ty, .. } = self;
        match &self.output {
            None => quote! {
                pub async fn #ident(&mut self, #(#args: #args_ty),*) {
                    self.transport.send(Message::Request(Request::#ident_cap(#(#args),*))).await;
                }
            },
            Some(out) => {
                quote! {
                    pub async fn #ident(&mut self, #(#args: #args_ty),*) -> Result<#out,()> {
                        self.transport.send(Message::Request(Request::#ident_cap(#(#args),*))).await;
                        match self.transport.next().await {
                            Some(Message::Response(Response::#ident_cap(out))) => Ok(out),
                            _ => Err(()),
                        }
                    }
                }
            }
        }
    }
}


struct Service<'a> {
    ast: &'a mut syn::ItemImpl,
    methods: Vec<Method>,
    meta: Attributes,
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
        Self { ast, methods, meta }
    }

    pub fn generate(&self) -> TokenStream {
        let ast = &self.ast;
        let (types, server, client) = (self.types(), self.server(), self.client());

        (quote!{
            #ast

            pub mod service {
                use super::*;
                use std::marker::PhantomData;
                use futures::prelude::*;
                use futures::future::{Future,FutureExt,ok,err};

                use async_trait::async_trait;
                use serde::{Deserialize,Serialize};

                use rpccaps::data::Capability;
                use rpccaps::rpc::service::{Service,ServiceMessage};
                use rpccaps::data::{signature as sig};

                #types
                #server
                #client
            }
        }).into()
    }

    fn types(&self) -> TokenStream2 {
        let ty = &*self.ast.self_ty;
        let (impl_generics, ty_generics, where_clause) = self.ast.generics.split_for_impl();

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

            pub type Message = ServiceMessage<#ty>;
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

    fn server(&self) -> TokenStream2 {
        let ty = &*self.ast.self_ty;
        let (impl_generics, ty_generics, where_clause) = self.ast.generics.split_for_impl();

        let variants = self.methods.iter().map(|method| method.get_dispatch_variant());
        quote! {
            #[async_trait]
            impl #impl_generics Service for #ty #ty_generics #where_clause {
                type Request = Request<#ty_generics>;
                type Response = Response<#ty_generics>;

                fn is_alive(&self) -> bool {
                    true
                }

                async fn dispatch(&mut self, request: Self::Request) -> Option<Self::Response> {
                    match request {
                        #(#variants),*
                        _ => None,
                    }
                }
            }
        }
    }

    fn client(&self) -> TokenStream2 {
        let mut generics = self.ast.generics.clone();
        generics.params.push(syn::parse_str::<syn::GenericParam>(r"SinkError: Unpin+Send").unwrap());
        generics.params.push(syn::parse_str::<syn::GenericParam>(&format!(
            r"Transport: Stream<Item=Message>+Sink<Message,Error=SinkError>+Unpin+Send"
        )).unwrap());

        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
        let methods = self.methods.iter().map(|m| m.get_client_method());

        quote! {
            pub struct Client #impl_generics #where_clause {
                transport: Transport,
            }

            impl #impl_generics Client #ty_generics #where_clause {
                pub fn new(transport: Transport) -> Self {
                    Self { transport: transport }
                }

                #(#methods)*
            }
        }
    }
}


/// Macro generating RPC service traits and types, for the decorated
/// struct impl block.
pub fn service(_attrs: TokenStream, input: TokenStream) -> TokenStream {
    let mut ast = syn::parse::<syn::ItemImpl>(input).unwrap();
    let service = Service::new(&mut ast);
    service.generate()
}

