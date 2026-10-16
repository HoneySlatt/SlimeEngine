#![doc = include_str!("../README.md")]

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use quote::{ToTokens, quote};
use syn::ext::IdentExt;
use syn::parse::ParseStream;
use syn::{
    Attribute, Data, DataEnum, DeriveInput, Expr, ExprLit, Fields, Ident, Index, Lit, LitInt,
    LitStr, Meta, Token, Type,
};

/// Derives `slime_reflect::Reflect` for a struct, a tuple struct, a unit struct, or an enum whose
/// variants hold data or not, under the stable name that `#[reflect(name = "...")]` gives.
/// `slime_reflect`'s documentation of the trait says which attributes it takes, and what it
/// refuses.
#[proc_macro_derive(Reflect, attributes(reflect))]
pub fn derive_reflect(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// What a type's `#[reflect(...)]` says.
#[derive(Default)]
struct TypeAttributes {
    name: Option<LitStr>,
    aliases: Vec<LitStr>,
    version: Option<LitInt>,
    default: bool,
    /// The function that accepts or refuses a value once it is read.
    check: Option<syn::Path>,
}

/// What a field's `#[reflect(...)]` says.
#[derive(Default)]
struct FieldAttributes {
    aliases: Vec<LitStr>,
    default: bool,
    skip: bool,
    transient: bool,
    range: Option<RangeAttribute>,
    unit: Option<LitStr>,
}

/// A field's `range(...)`: each bound a number or a constant.
#[derive(Default)]
struct RangeAttribute {
    min: Option<Tokens>,
    min_excluded: bool,
    max: Option<Tokens>,
}

/// A field as the generated code names and describes it.
struct Field<'a> {
    /// How the code reaches it: `self.intensity`, `self.0`.
    member: Tokens,
    /// Its name in values: `intensity`, `0`.
    name: String,
    ty: &'a Type,
    doc: String,
    attributes: FieldAttributes,
}

fn expand(input: &DeriveInput) -> syn::Result<Tokens> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "`Reflect` cannot be derived for a generic type or one with lifetimes: describe it \
             by hand",
        ));
    }
    let attributes = type_attributes(&input.attrs)?;
    let Some(name) = attributes.name.clone() else {
        return Err(syn::Error::new(
            input.ident.span(),
            "`Reflect` needs the type's stable name, `#[reflect(name = \"...\")]`: bare for the \
             engine's types, `<crate>::<Type>` for a game's \
             (docs/adr/0080-reflection-and-serialization.md)",
        ));
    };
    match &input.data {
        Data::Struct(data) => {
            let fields = fields(&data.fields)?;
            expand_struct(input, &attributes, &name, &data.fields, &fields)
        }
        Data::Enum(data) => expand_enum(input, &attributes, &name, data),
        Data::Union(data) => Err(syn::Error::new_spanned(
            data.union_token,
            "`Reflect` cannot be derived for a union: which field holds the value is not known",
        )),
    }
}

/// The documentation of an item, from its doc comments.
fn doc(attributes: &[Attribute]) -> String {
    let lines: Vec<String> = attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("doc"))
        .filter_map(|attribute| match &attribute.meta {
            Meta::NameValue(pair) => match &pair.value {
                Expr::Lit(ExprLit {
                    lit: Lit::Str(text),
                    ..
                }) => Some(text.value()),
                _ => None,
            },
            _ => None,
        })
        .map(|line| line.strip_prefix(' ').unwrap_or(&line).to_owned())
        .collect();
    lines.join("\n").trim().to_owned()
}

