use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::parse_macro_input;
use syn::visit_mut::VisitMut;

#[proc_macro_attribute]
pub fn sorted(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut out = input.clone();
    let item = parse_macro_input!(input as syn::Item);
    if let Some(variants) = enum_variants(&item) {
        if let Err(err) = check_sorted(variants) {
            out.extend(TokenStream::from(err.to_compile_error()));
        }
    } else {
        let err = syn::Error::new(Span::call_site(), "expected enum or match expression");
        out.extend(TokenStream::from(err.to_compile_error()));
    }
    out
}

// Determine whether the parsed item is an enum, and if so, return its variant identifiers.
fn enum_variants(item: &syn::Item) -> Option<Vec<syn::Ident>> {
    if let syn::Item::Enum(item_enum) = item {
        return Some(item_enum.variants.iter().map(|v| v.ident.clone()).collect());
    }
    None
}

// Returns an error if the enum variant identifiers are out of order.
fn check_sorted(variants: Vec<syn::Ident>) -> Result<(), syn::Error> {
    let mut checked = Vec::new();
    for variant in variants {
        let name = variant.to_string();
        if let Some(prev_name) = checked.last() {
            if &name < prev_name {
                // Finds the index where name should be inserted
                let idx = checked.binary_search(&name).unwrap_err();
                let errm = format!("{} should sort before {}", name, checked[idx]);
                return Err(syn::Error::new(variant.span(), errm));
            }
        }
        checked.push(name);
    }
    Ok(())
}

#[proc_macro_attribute]
pub fn check(_args: TokenStream, input: TokenStream) -> TokenStream {
    // Parse input
    let mut item_fn = parse_macro_input!(input as syn::ItemFn);

    // Check for unsorted match expressions
    let mut check = MatchSortedCheck::default();
    check.visit_item_fn_mut(&mut item_fn);

    // Return modified token stream and errors back to the compiler
    let mut out = quote! {#item_fn};
    if !check.errors.is_empty() {
        // Test 7 requires only one error, but really, all should be returned.
        out.extend(check.errors.first().unwrap().clone().into_compile_error());
    }
    out.into()
}

#[derive(Debug, Default)]
struct MatchSortedCheck {
    errors: Vec<syn::Error>,
}

impl VisitMut for MatchSortedCheck {
    fn visit_expr_match_mut(&mut self, expr: &mut syn::ExprMatch) {
        // Check for attribute
        if !expr.attrs.iter().any(|a| a.path().is_ident("sorted")) {
            return;
        }

        // Remove the `sorted` attribute from the match expression
        expr.attrs.retain(|a| !a.path().is_ident("sorted"));

        // Check match arms are sorted
        let mut checked = Vec::new();
        let mut found_wildcard = false;
        for arm in expr.arms.clone() {
            // If a previous arm was a wildcard and we got another arm, report an error.
            if found_wildcard {
                let err = syn::Error::new_spanned(&arm, "wildcard must be last arm");
                self.errors.push(err);
            }
            // Compare arm name to previously checked names.
            if let Some(path) = arm_path(&arm) {
                let name = path_name(&path);
                if let Some(prev_name) = checked.last() {
                    if &name < prev_name {
                        let idx = checked.binary_search(&name).unwrap_err();
                        let errm = format!("{} should sort before {}", name, checked[idx]);
                        self.errors.push(syn::Error::new_spanned(path, errm));
                    }
                }
                checked.push(name);
            } else if let syn::Pat::Wild(_) = arm.pat {
                found_wildcard = true;
            } else {
                let error = syn::Error::new_spanned(&arm.pat, "unsupported by #[sorted]");
                self.errors.push(error);
            }
        }
    }
}

// Determine the path for a match arm pattern.
fn arm_path(arm: &syn::Arm) -> Option<syn::Path> {
    match arm.pat {
        syn::Pat::Ident(syn::PatIdent { ident: ref id, .. }) => Some(id.clone().into()),
        syn::Pat::Path(ref p) => Some(p.path.clone()),
        syn::Pat::Struct(ref s) => Some(s.path.clone()),
        syn::Pat::TupleStruct(ref s) => Some(s.path.clone()),
        _ => None,
    }
}

// Dertermine a 'name' for a path
fn path_name(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|s| format!("{}", s.ident))
        .collect::<Vec<_>>()
        .join("::")
}
