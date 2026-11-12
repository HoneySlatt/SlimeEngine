#![doc = include_str!("../README.md")]

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::{format_ident, quote};
use syn::{Attribute, Data, DeriveInput, Fields, Ident, Type};

/// Derives the declarations of a struct of uwui's styles
/// (`docs/adr/0229-styles-typed-as-declarations.md`).
///
/// For a struct `S` with named fields, it generates `SDeclaration`, an enum with a variant for each
/// field, named after it in `UpperCamelCase`, which holds the field's value; for an array, the
/// place of one of its elements and that element's value, so that a declaration sets one element
/// (`docs/adr/0231-an-elements-own-style-written-in-code.md`); for a field marked
/// `#[declare(nested)]`, the declaration of one field of the field's own struct, which derives
/// `Declare` too: `LayoutStyleDeclaration::Padding(SidesDeclaration::Top(12.0))`. Each *leaf*, a
/// field that is not nested or an element of an array, has a number of its own. A field marked
/// `#[declare(lays_out)]`, nested or not, lays out what it styles; a field marked
/// `#[declare(color)]`, not nested, holds a color that transitions move: a field whose type names
/// `LinearColor`, an option or an array of them included, fails to compile unmarked, and an array
/// marked is refused, its colors not listed. A field whose type names `f32` is marked what its
/// numbers measure, as a style sheet writes them, `#[declare(length)]`, `#[declare(duration)]` or
/// `#[declare(number)]`, once, and fails to compile unmarked; another field is not marked. It
/// generates:
///
/// - `SDeclaration::apply`, which sets the leaf that the declaration names;
/// - `SDeclaration::path`, the leaf's path in a style sheet, `padding.top` or `size[0]`;
/// - `SDeclaration::leaf`, the leaf's number, from 0 to `S::LEAVES`, in the order of the struct;
/// - `SDeclaration::lays_out`, whether the leaf lays out;
/// - `Same` for `SDeclaration`, its value compared by its bits;
/// - `S::LEAVES`, the number of its leaves;
/// - `S::declared`, the declarations of the leaves of a value whose bits differ from another's, which
///   uwui's tests write own styles with, built for them alone;
/// - `S::read_fields`, which reads from a node of KDL the declarations of the fields that its
///   properties and children write, a leaf by its type's `Field`, an array as a child node of its
///   elements, a nested field as a child node of its own fields, each number of its field's
///   measure (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`);
/// - `S::named`, in uwui's tests alone, the declarations of the leaves that a part of `S` read by
///   reflection names, each holding the value that a whole `S` gives it, an array's every element:
///   the reflected reader's, the oracle of `S::read_fields` until reflection leaves uwui (M6d.3c);
/// - `Same` for `S`, field by field;
/// - `Colors` for `S`, the colors of its fields marked `#[declare(color)]` and of its nested
///   fields, in the order of the struct: the one list of colors that transitions move.
///
/// The code it generates names `crate::style::Same`, `crate::style::Colors`,
/// `crate::theme::sheet`, `uwui_kdl`, `slime_color::LinearColor` and `slime_reflect::Value`: it
/// serves uwui's own styles, and its items are `pub(crate)`.
#[proc_macro_derive(Declare, attributes(declare))]
pub fn derive_declare(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derives the reading of an enum of unit variants that a style sheet names by an identifier,
/// `direction=Column`, its variant's name as Rust writes it
/// (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).
///
/// It generates `crate::theme::sheet::Field` for the enum, which reads a variant from its name and
/// refuses any other value, naming every variant; the enum is `Copy`. It serves uwui's own styles
/// and selectors.
#[proc_macro_derive(Identifiers)]
pub fn derive_identifiers(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    identifiers(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn identifiers(input: &DeriveInput) -> syn::Result<Tokens> {
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "`Identifiers` is derived for an enum of unit variants",
        ));
    };
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "`Identifiers` cannot be derived for a generic type",
        ));
    }
    let mut variants = Vec::new();
    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                variant,
                "`Identifiers` is derived for an enum of unit variants",
            ));
        }
        let ident = &variant.ident;
        let name = ident.to_string();
        variants.push(quote! { (#name, Self::#ident) });
    }
    let ty = &input.ident;
    Ok(quote! {
        impl crate::theme::sheet::Field for #ty {
            fn from_entry(
                entry: &::uwui_kdl::Entry<'_, '_>,
                _: crate::theme::sheet::Measure,
                _: &crate::theme::sheet::Reading<'_>,
            ) -> ::std::result::Result<Self, crate::theme::sheet::Refusal> {
                crate::theme::sheet::identifier(entry, &[#(#variants),*])
            }
        }
    })
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
    /// What it holds: a leaf, an array of leaves, or a struct of its own declarations.
    shape: Shape,
    /// Whether it lays out what it styles (`#[declare(lays_out)]`).
    lays_out: bool,
    /// Whether it holds a color that transitions move (`#[declare(color)]`).
    color: bool,
    /// What its numbers measure (`#[declare(length)]`, `duration` or `number`), as a style sheet
    /// writes them; a length for a field that holds no `f32`, whose own type reads its numbers.
    measure: Tokens,
}

/// What a field holds, as its declarations name it.
enum Shape {
    /// One leaf.
    Leaf,
    /// An array, each element a leaf: the elements' type and the array's length.
    Array(Box<Type>, Box<syn::Expr>),
    /// A struct that derives `Declare`: its declarations' type, its type's last segment followed by
    /// `Declaration`.
    Nested(Ident),
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
            let (shape, lays_out, color, measure) = shape(field)?;
            Ok(Field {
                measure,
                variant: format_ident!("{}", camel(&name.to_string())),
                docs: field
                    .attrs
                    .iter()
                    .filter(|attribute| attribute.path().is_ident("doc"))
                    .cloned()
                    .collect(),
                shape,
                lays_out,
                color,
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
        match &field.shape {
            Shape::Nested(inner) => quote! { #(#docs)* #variant(#inner), },
            Shape::Array(element, _) => quote! { #(#docs)* #variant(u8, #element), },
            Shape::Leaf => quote! { #(#docs)* #variant(#ty), },
        }
    });
    let applies = fields.iter().map(|field| {
        let Field { name, variant, .. } = field;
        match field.shape {
            Shape::Nested(_) => quote! { Self::#variant(inner) => inner.apply(&mut target.#name), },
            Shape::Array(..) => quote! {
                Self::#variant(index, value) => {
                    target.#name[usize::from(*index)] = ::std::clone::Clone::clone(value);
                }
            },
            Shape::Leaf => quote! {
                Self::#variant(value) => target.#name = ::std::clone::Clone::clone(value),
            },
        }
    });
    let paths = fields.iter().map(|field| {
        let Field { name, variant, .. } = field;
        let name = name.to_string();
        match field.shape {
            Shape::Nested(_) => quote! {
                Self::#variant(inner) => ::std::format!("{}.{}", #name, inner.path()),
            },
            Shape::Array(..) => quote! {
                Self::#variant(index, _) => ::std::format!("{}[{}]", #name, index),
            },
            Shape::Leaf => quote! { Self::#variant(_) => ::std::string::String::from(#name), },
        }
    });
    let declared = fields.iter().map(|field| {
        let Field { name, variant, .. } = field;
        match field.shape {
            Shape::Nested(_) => quote! {
                self.#name.declared(&base.#name, &mut |inner| out(#declaration::#variant(inner)));
            },
            Shape::Array(..) => quote! {
                for (index, (value, at_base)) in self.#name.iter().zip(&base.#name).enumerate() {
                    if !crate::style::Same::same(value, at_base) {
                        out(#declaration::#variant(
                            u8::try_from(index).expect("an array of a style is short"),
                            ::std::clone::Clone::clone(value),
                        ));
                    }
                }
            },
            Shape::Leaf => quote! {
                if !crate::style::Same::same(&self.#name, &base.#name) {
                    out(#declaration::#variant(::std::clone::Clone::clone(&self.#name)));
                }
            },
        }
    });
    let named_arms = fields.iter().map(|field| {
        let Field { name, variant, .. } = field;
        let key = name.to_string();
        match field.shape {
            Shape::Nested(_) => quote! {
                #key => self.#name.named(part, &mut |inner| out(#declaration::#variant(inner))),
            },
            // A part sets an array whole: each of its elements.
            Shape::Array(..) => quote! {
                #key => {
                    for (index, value) in self.#name.iter().enumerate() {
                        out(#declaration::#variant(
                            u8::try_from(index).expect("an array of a style is short"),
                            ::std::clone::Clone::clone(value),
                        ));
                    }
                }
            },
            Shape::Leaf => quote! {
                #key => out(#declaration::#variant(::std::clone::Clone::clone(&self.#name))),
            },
        }
    });
    // Each field as a style sheet writes it: its name, and whether it is a child node.
    let read_forms = fields.iter().map(|field| {
        let name = field.name.to_string();
        match field.shape {
            Shape::Leaf => {
                let ty = &field.ty;
                quote! { (#name, <#ty as crate::theme::sheet::Field>::NODE) }
            }
            Shape::Nested(_) | Shape::Array(..) => quote! { (#name, true) },
        }
    });
    let read_arms = fields.iter().enumerate().map(|(index, field)| {
        let Field {
            variant,
            ty,
            measure,
            ..
        } = field;
        match &field.shape {
            Shape::Nested(_) => quote! {
                #index => {
                    if let crate::theme::sheet::Item::Node(child) = item {
                        <#ty>::read_fields(&child, reading, &[], &mut |inner, at| {
                            out(#declaration::#variant(inner), at);
                        });
                    }
                }
            },
            Shape::Array(element, len) => quote! {
                #index => {
                    if let Some(values) =
                        crate::theme::sheet::array::<#element, { #len }>(item, #measure, reading)
                    {
                        for (index, value) in values.into_iter().enumerate() {
                            out(
                                #declaration::#variant(
                                    u8::try_from(index).expect("an array of a style is short"),
                                    value,
                                ),
                                at,
                            );
                        }
                    }
                }
            },
            Shape::Leaf => quote! {
                #index => crate::theme::sheet::leaf::<#ty>(item, #measure, reading, &mut |value| {
                    out(#declaration::#variant(value), at);
                }),
            },
        }
    });
    let field_names = fields.iter().map(|field| &field.name);
    // How many leaves each field holds, and the number of its first, the sum of those before it.
    let counts: Vec<Tokens> = fields
        .iter()
        .map(|field| match &field.shape {
            Shape::Leaf => quote! { 1 },
            Shape::Array(_, len) => quote! { (#len) },
            Shape::Nested(_) => {
                let ty = &field.ty;
                quote! { <#ty>::LEAVES }
            }
        })
        .collect();
    let leaves = fields.iter().enumerate().map(|(place, field)| {
        let variant = &field.variant;
        let before = &counts[..place];
        let first = quote! { (0 #(+ #before)*) };
        match field.shape {
            Shape::Leaf => quote! { Self::#variant(_) => #first, },
            Shape::Array(..) => {
                quote! { Self::#variant(index, _) => #first + usize::from(*index), }
            }
            Shape::Nested(_) => quote! { Self::#variant(inner) => #first + inner.leaf(), },
        }
    });
    let lays_out = fields.iter().map(|field| {
        let variant = &field.variant;
        let whole = field.lays_out;
        match field.shape {
            Shape::Nested(_) if !whole => quote! { Self::#variant(inner) => inner.lays_out(), },
            Shape::Array(..) => quote! { Self::#variant(..) => #whole, },
            _ => quote! { Self::#variant(_) => #whole, },
        }
    });
    let sames = fields.iter().map(|field| {
        let variant = &field.variant;
        match field.shape {
            Shape::Array(..) => quote! {
                (Self::#variant(index, value), Self::#variant(other_index, other)) => {
                    index == other_index && crate::style::Same::same(value, other)
                }
            },
            _ => quote! {
                (Self::#variant(value), Self::#variant(other)) => crate::style::Same::same(value, other),
            },
        }
    });

    // The fields that hold colors: those marked `#[declare(color)]`, and the nested ones, whose own
    // fields say.
    let colored: Vec<&Field> = fields
        .iter()
        .filter(|field| field.color || matches!(field.shape, Shape::Nested(_)))
        .collect();
    let color_counts = colored.iter().map(|field| {
        let ty = &field.ty;
        quote! { <#ty as crate::style::Colors>::COUNT }
    });
    let color_names: Vec<&Ident> = colored.iter().map(|field| &field.name).collect();

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

            /// The path of the leaf that it declares, as a style sheet writes it: `padding.top`,
            /// `size[0]`.
            pub(crate) fn path(&self) -> ::std::string::String {
                match self {
                    #(#paths)*
                }
            }

            /// The number of the leaf that it declares, below the struct's `LEAVES`.
            pub(crate) fn leaf(&self) -> usize {
                match self {
                    #(#leaves)*
                }
            }

            /// Whether the leaf that it declares lays out what it styles.
            pub(crate) fn lays_out(&self) -> bool {
                match self {
                    #(#lays_out)*
                }
            }
        }

        impl crate::style::Same for #declaration {
            fn same(&self, other: &Self) -> bool {
                match (self, other) {
                    #(#sames)*
                    _ => false,
                }
            }
        }

        impl #ty {
            /// The number of its leaves: its fields', an array's elements and a nested struct's
            /// leaves each counted.
            pub(crate) const LEAVES: usize = 0 #(+ #counts)*;

            /// Gives `out` the declarations of the fields whose bits differ from `base`'s, in the
            /// order of the struct.
            #[cfg(test)]
            pub(crate) fn declared(&self, base: &Self, out: &mut dyn FnMut(#declaration)) {
                #(#declared)*
            }

            /// Reads the declarations of the fields that `node` writes, its properties and its
            /// children, in their order, giving `out` each with the byte where it is written; the
            /// children named in `also` are left to the caller. Each error is kept in `reading`, the
            /// fields written rightly read still (`crate::theme::sheet`).
            pub(crate) fn read_fields(
                node: &::uwui_kdl::Node<'_, '_>,
                reading: &mut crate::theme::sheet::Reading<'_>,
                also: &[&str],
                out: &mut dyn FnMut(#declaration, usize),
            ) {
                const FIELDS: &[(&str, bool)] = &[#(#read_forms),*];
                crate::theme::sheet::fields(node, reading, FIELDS, also, &mut |index, item, reading| {
                    let at = item.offset();
                    match index {
                        #(#read_arms)*
                        _ => {}
                    }
                });
            }

            /// Gives `out` the declarations of the fields that `part`, a part of this type as
            /// reflection reads one, names, each holding the value that `self`, the whole value
            /// laid under the part, gives it; in the order of the part: the reflected reader's,
            /// kept as the oracle of the reader of KDL until reflection leaves uwui (M6d.3c).
            #[cfg(test)]
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

        impl crate::style::Colors for #ty {
            const COUNT: usize = 0 #(+ #color_counts)*;

            fn read(&self, out: &mut dyn FnMut(::slime_color::LinearColor)) {
                let _ = &out;
                #(crate::style::Colors::read(&self.#color_names, out);)*
            }

            fn write(&mut self, next: &mut dyn FnMut() -> ::slime_color::LinearColor) {
                let _ = &next;
                #(crate::style::Colors::write(&mut self.#color_names, next);)*
            }
        }
    })
}

/// What `field` holds, whether it is marked `#[declare(lays_out)]` and whether it is marked
/// `#[declare(color)]`: a struct of its own declarations if it is marked `#[declare(nested)]`, an
/// array of leaves if its type is an array, a leaf otherwise.
///
/// # Errors
///
/// A field whose type names `LinearColor` and is not marked a color, so that no color is left out
/// of the list that transitions move; an array marked a color, whose elements the list does not
/// hold; a nested field marked a color.
fn shape(field: &syn::Field) -> syn::Result<(Shape, bool, bool, Tokens)> {
    let (mut nested, mut lays_out, mut color) = (false, false, false);
    let mut measure: Option<(&str, Tokens)> = None;
    for attribute in &field.attrs {
        if !attribute.path().is_ident("declare") {
            continue;
        }
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("nested") {
                nested = true;
                Ok(())
            } else if meta.path.is_ident("lays_out") {
                lays_out = true;
                Ok(())
            } else if meta.path.is_ident("color") {
                color = true;
                Ok(())
            } else if let Some(mark) = ["length", "duration", "number"]
                .into_iter()
                .find(|mark| meta.path.is_ident(mark))
            {
                if let Some((first, _)) = measure {
                    return Err(meta.error(format!("a number is marked once: `{first}` already")));
                }
                let variant = format_ident!("{}", camel(mark));
                measure = Some((mark, quote! { crate::theme::sheet::Measure::#variant }));
                Ok(())
            } else {
                Err(meta.error(
                    "`declare` takes `nested`, `lays_out`, `color`, `length`, `duration` and \
                     `number`",
                ))
            }
        })?;
    }
    // A number unmarked would be read as no kind in particular: every type that holds one by its
    // name is marked what it measures, so that a sheet that names a theme's value of another kind
    // is refused.
    let names_a_number = names(&field.ty, "f32");
    if names_a_number && measure.is_none() {
        return Err(syn::Error::new_spanned(
            &field.ty,
            "a number is marked `#[declare(length)]`, `#[declare(duration)]` or \
             `#[declare(number)]`, as a style sheet writes it",
        ));
    }
    if !names_a_number && measure.is_some() {
        return Err(syn::Error::new_spanned(
            &field.ty,
            "only a field that holds an `f32` is marked what it measures",
        ));
    }
    // A field that holds no `f32` reads its numbers by its own type, a size or a shadow, or holds
    // none: the measure given is not read.
    let measure = measure.map_or_else(
        || quote! { crate::theme::sheet::Measure::Length },
        |(_, measure)| measure,
    );
    // A color forgotten would not move with the others: every type that holds one by its name is
    // marked, an option or an array of colors included.
    if !nested && !color && names(&field.ty, "LinearColor") {
        return Err(syn::Error::new_spanned(
            &field.ty,
            "a color is marked `#[declare(color)]`, so that transitions move it",
        ));
    }
    if let Type::Array(array) = &field.ty {
        if color {
            return Err(syn::Error::new_spanned(
                &field.ty,
                "an array of colors is not listed for transitions: give each color a field of its own",
            ));
        }
        if nested {
            return Err(syn::Error::new_spanned(
                &field.ty,
                "an array's elements are leaves: it is not nested",
            ));
        }
        return Ok((
            Shape::Array(array.elem.clone(), Box::new(array.len.clone())),
            lays_out,
            color,
            measure,
        ));
    }
    if !nested {
        return Ok((Shape::Leaf, lays_out, color, measure));
    }
    if color {
        return Err(syn::Error::new_spanned(
            &field.ty,
            "a nested field gives its colors through its own fields' `#[declare(color)]`",
        ));
    }
    let Type::Path(path) = &field.ty else {
        return Err(syn::Error::new_spanned(
            &field.ty,
            "a nested field is of a struct that derives `Declare`",
        ));
    };
    let last = path.path.segments.last().expect("a path has a segment");
    Ok((
        Shape::Nested(format_ident!("{}Declaration", last.ident)),
        lays_out,
        false,
        measure,
    ))
}

/// Whether `ty` names the type `name` anywhere in it: `LinearColor`, `Option<LinearColor>`,
/// `[LinearColor; 2]`. A struct that holds one within, a shadow's color or a size's pixels, is not
/// seen: its own type reads it.
fn names(ty: &Type, name: &str) -> bool {
    fn holds(tokens: Tokens, name: &str) -> bool {
        tokens.into_iter().any(|token| match token {
            proc_macro2::TokenTree::Ident(ident) => ident == name,
            proc_macro2::TokenTree::Group(group) => holds(group.stream(), name),
            _ => false,
        })
    }
    holds(quote! { #ty }, name)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The message of the error that deriving `Declare` for `input` gives; none if it derives.
    fn refusal(input: &DeriveInput) -> Option<String> {
        expand(input).err().map(|error| error.to_string())
    }

    #[test]
    fn a_color_left_unmarked_is_refused_wherever_it_is_named() {
        let inputs: [DeriveInput; 3] = [
            syn::parse_quote! { struct S { background: LinearColor } },
            syn::parse_quote! { struct S { glow: Option<LinearColor> } },
            syn::parse_quote! { struct S { stops: [LinearColor; 2] } },
        ];
        for input in &inputs {
            let message = refusal(input);
            assert!(
                message
                    .as_deref()
                    .is_some_and(|text| text.contains("#[declare(color)]")),
                "{message:?}"
            );
        }
    }

    #[test]
    fn a_number_is_marked_what_it_measures_once_and_only_a_number_is() {
        let unmarked: [DeriveInput; 3] = [
            syn::parse_quote! { struct S { size: f32 } },
            syn::parse_quote! { struct S { radii: [f32; 4] } },
            syn::parse_quote! { struct S { width: Option<f32> } },
        ];
        for input in &unmarked {
            let message = refusal(input);
            assert!(
                message
                    .as_deref()
                    .is_some_and(|text| text.contains("#[declare(length)]")),
                "{message:?}"
            );
        }
        let marked: DeriveInput = syn::parse_quote! {
            struct S { #[declare(duration)] duration: f32, #[declare(length)] radii: [f32; 4] }
        };
        assert_eq!(refusal(&marked), None);
        let twice: DeriveInput = syn::parse_quote! {
            struct S { #[declare(length, number)] size: f32 }
        };
        assert!(refusal(&twice).is_some_and(|text| text.contains("marked once")));
        let not_a_number: DeriveInput = syn::parse_quote! {
            struct S { #[declare(length)] wrap: bool }
        };
        assert!(refusal(&not_a_number).is_some_and(|text| text.contains("only a field")));
    }

    #[test]
    fn identifiers_are_derived_for_unit_variants_alone() {
        let units: DeriveInput = syn::parse_quote! { enum E { A, B } };
        assert!(identifiers(&units).is_ok());
        let holding: DeriveInput = syn::parse_quote! { enum E { A, B(f32) } };
        assert!(identifiers(&holding).is_err());
        let a_struct: DeriveInput = syn::parse_quote! { struct S { a: f32 } };
        assert!(identifiers(&a_struct).is_err());
    }

    #[test]
    fn a_color_marked_derives_and_an_array_marked_is_refused() {
        let marked: DeriveInput = syn::parse_quote! {
            struct S { #[declare(color)] background: LinearColor }
        };
        assert_eq!(refusal(&marked), None);
        let array: DeriveInput = syn::parse_quote! {
            struct S { #[declare(color)] stops: [LinearColor; 2] }
        };
        assert!(refusal(&array).is_some_and(|text| text.contains("field of its own")));
    }
}