fn type_attributes(attributes: &[Attribute]) -> syn::Result<TypeAttributes> {
    let mut parsed = TypeAttributes::default();
    for attribute in attributes.iter().filter(|a| a.path().is_ident("reflect")) {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                parsed.name = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("alias") {
                parsed.aliases.push(meta.value()?.parse()?);
            } else if meta.path.is_ident("version") {
                parsed.version = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("default") {
                parsed.default = true;
            } else if meta.path.is_ident("check") {
                parsed.check = Some(meta.value()?.parse()?);
            } else {
                return Err(meta.error(
                    "a type takes `name`, `alias`, `version`, `default` and `check` in \
                     `#[reflect(...)]`",
                ));
            }
            Ok(())
        })?;
    }
    Ok(parsed)
}

fn field_attributes(attributes: &[Attribute]) -> syn::Result<FieldAttributes> {
    let mut parsed = FieldAttributes::default();
    for attribute in attributes.iter().filter(|a| a.path().is_ident("reflect")) {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("alias") {
                parsed.aliases.push(meta.value()?.parse()?);
            } else if meta.path.is_ident("default") {
                parsed.default = true;
            } else if meta.path.is_ident("skip") {
                parsed.skip = true;
            } else if meta.path.is_ident("transient") {
                parsed.transient = true;
            } else if meta.path.is_ident("unit") {
                parsed.unit = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("range") {
                let mut range = RangeAttribute::default();
                meta.parse_nested_meta(|bound| {
                    let value = bound.value()?;
                    if bound.path.is_ident("min") {
                        range.min = Some(number(value)?);
                    } else if bound.path.is_ident("above") {
                        range.min = Some(number(value)?);
                        range.min_excluded = true;
                    } else if bound.path.is_ident("max") {
                        range.max = Some(number(value)?);
                    } else {
                        return Err(bound.error("a range takes `min`, `above` and `max`"));
                    }
                    Ok(())
                })?;
                parsed.range = Some(range);
            } else {
                return Err(meta.error(
                    "a field takes `alias`, `default`, `skip`, `transient`, `range` and `unit` \
                     in `#[reflect(...)]`",
                ));
            }
            Ok(())
        })?;
        if parsed.skip
            && (parsed.default
                || parsed.transient
                || parsed.range.is_some()
                || parsed.unit.is_some()
                || !parsed.aliases.is_empty())
        {
            return Err(syn::Error::new_spanned(
                attribute,
                "a skipped field is not described: it takes no other attribute",
            ));
        }
    }
    Ok(parsed)
}

/// A bound of a range: a number, negative or not, or a constant's path.
fn number(input: ParseStream<'_>) -> syn::Result<Tokens> {
    if input.peek(Token![-]) || input.peek(Lit) {
        let negative = input.parse::<Option<Token![-]>>()?.is_some();
        let literal: Lit = input.parse()?;
        if !matches!(literal, Lit::Int(_) | Lit::Float(_)) {
            return Err(syn::Error::new_spanned(
                literal,
                "a bound of a range is a number",
            ));
        }
        let sign = negative.then(|| quote!(-));
        return Ok(quote!(#sign #literal));
    }
    let path: syn::Path = input.parse()?;
    Ok(path.into_token_stream())
}

fn fields(fields: &Fields) -> syn::Result<Vec<Field<'_>>> {
    fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let (member, name) = match &field.ident {
                Some(ident) => (ident.to_token_stream(), ident.unraw().to_string()),
                None => (Index::from(index).to_token_stream(), index.to_string()),
            };
            Ok(Field {
                member,
                name,
                ty: &field.ty,
                doc: doc(&field.attrs),
                attributes: field_attributes(&field.attrs)?,
            })
        })
        .collect()
}

