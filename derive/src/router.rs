use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ImplItem, ItemImpl, Meta, Path, Token, Type, parse::Parser, punctuated::Punctuated};

pub(super) fn expand(attr: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let mut crate_path: Option<Path> = None;
    syn::meta::parser(|meta| {
        if !meta.path.is_ident("crate") {
            return Err(meta.error("unknown app_router argument; expected crate"));
        }
        if crate_path.is_some() {
            return Err(meta.error("duplicate app_router crate argument"));
        }
        crate_path = Some(meta.value()?.parse()?);
        Ok(())
    })
    .parse2(attr)?;
    let crate_path = crate_path.unwrap_or_else(|| syn::parse_quote!(::mcp_apps_server));
    let mut item_impl: ItemImpl = syn::parse2(input)?;
    if item_impl.trait_.is_some() {
        return Err(syn::Error::new_spanned(
            &item_impl.self_ty,
            "app_router requires an inherent impl block",
        ));
    }

    let mut resources = Vec::new();
    let mut tools = Vec::new();
    for item in &mut item_impl.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };
        if method.sig.ident == "app_router" {
            return Err(syn::Error::new_spanned(
                &method.sig.ident,
                "app_router generates a method named app_router; use a different helper name",
            ));
        }
        let mut resource: Option<Type> = None;
        let mut has_ui = false;
        let mut tool_count = 0;
        let mut gates = Vec::new();
        for attribute in &method.attrs {
            if let Some(gate) = gate_meta(&attribute.meta)? {
                gates.push(gate);
            }
            if attribute
                .path()
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "tool")
            {
                tool_count += 1;
            }
            if attribute.path().is_ident("ui") {
                if has_ui {
                    return Err(syn::Error::new_spanned(attribute, "duplicate ui marker"));
                }
                has_ui = true;
                attribute.parse_nested_meta(|meta| {
                    if !meta.path.is_ident("resource") {
                        return Err(meta.error("unknown ui argument; expected resource"));
                    }
                    if resource.is_some() {
                        return Err(meta.error("duplicate ui resource argument"));
                    }
                    resource = Some(meta.value()?.parse()?);
                    Ok(())
                })?;
                if resource.is_none() {
                    return Err(syn::Error::new_spanned(
                        attribute,
                        "ui requires resource = ResourceType",
                    ));
                }
            }
        }
        if has_ui && tool_count == 0 {
            return Err(syn::Error::new_spanned(
                &method.sig.ident,
                "ui requires a directly annotated rmcp tool method",
            ));
        }
        if tool_count > 1 {
            return Err(syn::Error::new_spanned(
                &method.sig.ident,
                "a method must have only one tool attribute",
            ));
        }
        if tool_count == 0 {
            continue;
        }
        method
            .attrs
            .retain(|attribute| !attribute.path().is_ident("ui"));
        // Gates must run before rmcp's tool macro emits its metadata factory.
        let attributes = std::mem::take(&mut method.attrs);
        method.attrs = gates
            .iter()
            .map(|gate| syn::parse_quote!(#[#gate]))
            .chain(
                attributes
                    .into_iter()
                    .filter(|attribute| !attribute.path().is_ident("cfg")),
            )
            .collect();
        let handler = &method.sig.ident;
        let factory = format_ident!("{handler}_tool_attr");
        let tool_definition = if let Some(resource) = resource {
            let uri = format_ident!("__mcp_apps_resource_{}", tools.len());
            resources.push(quote! {
                #(#[#gates])*
                let #uri = router.ensure_static::<#resource>()?;
            });
            quote! {
                let mut definition = Self::#factory();
                #crate_path::ToolUi::new(#uri).apply_to(&mut definition)?;
                router.register_tool((definition, Self::#handler))?;
            }
        } else {
            quote! {
                router.register_tool((Self::#factory(), Self::#handler))?;
            }
        };
        tools.push(quote! {
            #(#[#gates])*
            { #tool_definition }
        });
    }
    if tools.is_empty() {
        return Err(syn::Error::new_spanned(
            &item_impl.self_ty,
            "app_router requires at least one directly annotated rmcp tool method",
        ));
    }
    item_impl.items.push(syn::parse2(quote! {
        /// Prepare the declared tools and static UI resources without starting a server.
        ///
        /// Returns registration errors for conflicting resources, duplicate tool
        /// names or invalid/inconsistent metadata. Resource methods and rmcp tool
        /// metadata factories run during this call; request handlers do not.
        pub fn app_router() -> ::core::result::Result<
            #crate_path::AppRouter<Self>, #crate_path::RegistrationError
        > where Self: ::core::marker::Sized + ::core::marker::Send + ::core::marker::Sync + 'static {
            #[allow(unused_mut, reason = "All declared tools may be excluded by cfg")]
            let mut router = #crate_path::AppRouter::new();
            #(#resources)*
            #(#tools)*
            router.validate()?;
            ::core::result::Result::Ok(router)
        }
    })?);
    Ok(quote!(#item_impl))
}

// Only conditional gates belong on generated registration statements. Other
// method attributes (for example deprecated) must not be applied to those lets.
fn gate_meta(meta: &Meta) -> syn::Result<Option<Meta>> {
    if meta.path().is_ident("cfg") {
        return Ok(Some(meta.clone()));
    }
    if !meta.path().is_ident("cfg_attr") {
        return Ok(None);
    }
    let Meta::List(list) = meta else {
        return Err(syn::Error::new_spanned(meta, "expected cfg_attr arguments"));
    };
    let args = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens.clone())?;
    let mut args = args.into_iter();
    let condition = args.next().ok_or_else(|| {
        syn::Error::new_spanned(meta, "cfg_attr requires a condition and attributes")
    })?;
    let mut gates = Vec::new();
    for argument in args {
        if argument.path().is_ident("ui")
            || argument
                .path()
                .segments
                .last()
                .is_some_and(|part| part.ident == "tool")
        {
            return Err(syn::Error::new_spanned(
                argument,
                "put tool and ui attributes directly on the method; use cfg to gate the method",
            ));
        }
        if let Some(gate) = gate_meta(&argument)? {
            gates.push(gate);
        }
    }
    Ok((!gates.is_empty()).then(|| syn::parse_quote!(cfg_attr(#condition, #(#gates),*))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambiguous_and_misplaced_authoring_markers_have_targeted_diagnostics() {
        // These are invalid declarations, independently of generated token layout.
        let cases = [
            (quote!(impl Trait for Server {}), "inherent impl"),
            (quote!(impl Server {}), "at least one directly annotated"),
            (
                quote!(impl Server { #[ui(resource = View)] fn show(&self) {} }),
                "rmcp tool",
            ),
            (
                quote!(impl Server { #[tool] #[ui()] fn show(&self) {} }),
                "requires resource",
            ),
            (
                quote!(impl Server { #[tool] #[ui(resource = View, resource = Other)] fn show(&self) {} }),
                "duplicate ui resource",
            ),
            (
                quote!(impl Server { #[tool] #[ui(other = View)] fn show(&self) {} }),
                "unknown ui argument",
            ),
            (
                quote!(impl Server { #[tool] #[ui(resource = View)] #[ui(resource = Other)] fn show(&self) {} }),
                "duplicate ui marker",
            ),
            (
                quote!(impl Server { #[tool] fn app_router(&self) {} }),
                "generates a method",
            ),
            (
                quote!(impl Server { #[cfg_attr(all(), tool)] fn show(&self) {} }),
                "put tool and ui attributes directly",
            ),
        ];
        for (input, diagnostic) in cases {
            let error = expand(TokenStream::new(), input).expect_err("invalid declaration");
            assert!(error.to_string().contains(diagnostic), "{error}");
        }
    }
}
