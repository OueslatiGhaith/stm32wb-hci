//! Procedural entry points for the declarative STM32WB protocol catalog.

use proc_macro::TokenStream;
use quote::quote;
use stm32wb_hci_schema::VendorCommand;

/// Parse a vendor command through the shared schema, then delegate generation
/// to the established `macro_rules! vendor_cmd` implementation.
///
/// This deliberately separates the parser migration from the code-generation
/// migration. The representative command therefore gets the new parser and
/// diagnostics while retaining byte-for-byte identical generated Rust. Once
/// the catalog is migrated, generation can move here without changing the
/// declaration language or the compliance parser again.
#[proc_macro]
pub fn vendor_cmd(input: TokenStream) -> TokenStream {
    let original = proc_macro2::TokenStream::from(input);
    match syn::parse2::<VendorCommand>(original.clone()) {
        Ok(_) => quote! {
            vendor_cmd! { #original }
        }
        .into(),
        Err(error) => error.into_compile_error().into(),
    }
}
