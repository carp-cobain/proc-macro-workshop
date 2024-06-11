use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{
    parse_macro_input, parse_quote, spanned::Spanned, Attribute, Data, DeriveInput, Expr, Field,
    Fields, FieldsNamed, GenericArgument, Generics, Lit, Meta, PathArguments, PredicateType, Token,
    Type, TypeParam, TypePath, WherePredicate,
};

#[proc_macro_derive(CustomDebug, attributes(debug))]
pub fn derive(input: TokenStream) -> TokenStream {
    // Parse input tokens into a syntax tree.
    let ast = parse_macro_input!(input as DeriveInput);
    // Expand user defined struct and hand the output tokens back to the compiler
    expand(&ast.ident, &ast.data, ast.generics, ast.attrs).into()
}

/// Proc macro expansion
fn expand(
    name: &Ident,
    data: &Data,
    generics: Generics,
    attrs: Vec<Attribute>,
) -> proc_macro2::TokenStream {
    if let Data::Struct(ref struct_data) = data {
        if let Fields::Named(ref fields) = struct_data.fields {
            return expand_struct(name, fields, generics, attrs);
        }
    }
    unimplemented!("only structs with named fields are supported")
}

/// Proc macro token expansion of struct fields.
fn expand_struct(
    name: &Ident,
    fields: &FieldsNamed,
    generics: Generics,
    attrs: Vec<Attribute>,
) -> proc_macro2::TokenStream {
    // The fields of the debug_struct function call chain
    let debug_struct_fields = fields.named.iter().map(|f| {
        let value = f.ident.as_ref().unwrap();
        let name = value.to_string();
        if let Some(fmt) = debug_attribute_fmt(f) {
            quote! { field(#name, &format_args!(#fmt, &self.#value)) }
        } else {
            quote! { field(#name, &self.#value) }
        }
    });

    // Process trait bounds, accounting for escape hatch attributes (test 8)
    let generics = if let Some(attr) = attrs.iter().find(|a| a.path().is_ident("debug")) {
        escape_hatch_bounds(&attr, generics)
    } else {
        heuristic_bounds(fields, generics)
    };

    // Tie it all together
    let name_label = name.to_string();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote! {
        impl #impl_generics std::fmt::Debug for #name #ty_generics #where_clause {
            fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                fmt.debug_struct(#name_label)
                    #(.#debug_struct_fields)*
                    .finish()
            }
        }
    }
}

// Determine whether a field has the 'debug' attribute and return the format string if found.
fn debug_attribute_fmt(f: &Field) -> Option<String> {
    let attr = f.attrs.iter().find(|attr| attr.path().is_ident("debug"))?;
    if let Meta::NameValue(nv) = &attr.meta {
        if let Expr::Lit(expr) = &nv.value {
            if let Lit::Str(fmt) = &expr.lit {
                return Some(fmt.value());
            }
        }
    }
    None
}

// Add a where clause predicate for a debug bound attribute.
// TODO: Report illegal attribute formats back to the compiler...
fn escape_hatch_bounds(attr: &Attribute, mut generics: Generics) -> Generics {
    if let Meta::List(ml) = &attr.meta {
        let _ = ml.parse_nested_meta(|meta| {
            if !meta.path.is_ident("bound") {
                return Err(meta.error("expected `bound`"));
            }
            let value = meta.value()?; // this parses the `=`
            let name: syn::LitStr = value.parse()?; // this parses the attribute name
            let bound = name.parse()?;
            generics.make_where_clause().predicates.push(bound);
            Ok(())
        });
    }
    generics
}

// Add a bound `T: std::fmt::Debug` to every type parameter T, excluding phantom data.
fn heuristic_bounds(fields: &FieldsNamed, mut generics: Generics) -> Generics {
    // Search for and track associated types in struct fields
    let assoc_types: Vec<&TypePath> = generics
        .type_params_mut()
        .into_iter()
        .flat_map(|tp| {
            let state = fields.named.iter().fold(State::default(), |state, f| {
                state.merge(&mut check_type(&f.ty, tp))
            });
            if state.type_param_used {
                tp.bounds.push(parse_quote!(std::fmt::Debug))
            }
            state.assoc_types
        })
        .collect();

    // Create where clause for any found associated types
    assoc_types.into_iter().for_each(|tp| {
        let mut pt = PredicateType {
            lifetimes: None,
            bounded_ty: Type::Path(tp.clone()),
            colon_token: Token![:](tp.span()),
            bounds: syn::punctuated::Punctuated::new(),
        };
        pt.bounds.push(parse_quote!(std::fmt::Debug));
        generics
            .make_where_clause()
            .predicates
            .push(WherePredicate::Type(pt));
    });
    generics
}

/// Tracks associated types when expanding trait bounds recursively.
#[derive(Default)]
struct State<'a> {
    pub type_param_used: bool,
    pub assoc_types: Vec<&'a TypePath>,
}
impl State<'_> {
    /// Combine two states
    pub fn merge(mut self, other: &mut Self) -> Self {
        self.type_param_used = self.type_param_used || other.type_param_used;
        self.assoc_types.append(&mut other.assoc_types);
        self
    }
}

// Check whether a type has any associations with a generic type parameter.
// NOTE: Only supports type paths.
fn check_type<'a>(ty: &'a Type, type_param: &TypeParam) -> State<'a> {
    if let Type::Path(type_path) = ty {
        return check_type_path(type_path, type_param);
    }
    State::default()
}

// Check whether a type path has any associations with a generic type parameter.
fn check_type_path<'a>(type_path: &'a TypePath, type_param: &TypeParam) -> State<'a> {
    if type_path.path.is_ident(&type_param.ident) {
        State {
            type_param_used: true,
            assoc_types: vec![],
        }
    } else if type_path.path.segments.len() > 1
        && type_path.path.segments[0].ident == type_param.ident
    {
        State {
            type_param_used: false,
            assoc_types: vec![type_path],
        }
    } else {
        // Handle special case for test 5
        let segments = type_path
            .path
            .segments
            .iter()
            .filter(|s| s.ident != "PhantomData");

        // Need to check types of type path segments' arguments
        segments.fold(State::default(), |state, segment| {
            state.merge(&mut check_arguments(&segment.arguments, type_param))
        })
    }
}

// Check for associated type usage in a path segments' arguments.
fn check_arguments<'a>(arguments: &'a PathArguments, type_param: &TypeParam) -> State<'a> {
    if let PathArguments::AngleBracketed(ab) = arguments {
        return ab.args.iter().fold(State::default(), |state, arg| {
            if let GenericArgument::Type(t) = arg {
                state.merge(&mut check_type(t, type_param))
            } else {
                state
            }
        });
    }
    State::default()
}
