//! Optional resource and routing macros for MCP Apps Server.
//!
//! Enable the runtime crate's `macros` feature and import its reexports. The derive
//! accepts structs, preserves their generics and bounds, and never inspects fields
//! or requires field traits. It generates resource construction, embedded HTML and
//! optional open metadata and typed UI settings; it does not register handlers or
//! perform runtime I/O.
//!
//! Supply `#[ui_resource(...)]` declarations with required string literal `name`,
//! either `id` or `uri`, and either `html_file` or `html`. Optional keys are
//! `description`, function paths `ui` and `meta`, and runtime crate path `crate`.
//! For example:
//!
//! ```text
//! #[derive(StaticUiResource)]
//! #[ui_resource(id = "inventory/dashboard",
//!               name = "inventory-dashboard", html_file = "dashboard.html",
//!               description = "Inventory dashboard", ui = dashboard_ui,
//!               meta = dashboard_meta)]
//! struct Dashboard;
//! ```
//!
//! `id` must be nonempty and omit `ui://`; the macro adds that prefix. `uri`
//! accepts a complete UI URI. `html` remains a supported file-path spelling.
//! Each pair is exclusive. Logical identity stays independent of the asset path.
//!
//! The generated `include_str!` embeds the UTF-8 file at compilation, relative to
//! the declaring Rust file. The compiler diagnoses missing or non-UTF-8 files.
//! `ui` is a zero-argument function returning the runtime's `ResourceUi`; omission
//! retains the trait default. `meta` is a zero-argument function returning
//! `Result<MetaObject, MetadataError>` from the runtime crate. Its open metadata
//! is composed with `ui` at registration, preserving other extension namespaces
//! and propagating preparation/conflict errors. Use `crate = apps_sdk` if the
//! runtime dependency was renamed to `apps_sdk`. URI literals undergo the same
//! prefix and RFC syntax
//! checks as the runtime constructor, which the generated implementation still
//! calls fallibly. Unknown, duplicate or missing attributes are compilation errors.
//! The corresponding handwritten trait implementation remains available without
//! the macros feature. The compatibility `derive` feature enables the same package.
//!
//! With the runtime's `server` feature, `#[app_router]` on an inherent impl keeps
//! rmcp's `#[tool]` declarations and consumes `#[ui(resource = Type)]` markers.
//! Its fallible `app_router()` method prepares resources and checked tool routes,
//! without starting a server or invoking request handlers. Import this through
//! the runtime reexport for its complete contract and executable examples.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::quote;
use syn::{Data, DeriveInput, LitStr, Path, parse_macro_input};

mod router;

