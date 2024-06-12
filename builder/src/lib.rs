use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Field, Fields, FieldsNamed, GenericArgument,
    LitStr, Meta, PathArguments, Type,
};

#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive(input: TokenStream) -> TokenStream {
    // Parse input tokens into a syntax tree.
    let ast = parse_macro_input!(input as DeriveInput);
    // Expand user defined struct and hand the output tokens back to the compiler.
    expand(&ast.ident, &ast.data).into()
}

/// Proc macro expansion
fn expand(name: &Ident, data: &Data) -> proc_macro2::TokenStream {
    if let Data::Struct(ref data) = data {
        if let Fields::Named(ref fields) = data.fields {
            return expand_struct(name, fields);
        }
    }
    unimplemented!("only structs with named fields are supported")
}

/// Proc macro token expansion of struct fields.
fn expand_struct(name: &Ident, fields: &FieldsNamed) -> proc_macro2::TokenStream {
    // Name of the builder struct.
    let builder_name = Ident::new(&format!("{}Builder", name), name.span());

    // Field generation tokens
    let mut builder_fields = Vec::with_capacity(fields.named.len());
    let mut builder_ctor_fields = Vec::with_capacity(fields.named.len());
    let mut builder_methods = Vec::with_capacity(fields.named.len());
    let mut build_fields = Vec::with_capacity(fields.named.len());

    // Loop over fields of input struct
    for f in &fields.named {
        let name = f.ident.as_ref().unwrap();
        let ty = &f.ty;

        let builder_attr = builder_attr(f);
        let opt_inner_ty = inner_type("Option", ty);

        // Builder struct field
        let token = if opt_inner_ty.is_some() || builder_attr.is_some() {
            quote! { #name: #ty, }
        } else {
            quote! { #name: std::option::Option<#ty>, }
        };
        builder_fields.push(token);

        // Builder constructor body field
        let token = if builder_attr.is_some() && inner_type("Vec", &f.ty).is_some() {
            quote! { #name: std::vec::Vec::new(), }
        } else {
            quote! { #name: std::option::Option::None, }
        };
        builder_ctor_fields.push(token);

        // Builder setter or extension method:
        // Try to create an extension method if an attribute is found on the field.
        // If no attribute was found, create a setter.
        let token = builder_attr
            .and_then(|attr| builder_ext_method(name, ty, attr))
            .unwrap_or_else(|| {
                let arg_type = opt_inner_ty.unwrap_or(ty);
                quote! {
                    pub fn #name(&mut self, #name: #arg_type) -> &mut Self {
                        self.#name = std::option::Option::Some(#name);
                        self
                    }
                }
            });
        builder_methods.push(token);

        // Build function field
        let token = if opt_inner_ty.is_some() || builder_attr.is_some() {
            quote! { #name: self.#name.clone(), }
        } else {
            quote! { #name: self.#name.clone().ok_or(concat!(stringify!(#name), " not set"))?, }
        };
        build_fields.push(token);
    }

    // Rustdoc for builder struct
    let doc = format!("Implements the builder pattern for `{}`", name);

    // Tie it all together
    quote! {
        #[doc = #doc]
        pub struct #builder_name {
            #(#builder_fields)*
        }
        impl #builder_name {
            #(#builder_methods)*
            pub fn build(&mut self) -> std::result::Result<#name, std::boxed::Box<dyn std::error::Error>> {
                std::result::Result::Ok(#name {
                    #(#build_fields)*
                })
            }
        }
        impl #name {
            fn builder() -> #builder_name {
                #builder_name {
                    #(#builder_ctor_fields)*
                }
            }
        }
    }
}

// Determine the the inner type of a wrapper type (ie Option). If not a wrapper type, return none.
fn inner_type<'a>(name: &str, ty: &'a Type) -> Option<&'a Type> {
    if let Type::Path(ref tp) = ty {
        if tp.path.segments.len() != 1 || tp.path.segments[0].ident != name {
            return None;
        }
        if let PathArguments::AngleBracketed(ref inner_ty) = tp.path.segments[0].arguments {
            if let Some(GenericArgument::Type(ref t)) = inner_ty.args.first() {
                return Some(t);
            }
        }
    }
    None
}

// Determine whether a field has the 'builder' attribute and return it if found.
fn builder_attr(f: &Field) -> Option<&Attribute> {
    f.attrs.iter().find(|attr| {
        if let Meta::List(ml) = &attr.meta {
            return ml.path.segments.len() == 1 && ml.path.segments[0].ident == "builder";
        }
        false
    })
}

// Determine whether a builder extension method should be generated.
fn builder_ext_method(
    name: &Ident,
    ty: &Type,
    attr: &Attribute,
) -> Option<proc_macro2::TokenStream> {
    let mut tokens = Some(
        syn::Error::new_spanned(attr, "expected `builder(each = \"...\")`").to_compile_error(),
    );
    if let Meta::List(ml) = &attr.meta {
        let _ = ml.parse_nested_meta(|meta| {
            if meta.path.is_ident("each") {
                let lits: LitStr = meta.value()?.parse()?;
                let arg = Ident::new(lits.value().as_ref(), proc_macro2::Span::call_site());
                tokens = inner_type("Vec", ty)
                    .map(|inner_ty| Some(ext_method_tokens(name, &arg, inner_ty)))
                    .unwrap_or_default(); // Kicks back none so setter is generated
            }
            Ok(())
        });
    }
    tokens
}

// Generate ext method tokens
fn ext_method_tokens(name: &Ident, arg: &Ident, ty: &Type) -> proc_macro2::TokenStream {
    quote! {
        pub fn #arg(&mut self, #arg: #ty) -> &mut Self {
            self.#name.push(#arg);
            self
        }
    }
}
