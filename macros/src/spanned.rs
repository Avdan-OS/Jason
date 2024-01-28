//!
//! Utilities for #[derive(Spanned)]
//!

use proc_macro::{Diagnostic, Level};
use proc_macro2::Span;
use quote::quote;
use syn::{punctuated::Punctuated, spanned::Spanned};

use crate::type_traversal::{
    field_access, index, is_named_type, self_keyword, variant_path, Generic, ToMember,
};

mod paths {
    use proc_macro2::Span;
    use syn::punctuated::Punctuated;

    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// crate::common::Spanned::span
    /// ```
    ///
    pub fn span() -> syn::Expr {
        syn::Expr::Path(syn::ExprPath {
            attrs: Default::default(),
            qself: Default::default(),
            path: syn::Path {
                leading_colon: Default::default(),
                segments: Punctuated::from_iter(
                    ["crate", "common", "Spanned", "span"]
                        .into_iter()
                        .map(|s| syn::PathSegment {
                            ident: syn::Ident::new(s, Span::call_site()),
                            arguments: syn::PathArguments::None,
                        }),
                ),
            },
        })
    }

    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// crate::common::SpanIter::combine
    /// ```
    ///
    pub fn combine() -> syn::Expr {
        syn::Expr::Path(syn::ExprPath {
            attrs: Default::default(),
            qself: Default::default(),
            path: syn::Path {
                leading_colon: Default::default(),
                segments: Punctuated::from_iter(
                    ["crate", "common", "SpanIter", "combine"]
                        .into_iter()
                        .map(|s| syn::PathSegment {
                            ident: syn::Ident::new(s, Span::call_site()),
                            arguments: syn::PathArguments::None,
                        }),
                ),
            },
        })
    }
}

///
/// Equivalent to:
///
/// ```
/// crate::utils::Spanned::span(& $expr)
/// ```
///
fn span_of(expr: syn::Expr) -> syn::Expr {
    let reference = syn::Expr::Reference(syn::ExprReference {
        attrs: Default::default(),
        and_token: Default::default(),
        mutability: None,
        expr: Box::new(expr),
    });

    syn::Expr::Call(syn::ExprCall {
        attrs: Default::default(),
        func: Box::new(paths::span()),
        paren_token: Default::default(),
        args: Punctuated::from_iter([reference]),
    })
}

///
/// Equivalent to:
///
/// ```
/// crate::utils::SpanIter::combine([crate::utils::Spanned::span(& $expr), .. ])
/// ```
///
fn spans_of(exprs: impl IntoIterator<Item = syn::Expr>) -> syn::Expr {
    let spans = exprs.into_iter().map(span_of);

    syn::Expr::Call(syn::ExprCall {
        attrs: Default::default(),
        func: Box::new(paths::combine()),
        paren_token: Default::default(),
        args: Punctuated::from_iter([syn::Expr::Array(syn::ExprArray {
            attrs: Default::default(),
            bracket_token: Default::default(),
            elems: Punctuated::from_iter(spans),
        })]),
    })
}

pub fn spanned_for_struct(st: &syn::ItemStruct) -> Option<syn::Expr> {
    let syn::ItemStruct { fields, .. } = st;

    let span_field = fields
        .iter()
        .enumerate()
        .find(|(_, syn::Field { ty, .. })| is_named_type(ty, "Span").is_some());

    // Case 1: this struct represents a terminal token.
    //         Use the included `Span` field.
    if let Some((idx, span_field)) = span_field {
        if matches!(fields, syn::Fields::Unnamed(_)) {
            if fields.len() > 1 {
                Diagnostic::spanned(
                    st.fields.span().unwrap(),
                    Level::Warning,
                    "Non single-field tuple with Span field.",
                )
                .emit();

                Diagnostic::spanned(
                    st.ident.span().unwrap(),
                    Level::Help,
                    "Make these fields named with a `span` field instead.",
                )
                .emit()
            }

            return Some(span_of(field_access(index(idx as u32))));
        }

        if matches!(fields, syn::Fields::Named(_)) {
            // Unwrap ok since we're not a tuple-struct.
            let ident = span_field.ident.clone().unwrap();

            if ident != "span" {
                Diagnostic::spanned(
                    ident.span().unwrap(),
                    Level::Warning,
                    "Named Span field should be called `span`.",
                )
                .emit();

                Diagnostic::spanned(
                    ident.span().unwrap(),
                    Level::Help,
                    "Rename this field to `span`.",
                )
                .emit();
            }

            return Some(span_of(field_access(ident)));
        }
    }

    // Case 2: Product type => combine all span values of our fields, in order.

    match fields {
        syn::Fields::Named(syn::FieldsNamed { named, .. }) => 'a: {
            if named.is_empty() {
                break 'a;
            }

            return Some(spans_of(
                named
                    .into_iter()
                    .cloned()
                    .filter_map(|f| f.ident)
                    .map(field_access),
            ));
        }
        syn::Fields::Unnamed(syn::FieldsUnnamed { unnamed, .. }) => 'a: {
            if unnamed.is_empty() {
                break 'a;
            }

            return Some(spans_of(
                unnamed
                    .into_iter()
                    .cloned()
                    .enumerate()
                    .map(|(i, _)| index(i as u32))
                    .map(field_access),
            ));
        }
        syn::Fields::Unit => (),
    }

    Diagnostic::spanned(
        st.span().unwrap(),
        Level::Error,
        "Cannot derive `Spanned` for unit-like struct.",
    )
    .emit();

    None
}