/// Generate checked application registration for an inherent tool impl.
///
/// Use through the runtime crate's `app_router` reexport. The optional `crate`
/// argument selects a renamed runtime dependency. See that reexport's reference
/// for UI markers, feature requirements and the generated preparation method.
#[proc_macro_attribute]
pub fn app_router(attr: TokenStream, input: TokenStream) -> TokenStream {
    router::expand(attr.into(), input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Implement `mcp_apps_server::StaticUiResource` for a struct.
///
/// Required `ui_resource` keys are string literal `name`, either `id` or `uri`,
/// and either `html_file` or the compatible `html` spelling. Both pairs are exclusive.
/// Optional keys are string literal `description`, function paths `ui` and `meta`,
/// and runtime crate path `crate`. See the crate documentation for embedding and validation.
/// Generated code preserves the struct's generic parameters and existing bounds
/// without requiring traits on its fields. It constructs the resource through the
/// fallible runtime constructor and leaves registration to the consuming server.
#[proc_macro_derive(StaticUiResource, attributes(ui_resource))]
pub fn derive_static_ui_resource(input: TokenStream) -> TokenStream {
    expand(&parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[derive(Default)]
struct Attributes {
    uri: Option<LitStr>,
    id: Option<LitStr>,
    name: Option<LitStr>,
    description: Option<LitStr>,
    html: Option<LitStr>,
    html_file: Option<LitStr>,
    ui: Option<Path>,
    meta: Option<Path>,
    crate_path: Option<Path>,
}

fn attributes(input: &DeriveInput) -> syn::Result<Attributes> {
    let mut result = Attributes::default();
    for attr in &input.attrs {
        if !attr.path().is_ident("ui_resource") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            macro_rules! read {
                ($field:ident, $type:ty) => {{
                    if result.$field.is_some() {
                        return Err(meta.error("duplicate ui_resource attribute"));
                    }
                    result.$field = Some(meta.value()?.parse::<$type>()?);
                }};
            }
            if meta.path.is_ident("uri") {
                read!(uri, LitStr);
            } else if meta.path.is_ident("id") {
                read!(id, LitStr);
            } else if meta.path.is_ident("name") {
                read!(name, LitStr);
            } else if meta.path.is_ident("description") {
                read!(description, LitStr);
            } else if meta.path.is_ident("html") {
                read!(html, LitStr);
            } else if meta.path.is_ident("html_file") {
                read!(html_file, LitStr);
            } else if meta.path.is_ident("ui") {
                read!(ui, Path);
            } else if meta.path.is_ident("meta") {
                read!(meta, Path);
            } else if meta.path.is_ident("crate") {
                read!(crate_path, Path);
            } else {
                return Err(meta.error("unknown ui_resource attribute"));
            }
            Ok(())
        })?;
    }
    Ok(result)
}

fn required<T>(value: Option<T>, key: &str, input: &DeriveInput) -> syn::Result<T> {
    value.ok_or_else(|| {
        syn::Error::new_spanned(
            &input.ident,
            format!("missing required ui_resource attribute `{key}`"),
        )
    })
}

fn expand(input: &DeriveInput) -> syn::Result<Tokens> {
    if !matches!(input.data, Data::Struct(_)) {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "StaticUiResource requires a struct",
        ));
    }
    let attributes = attributes(input)?;
    let uri = match (attributes.uri, attributes.id) {
        (Some(uri), None) => uri,
        (None, Some(id)) => {
            let value = id.value();
            if value.is_empty() || value.starts_with("ui://") {
                return Err(syn::Error::new_spanned(
                    id,
                    "resource id must be nonempty and omit the ui:// prefix",
                ));
            }
            LitStr::new(&format!("ui://{value}"), id.span())
        }
        (Some(_), Some(id)) => {
            return Err(syn::Error::new_spanned(id, "choose either id or uri"));
        }
        (None, None) => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "missing required ui_resource attribute `id` or `uri`",
            ));
        }
    };
    let name = required(attributes.name, "name", input)?;
    let html = match (attributes.html, attributes.html_file) {
        (Some(html), None) | (None, Some(html)) => html,
        (Some(_), Some(html)) => {
            return Err(syn::Error::new_spanned(
                html,
                "choose either html_file or html",
            ));
        }
        (None, None) => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "missing required ui_resource attribute `html_file` or `html`",
            ));
        }
    };
    let uri_value = uri.value();
    if !uri_value.starts_with("ui://") {
        return Err(syn::Error::new_spanned(
            &uri,
            "UI resource URI must start with ui://",
        ));
    }
    fluent_uri::Uri::parse(uri_value.as_str()).map_err(|error| {
        syn::Error::new_spanned(&uri, format!("invalid UI resource URI syntax: {error}"))
    })?;
    let crate_path = attributes
        .crate_path
        .unwrap_or_else(|| syn::parse_quote!(::mcp_apps_server));
    let description = attributes
        .description
        .map(|description| quote!(.with_description(#description)));
    let ui = attributes.ui.map(|ui| {
        quote! {
            fn ui() -> #crate_path::ResourceUi {
                #ui()
            }
        }
    });
    let meta = attributes.meta.map(|meta| {
        quote! {
            fn meta() -> ::core::result::Result<#crate_path::MetaObject, #crate_path::MetadataError> {
                #meta()
            }
        }
    });
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #crate_path::StaticUiResource for #ident #ty_generics #where_clause {
            fn resource() -> ::core::result::Result<#crate_path::UiResource, #crate_path::UiUriError> {
                ::core::result::Result::Ok(
                    #crate_path::UiResource::new(#crate_path::UiResourceUri::new(#uri)?, #name)
                        #description
                )
            }

            fn html() -> &'static ::core::primitive::str {
                ::core::include_str!(#html)
            }

            #ui
            #meta
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definition_errors_are_reported_before_generating_code() {
        // These diagnostics protect the documented derive grammar and the same
        // prefix/RFC syntax boundary used by runtime URI construction.
        let cases = [
            (
                quote!(
                    #[ui_resource(id = "", name = "a", html_file = "a.html")]
                    struct View;
                ),
                "resource id must be nonempty",
            ),
            (
                quote!(
                    #[ui_resource(id = "ui://a", name = "a", html_file = "a.html")]
                    struct View;
                ),
                "omit the ui:// prefix",
            ),
            (
                quote!(
                    #[ui_resource(id = "a/%ZZ", name = "a", html_file = "a.html")]
                    struct View;
                ),
                "invalid UI resource URI syntax",
            ),
            (
                quote!(
                    #[ui_resource(id = "a", uri = "ui://a", name = "a", html_file = "a.html")]
                    struct View;
                ),
                "choose either id or uri",
            ),
            (
                quote!(
                    #[ui_resource(id = "a", name = "a", html_file = "a.html", html = "b.html")]
                    struct View;
                ),
                "choose either html_file or html",
            ),
            (
                quote!(
                    #[ui_resource(unknown = "x")]
                    struct View;
                ),
                "unknown ui_resource attribute",
            ),
            (
                quote!(
                    #[ui_resource(uri = "ui://a", uri = "ui://b")]
                    struct View;
                ),
                "duplicate ui_resource attribute",
            ),
            (
                quote!(
                    struct View;
                ),
                "missing required ui_resource attribute `id` or `uri`",
            ),
            (
                quote!(
                    #[ui_resource(uri = "ui://a")]
                    struct View;
                ),
                "missing required ui_resource attribute `name`",
            ),
            (
                quote!(
                    #[ui_resource(uri = "ui://a", name = "a")]
                    struct View;
                ),
                "missing required ui_resource attribute `html_file` or `html`",
            ),
            (
                quote!(
                    #[ui_resource(uri = "https://a", name = "a", html = "a.html")]
                    struct View;
                ),
                "UI resource URI must start with ui://",
            ),
            (
                quote!(
                    #[ui_resource(uri = "ui://a/%ZZ", name = "a", html = "a.html")]
                    struct View;
                ),
                "invalid UI resource URI syntax",
            ),
            (
                quote!(
                    enum View {
                        One,
                    }
                ),
                "StaticUiResource requires a struct",
            ),
        ];
        for (tokens, diagnostic) in cases {
            let input = syn::parse2::<DeriveInput>(tokens).expect("valid derive input fixture");
            let error = expand(&input).expect_err("invalid definition must fail");
            assert!(error.to_string().contains(diagnostic), "{error}");
        }
    }
}
