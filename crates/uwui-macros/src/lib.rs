#![doc = include_str!("../README.md")]

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::{format_ident, quote};
use syn::{Attribute, Data, DeriveInput, Fields, Ident, Type};

/// Derives the declarations of a struct of uwui's styles
/// (`docs/adr/0229-styles-typed-as-declarations.md`).
///
/// For a struct `S` with named fields, it generates `SDeclaration`, an enum with a variant for each
/// field, named after it in `UpperCamelCase`, which holds the field's value, or, for a field marked
/// `#[declare(nested)]`, the declaration of one field of the field's own struct, which derives
/// `Declare` too: `LayoutStyleDeclaration::Padding(SidesDeclaration::Top(12.0))`. It generates:
///
/// - `SDeclaration::apply`, which sets the field that the declaration names;
/// - `SDeclaration::path`, the field's path in a style sheet, `padding.top`;
/// - `S::declared`, the declarations of the fields of a value whose bits differ from another's;
/// - `S::named`, the declarations of the fields that a part of `S` read by reflection names, each
///   holding the value that a whole `S` gives it; it goes when uwui reads its documents with
///   `serde` (M6d.3);
/// - `Same` for `S`, field by field.
///
/// The code it generates names `crate::style::Same` and `slime_reflect::Value`: it serves uwui's
/// own styles, and its items are `pub(crate)`.
#[proc_macro_derive(Declare, attributes(declare))]
pub fn derive_declare(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// A field as the generated code names it.
struct Field {
    /// Its name in the struct and in style sheets: `padding`.
    name: Ident,
    /// Its variant in the declaration's enum: `Padding`.
    variant: Ident,
    ty: Type,
    /// Its doc comments, which its variant takes.
    docs: Vec<Attribute>,
    /// Whether it is a struct of its own declarations, whose name its type's last segment gives.
    nested: Option<Ident>,
}

fn expand(input: &DeriveInput) -> syn::Result<Tokens> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "`Declare` cannot be derived for a generic type",
        ));
    }
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "`Declare` is derived for a struct of named fields",
        ));
    };
    let Fields::Named(named) = &data.fields else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "`Declare` is derived for a struct of named fields",
        ));
    };
    let fields = named
        .named
        .iter()
        .map(|field| {
            let name = field.ident.clone().expect("a named field");
            Ok(Field {
                variant: format_ident!("{}", camel(&name.to_string())),
                docs: field
                    .attrs
                    .iter()
                    .filter(|attribute| attribute.path().is_ident("doc"))
                    .cloned()
                    .collect(),
                nested: nested(field)?,
                ty: field.ty.clone(),
                name,
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;
    let ty = &input.ident;
    let declaration = format_ident!("{ty}Declaration");
    let doc = format!(
        "A declaration of one field of [`{ty}`]: what a rule of a style sheet, or an element's own \
         style, sets of it (`docs/adr/0229-styles-typed-as-declarations.md`)."
    );

    let variants = fields.iter().map(|field| {
        let Field {
            variant, ty, docs, ..
        } = field;
        match &field.nested {
            Some(inner) => quote! { #(#docs)* #variant(#inner), },
            None => quote! { #(#docs)* #variant(#ty), },
        }
    });
    let applies = fields.iter().map(|field| {
        let Field { name, variant, .. } = field;
        if field.nested.is_some() {
            quote! { Self::#variant(inner) => inner.apply(&mut target.#name), }
        } else {
            quote! { Self::#variant(value) => target.#name = ::std::clone::Clone::clone(value), }
        }
    });
    let paths = fields.iter().map(|field| {
        let Field { name, variant, .. } = field;
        let name = name.to_string();
        if field.nested.is_some() {
            quote! { Self::#variant(inner) => ::std::format!("{}.{}", #name, inner.path()), }
        } else {
            quote! { Self::#variant(_) => ::std::string::String::from(#name), }
        }
    });
    let declared = fields.iter().map(|field| {
        let Field { name, variant, .. } = field;
        if field.nested.is_some() {
            quote! {
                self.#name.declared(&base.#name, &mut |inner| out(#declaration::#variant(inner)));
            }
        } else {
            quote! {
                if !crate::style::Same::same(&self.#name, &base.#name) {
                    out(#declaration::#variant(::std::clone::Clone::clone(&self.#name)));
                }
            }
        }
    });
    let named_arms = fields.iter().map(|field| {
        let Field { name, variant, .. } = field;
        let key = name.to_string();
        if field.nested.is_some() {
            quote! {
                #key => self.#name.named(part, &mut |inner| out(#declaration::#variant(inner))),
            }
        } else {
            quote! {
                #key => out(#declaration::#variant(::std::clone::Clone::clone(&self.#name))),
            }
        }
    });
    let field_names = fields.iter().map(|field| &field.name);

    Ok(quote! {
        #[doc = #doc]
        #[derive(Clone, Debug)]
        pub(crate) enum #declaration {
            #(#variants)*
        }

        impl #declaration {
            /// Sets in `target` the field that it declares.
            pub(crate) fn apply(&self, target: &mut #ty) {
                match self {
                    #(#applies)*
                }
            }

            /// The path of the field that it declares, as a style sheet writes it: `padding.top`.
            pub(crate) fn path(&self) -> ::std::string::String {
                match self {
                    #(#paths)*
                }
            }
        }

        impl #ty {
            /// Gives `out` the declarations of the fields whose bits differ from `base`'s, in the
            /// order of the struct.
            pub(crate) fn declared(&self, base: &Self, out: &mut dyn FnMut(#declaration)) {
                #(#declared)*
            }

            /// Gives `out` the declarations of the fields that `part`, a part of this type as
            /// reflection reads one, names, each holding the value that `self`, the whole value
            /// laid under the part, gives it; in the order of the part.
            pub(crate) fn named(
                &self,
                part: &::slime_reflect::Value,
                out: &mut dyn FnMut(#declaration),
            ) {
                let ::slime_reflect::Value::Struct(fields) = part else {
                    return;
                };
                for (key, part) in fields {
                    match key.as_str() {
                        #(#named_arms)*
                        // The reader refused any other name before.
                        _ => {}
                    }
                }
            }
        }

        impl crate::style::Same for #ty {
            fn same(&self, other: &Self) -> bool {
                true #(&& crate::style::Same::same(&self.#field_names, &other.#field_names))*
            }
        }
    })
}

/// The declaration's type of `field` if it is marked `#[declare(nested)]`: its type's last
/// segment followed by `Declaration`.
fn nested(field: &syn::Field) -> syn::Result<Option<Ident>> {
    let mut nested = false;
    for attribute in &field.attrs {
        if !attribute.path().is_ident("declare") {
            continue;
        }
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("nested") {
                nested = true;
                Ok(())
            } else {
                Err(meta.error("`declare` takes `nested` alone"))
            }
        })?;
    }
    if !nested {
        return Ok(None);
    }
    let Type::Path(path) = &field.ty else {
        return Err(syn::Error::new_spanned(
            &field.ty,
            "a nested field is of a struct that derives `Declare`",
        ));
    };
    let last = path.path.segments.last().expect("a path has a segment");
    Ok(Some(format_ident!("{}Declaration", last.ident)))
}

/// `snake_case` as `UpperCamelCase`.
fn camel(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut letters = word.chars();
            letters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(letters).collect()
            })
        })
        .collect()
}
