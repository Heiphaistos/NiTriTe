//! `#[tauri::command]` de la cale : laisse la fonction intacte et enregistre
//! a cote un adaptateur `fn(Invoke) -> CommandFuture` qui
//! 1. injecte `Window` / `AppHandle` / `State<T>` comme le ferait Tauri,
//! 2. lit les autres arguments du JSON par leur nom camelCase (`app_id` ->
//!    `appId`, convention Tauri par defaut),
//! 3. serialise le retour (`Result<T, E>` -> Ok/Err, sinon valeur).

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, FnArg, GenericArgument, ItemFn, Pat, PathArguments, ReturnType, Type};

fn camel_case(snake: &str) -> String {
    let mut out = String::with_capacity(snake.len());
    let mut upper = false;
    for (i, c) in snake.trim_start_matches('_').chars().enumerate() {
        if c == '_' {
            upper = i > 0;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn last_segment(ty: &Type) -> Option<&syn::PathSegment> {
    match ty {
        Type::Path(p) => p.path.segments.last(),
        Type::Reference(r) => last_segment(&r.elem),
        _ => None,
    }
}

/// `State<'_, T>` -> `T`.
fn state_inner(seg: &syn::PathSegment) -> Option<&Type> {
    if let PathArguments::AngleBracketed(a) = &seg.arguments {
        a.args.iter().find_map(|g| match g {
            GenericArgument::Type(t) => Some(t),
            _ => None,
        })
    } else {
        None
    }
}

#[proc_macro_attribute]
pub fn command(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let name = &func.sig.ident;
    let name_str = name.to_string();
    let wrapper = format_ident!("__nitrite_cmd_{}", name);

    let mut prelude = Vec::new();
    let mut call_args = Vec::new();
    for (i, input) in func.sig.inputs.iter().enumerate() {
        let FnArg::Typed(pt) = input else {
            return syn::Error::new_spanned(input, "commande avec `self` non supportee").to_compile_error().into();
        };
        let ty = &*pt.ty;
        let seg = last_segment(ty).map(|s| s.ident.to_string()).unwrap_or_default();
        let var = format_ident!("__a{}", i);
        match seg.as_str() {
            "Window" | "WebviewWindow" | "Webview" => {
                prelude.push(quote! { let #var = ::tauri::__private::window(); });
                call_args.push(quote! { #var });
            }
            "AppHandle" => {
                prelude.push(quote! { let #var = ::tauri::__private::app_handle(); });
                call_args.push(quote! { #var });
            }
            "State" => {
                let inner = last_segment(ty).and_then(state_inner);
                let Some(inner) = inner else {
                    return syn::Error::new_spanned(ty, "State<T> sans type").to_compile_error().into();
                };
                let arc = format_ident!("__s{}", i);
                prelude.push(quote! { let #arc = ::tauri::__private::managed::<#inner>(); });
                call_args.push(quote! { ::tauri::__private::state_ref(&*#arc) });
            }
            _ => {
                let key = match &*pt.pat {
                    Pat::Ident(pi) => camel_case(&pi.ident.to_string()),
                    other => {
                        return syn::Error::new_spanned(other, "argument de commande non nomme").to_compile_error().into();
                    }
                };
                prelude.push(quote! {
                    let #var: #ty = match ::tauri::__private::arg(&__invoke.args, #name_str, #key) {
                        Ok(v) => v,
                        Err(e) => return Err(e),
                    };
                });
                call_args.push(quote! { #var });
            }
        }
    }

    let call = if func.sig.asyncness.is_some() {
        quote! { #name(#(#call_args),*).await }
    } else {
        quote! { #name(#(#call_args),*) }
    };

    let returns_result = match &func.sig.output {
        ReturnType::Type(_, ty) => last_segment(ty).map(|s| s.ident == "Result").unwrap_or(false),
        ReturnType::Default => false,
    };
    let convert = if returns_result {
        quote! {
            match #call {
                Ok(v) => ::tauri::__private::ok(v),
                Err(e) => ::tauri::__private::err(e),
            }
        }
    } else {
        quote! { ::tauri::__private::ok(#call) }
    };

    quote! {
        #func

        #[doc(hidden)]
        #[allow(non_snake_case, unused_variables, clippy::all)]
        fn #wrapper(__invoke: ::tauri::Invoke) -> ::tauri::CommandFuture {
            ::std::boxed::Box::pin(async move {
                #(#prelude)*
                #convert
            })
        }

        ::tauri::__private::inventory::submit! {
            ::tauri::CommandDef { name: #name_str, handler: #wrapper }
        }
    }
    .into()
}

/// `#[cfg_attr(mobile, tauri::mobile_entry_point)]` : sans objet ici.
#[proc_macro_attribute]
pub fn mobile_entry_point(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