fn ident_pat(ident: syn::Ident) -> syn::Pat {
    syn::Pat::Ident(syn::PatIdent {
        attrs: Default::default(),
        by_ref: None,
        mutability: None,
        ident,
        subpat: None,
    })
}

fn spanned_variant_arm(var: &syn::Variant) -> syn::Arm {
    let syn::Variant { ident, fields, .. } = var;
    let path = variant_path(ident);

    let (members, f_idents): (Vec<_>, Vec<_>) = fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            f.ident
                .clone()
                .map(|i| (i.clone().to_member(), i))
                .unwrap_or_else(|| {
                    (
                        index(i as u32).to_member(),
                        syn::Ident::new(&format!("f{i}"), Span::call_site()),
                    )
                })
        })
        .unzip();

    let pat = match fields {
        syn::Fields::Named(_) => syn::Pat::Struct(syn::PatStruct {
            attrs: Default::default(),
            qself: Default::default(),
            path,
            brace_token: Default::default(),
            fields: Punctuated::from_iter(members.into_iter().zip(f_idents.iter().cloned()).map(
                |(member, ident)| syn::FieldPat {
                    attrs: Default::default(),
                    member,
                    colon_token: Default::default(),
                    pat: Box::new(ident_pat(ident)),
                },
            )),
            rest: Default::default(),
        }),
        syn::Fields::Unnamed(_) => syn::Pat::TupleStruct(syn::PatTupleStruct {
            attrs: Default::default(),
            qself: Default::default(),
            path,
            paren_token: Default::default(),
            elems: Punctuated::from_iter(f_idents.iter().cloned().map(ident_pat)),
        }),
        syn::Fields::Unit => unreachable!(),
    };

    syn::Arm {
        attrs: Default::default(),
        pat,
        guard: None,
        fat_arrow_token: Default::default(),
        body: Box::new(spans_of(f_idents.into_iter().map(|ident| {
            syn::Expr::Path(syn::ExprPath {
                attrs: Default::default(),
                qself: Default::default(),
                path: ident.into(),
            })
        }))),
        comma: Some(Default::default()),
    }
}

pub fn spanned_for_enum(en: &syn::ItemEnum) -> Option<syn::Expr> {
    let vars = &en.variants;

    if vars.is_empty() {
        Diagnostic::spanned(
            en.span().unwrap(),
            Level::Error,
            "Cannot derive spanned for enum no variants.",
        )
        .emit();

        return None;
    }

    // Check if any variants are unit-like, if so give errors then terminate.
    if vars
        .iter()
        .filter(|syn::Variant { fields, .. }| fields.is_empty())
        .map(|f| {
            Diagnostic::spanned(
                f.span().unwrap(),
                Level::Error,
                "Cannot derive spanned for enum with unit-like variants.",
            )
            .emit()
        })
        .next()
        .is_some()
    {
        return None;
    }

    Some(syn::Expr::Match(syn::ExprMatch {
        attrs: Default::default(),
        match_token: Default::default(),
        expr: Box::new(self_keyword()),
        brace_token: Default::default(),
        arms: vars.iter().map(spanned_variant_arm).collect(),
    }))
}

fn derive_spanned(gen: &impl Generic, span_expr: Option<syn::Expr>) -> proc_macro::TokenStream {
    let ident = gen.ident();
    let generics = gen.generics();
    let generic_letters = gen.generic_letters();

    if let Some(span) = span_expr {
        return quote! {
            impl #generics crate::common::Spanned for #ident #generic_letters {
                fn span(&self) -> crate::common::Span {
                    #span
                }
            }
        }
        .into();
    }

    Default::default()
}

pub fn derive_spanned_for_struct(st: &syn::ItemStruct) -> proc_macro::TokenStream {
    let span_expr = spanned_for_struct(st);
    derive_spanned(st, span_expr)
}

pub fn derive_spanned_for_enum(en: &syn::ItemEnum) -> proc_macro::TokenStream {
    let span_expr = spanned_for_enum(en);
    derive_spanned(en, span_expr)
}