/// The `FieldInfo`s of the fields that are described.
fn field_infos(fields: &[Field<'_>]) -> Vec<Tokens> {
    fields
        .iter()
        .filter(|field| !field.attributes.skip)
        .map(|field| {
            let Field { name, ty, doc, .. } = field;
            let FieldAttributes {
                aliases,
                default,
                transient,
                range,
                unit,
                ..
            } = &field.attributes;
            let range = match range {
                Some(RangeAttribute {
                    min,
                    min_excluded,
                    max,
                }) => {
                    let min = min
                        .as_ref()
                        .map_or(quote!(f64::NEG_INFINITY), |min| quote!((#min) as f64));
                    let max = max
                        .as_ref()
                        .map_or(quote!(f64::INFINITY), |max| quote!((#max) as f64));
                    quote!(::std::option::Option::Some(::slime_reflect::Range {
                        min: #min,
                        min_excluded: #min_excluded,
                        max: #max,
                    }))
                }
                None => quote!(::std::option::Option::None),
            };
            let unit = match unit {
                Some(unit) => {
                    quote!(::std::option::Option::Some(::std::string::String::from(#unit)))
                }
                None => quote!(::std::option::Option::None),
            };
            quote! {
                ::slime_reflect::FieldInfo {
                    aliases: ::std::vec![#(::std::string::String::from(#aliases)),*],
                    doc: ::std::string::String::from(#doc),
                    default: #default,
                    transient: #transient,
                    range: #range,
                    unit: #unit,
                    ..::slime_reflect::FieldInfo::new(
                        #name,
                        <#ty as ::slime_reflect::Reflect>::type_name(),
                    )
                }
            }
        })
        .collect()
}

/// The checks that the fields given a range hold numbers, as items of their own.
fn range_checks(fields: &[Field<'_>]) -> Vec<Tokens> {
    fields
        .iter()
        .filter(|field| field.attributes.range.is_some())
        .map(|field| {
            let ty = field.ty;
            quote!(const _: fn() = ::slime_reflect::support::ranged::<#ty>;)
        })
        .collect()
}

/// The registrations of the described fields' types.
fn dependencies(fields: &[Field<'_>]) -> Vec<Tokens> {
    fields
        .iter()
        .filter(|field| !field.attributes.skip)
        .map(|field| {
            let ty = field.ty;
            quote!(registry.register::<#ty>();)
        })
        .collect()
}

/// The pairs of a value's fields, of the described fields reached by `access`.
fn field_values(fields: &[Field<'_>], access: impl Fn(&Field<'_>) -> Tokens) -> Vec<Tokens> {
    fields
        .iter()
        .filter(|field| !field.attributes.skip)
        .map(|field| {
            let name = &field.name;
            let at = access(field);
            quote!((::std::string::String::from(#name), ::slime_reflect::Reflect::to_value(#at)))
        })
        .collect()
}

/// The checks that a value's fields are known: by their names and aliases.
fn known(fields: &[Field<'_>]) -> Tokens {
    let groups = fields.iter().filter(|f| !f.attributes.skip).map(|field| {
        let name = &field.name;
        let aliases = &field.attributes.aliases;
        quote!(&[#name #(, #aliases)*])
    });
    quote!(::slime_reflect::support::known(fields, &[#(#groups),*])?;)
}

/// What reads each field from `fields`, in order: `base` holds the type's default, when the type
/// declares one and the fields belong to a struct.
fn reads(fields: &[Field<'_>], base: bool) -> Vec<Tokens> {
    fields
        .iter()
        .map(|field| {
            if field.attributes.skip {
                return quote!(::std::default::Default::default());
            }
            let Field {
                name, ty, member, ..
            } = field;
            let aliases = &field.attributes.aliases;
            let missing = if field.attributes.default {
                quote!(::std::default::Default::default())
            } else if base {
                quote!(base.#member)
            } else {
                quote!(::slime_reflect::support::missing::<#ty>(#name)?)
            };
            quote! {
                match ::slime_reflect::support::field::<#ty>(fields, #name, &[#(#aliases),*])? {
                    ::std::option::Option::Some(value) => value,
                    ::std::option::Option::None => #missing,
                }
            }
        })
        .collect()
}

/// `Self { a: .., b: .. }`, `Self(.., ..)` or `Self` of `reads`, for `shape`.
fn construct(path: Tokens, shape: &Fields, fields: &[Field<'_>], reads: &[Tokens]) -> Tokens {
    match shape {
        Fields::Named(_) => {
            let members = fields.iter().map(|field| &field.member);
            quote!(#path { #(#members: #reads),* })
        }
        Fields::Unnamed(_) => quote!(#path(#(#reads),*)),
        Fields::Unit => path,
    }
}

/// What the type's attributes give its `TypeInfo`, and its `default_value`.
/// What returns the value read, `read`: once the type's check accepts it, if it declares one.
fn finish(attributes: &TypeAttributes) -> Tokens {
    match &attributes.check {
        Some(check) => quote!(::slime_reflect::support::checked(read, #check)),
        None => quote!(::std::result::Result::Ok(read)),
    }
}

fn type_parts(input: &DeriveInput, attributes: &TypeAttributes) -> (Tokens, Tokens) {
    let aliases = &attributes.aliases;
    let version = attributes
        .version
        .as_ref()
        .map_or(quote!(1), ToTokens::to_token_stream);
    let doc = doc(&input.attrs);
    let default = attributes.default;
    let head = quote! {
        aliases: ::std::vec![#(::std::string::String::from(#aliases)),*],
        version: #version,
        doc: ::std::string::String::from(#doc),
        default: #default,
    };
    let default_value = if default {
        quote! {
            fn default_value() -> ::std::option::Option<Self> {
                ::std::option::Option::Some(::std::default::Default::default())
            }
        }
    } else {
        Tokens::new()
    };
    (head, default_value)
}

fn expand_struct(
    input: &DeriveInput,
    attributes: &TypeAttributes,
    name: &LitStr,
    shape: &Fields,
    fields: &[Field<'_>],
) -> syn::Result<Tokens> {
    let ident = &input.ident;
    let (head, default_value) = type_parts(input, attributes);
    let finish = finish(attributes);
    let infos = field_infos(fields);
    let dependencies = dependencies(fields);
    let values = field_values(fields, |field| {
        let member = &field.member;
        quote!(&self.#member)
    });
    let known = known(fields);
    let base = attributes
        .default
        .then(|| quote!(let base: Self = ::std::default::Default::default();));
    let reads = reads(fields, attributes.default);
    let construct = construct(quote!(Self), shape, fields, &reads);
    let checks = range_checks(fields);
    Ok(quote! {
        impl ::slime_reflect::Reflect for #ident {
            fn type_name() -> ::std::string::String {
                ::std::string::String::from(#name)
            }

            fn type_info() -> ::slime_reflect::TypeInfo {
                ::slime_reflect::TypeInfo {
                    name: ::std::string::String::from(#name),
                    #head
                    kind: ::slime_reflect::Kind::Struct(::std::vec![#(#infos),*]),
                }
            }

            fn register_dependencies(registry: &mut ::slime_reflect::Registry) {
                let _ = &registry;
                #(#dependencies)*
            }

            #default_value

            fn to_value(&self) -> ::slime_reflect::Value {
                ::slime_reflect::Value::Struct(::std::vec![#(#values),*])
            }

            fn from_value(
                value: &::slime_reflect::Value,
            ) -> ::std::result::Result<Self, ::slime_reflect::ReflectError> {
                let fields = ::slime_reflect::support::fields(value, #name)?;
                #known
                #base
                let read = #construct;
                #finish
            }
        }
        #(#checks)*
    })
}

fn expand_enum(
    input: &DeriveInput,
    attributes: &TypeAttributes,
    name: &LitStr,
    data: &DataEnum,
) -> syn::Result<Tokens> {
    let ident = &input.ident;
    let (head, default_value) = type_parts(input, attributes);
    let finish = finish(attributes);
    let mut infos = Vec::new();
    let mut dependencies = Vec::new();
    let mut to_values = Vec::new();
    let mut from_values = Vec::new();
    let mut variant_names = Vec::new();
    let mut checks = Vec::new();
    for variant in &data.variants {
        let variant_ident = &variant.ident;
        let variant_name = variant_ident.unraw().to_string();
        let mut aliases = Vec::new();
        for attribute in variant
            .attrs
            .iter()
            .filter(|a| a.path().is_ident("reflect"))
        {
            attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("alias") {
                    aliases.push(meta.value()?.parse::<LitStr>()?);
                    Ok(())
                } else {
                    Err(meta.error("a variant takes `alias` in `#[reflect(...)]`"))
                }
            })?;
        }
        let fields = fields(&variant.fields)?;
        let doc = doc(&variant.attrs);
        let field_infos = field_infos(&fields);
        infos.push(quote! {
            ::slime_reflect::VariantInfo {
                name: ::std::string::String::from(#variant_name),
                aliases: ::std::vec![#(::std::string::String::from(#aliases)),*],
                doc: ::std::string::String::from(#doc),
                fields: ::std::vec![#(#field_infos),*],
            }
        });
        dependencies.extend(self::dependencies(&fields));
        checks.extend(range_checks(&fields));
        // The pattern that binds the variant's fields, each to `field_<name>`.
        let binding = |field: &Field<'_>| -> Ident { quote::format_ident!("field_{}", field.name) };
        let pattern = match &variant.fields {
            Fields::Named(_) => {
                let members = fields.iter().map(|field| &field.member);
                let bindings = fields.iter().map(binding);
                quote!(Self::#variant_ident { #(#members: #bindings),* })
            }
            Fields::Unnamed(_) => {
                let bindings = fields.iter().map(binding);
                quote!(Self::#variant_ident(#(#bindings),*))
            }
            Fields::Unit => quote!(Self::#variant_ident),
        };
        let values = field_values(&fields, |field| binding(field).to_token_stream());
        let unused: Vec<Ident> = fields
            .iter()
            .filter(|field| field.attributes.skip)
            .map(binding)
            .collect();
        to_values.push(quote! {
            #pattern => {
                #(let _ = #unused;)*
                ::slime_reflect::Value::Enum(
                    ::std::string::String::from(#variant_name),
                    ::std::vec![#(#values),*],
                )
            }
        });
        let known = known(&fields);
        let reads = reads(&fields, false);
        let construct = construct(
            quote!(Self::#variant_ident),
            &variant.fields,
            &fields,
            &reads,
        );
        let names = std::iter::once(variant_name.clone()).chain(aliases.iter().map(LitStr::value));
        from_values.push(quote! {
            #(#names)|* => {
                #known
                #construct
            }
        });
        variant_names.push(variant_name);
    }
    Ok(quote! {
        impl ::slime_reflect::Reflect for #ident {
            fn type_name() -> ::std::string::String {
                ::std::string::String::from(#name)
            }

            fn type_info() -> ::slime_reflect::TypeInfo {
                ::slime_reflect::TypeInfo {
                    name: ::std::string::String::from(#name),
                    #head
                    kind: ::slime_reflect::Kind::Enum(::std::vec![#(#infos),*]),
                }
            }

            fn register_dependencies(registry: &mut ::slime_reflect::Registry) {
                let _ = &registry;
                #(#dependencies)*
            }

            #default_value

            fn to_value(&self) -> ::slime_reflect::Value {
                match self {
                    #(#to_values)*
                }
            }

            fn from_value(
                value: &::slime_reflect::Value,
            ) -> ::std::result::Result<Self, ::slime_reflect::ReflectError> {
                let (variant, fields) = ::slime_reflect::support::variant(value, #name)?;
                let read = match variant {
                    #(#from_values)*
                    other => {
                        return ::std::result::Result::Err(
                            ::slime_reflect::support::unknown_variant(other, &[#(#variant_names),*]),
                        );
                    }
                };
                #finish
            }
        }
        #(#checks)*
    })
}
