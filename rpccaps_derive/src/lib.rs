use proc_macro::TokenStream;


mod service;
mod utils;


/// Generates RPC service and related classes around a server-side `impl` block of RPC methods.
///
/// The code is generated inside the `service` module:
/// - `Client` trait: client implementation to call RPC, mapping service's RPC methods. Only
///     `send_request(&mut self, request: Request)` must be implemented by user.
/// - `Request`, `Response` enums: a variant for each RPC method. They have same generics as
/// Service.
/// - Implementaton of `Service` trait for the struct implementing RPC methods;
///
///
/// # Example
///
#[proc_macro_attribute]
pub fn service(a: TokenStream, i: TokenStream) -> TokenStream {
    service::service(a, i)
}

