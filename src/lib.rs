use proc_macro::{self, TokenStream};
use proc_macro2::TokenStream as TokenStream2;
use proc_macro2::{Ident, Span};
use quote::quote;
use syn::{parse_macro_input, DeriveInput, FieldsNamed, Meta};

#[proc_macro_derive(
    CreateInsertableStruct,
    attributes(changeset_options, non_new_fields, table_name)
)]
pub fn create_insertable_struct(input: TokenStream) -> TokenStream {
    let DeriveInput {
        ident, data, attrs, ..
    } = parse_macro_input!(input);

    let changeset_options_attr = match attrs
        .iter()
        .find(|attr| attr.path.is_ident("changeset_options"))
    {
        Some(attr) => attr,
        None => panic!("derive(CreateInsertableStruct) requires a changeset_options attribute"),
    };

    let table_name_attr = match attrs.iter().find(|attr| attr.path.is_ident("table_name")) {
        Some(attr) => attr,
        None => panic!("derive(CreateInsertableStruct) requires a table_name attribute"),
    };

    let non_new_fields_attr = attrs
        .iter()
        .find(|attr| attr.path.is_ident("non_new_fields"));

    let non_new_field_names: Vec<String> = match non_new_fields_attr {
        Some(attr) => match attr.parse_args() {
            Ok(Meta::List(list)) => list
                .nested
                .iter()
                .map(|nested| match nested {
                    syn::NestedMeta::Meta(Meta::Path(path)) => path
                        .get_ident()
                        .expect("non_new_fields must be a list of identifiers")
                        .to_string(),
                    _ => panic!("non_new_fields must be a list of identifiers"),
                })
                .collect(),
            _ => panic!("non_new_fields must be a list of identifiers"),
        },
        None => vec![
            "created_at".to_string(),
            "updated_at".to_string(),
            "id".to_string(),
        ],
    };

    let struct_token = match data {
        syn::Data::Struct(s) => s,
        _ => panic!("derive(CreateInsertableStruct) only supports structs"),
    };

    let fields = match struct_token.fields {
        syn::Fields::Named(FieldsNamed { named, .. }) => named,
        _ => panic!("derive(CreateInsertableStruct) only supports named fields"),
    };

    let (idents, types): (Vec<_>, Vec<_>) = fields
        .iter()
        .filter_map(|f| match f.ident {
            Some(ref i) => Some((i, &f.ty)),
            None => None,
        })
        .unzip();

    let mut filtered_field_declarations = TokenStream2::default();
    let mut into_field_declaration = TokenStream2::default();
    let mut into_ref_field_declaration = TokenStream2::default();

    idents
        .into_iter()
        .zip(types.into_iter())
        .for_each(|(field, ftype)| {
            if non_new_field_names.contains(&field.to_string()) {
                return;
            }

            filtered_field_declarations.extend::<TokenStream2>(quote! { pub #field : #ftype, });
            into_field_declaration.extend::<TokenStream2>(quote! { #field : self.#field, });
            into_ref_field_declaration
                .extend::<TokenStream2>(quote! { #field : self.#field.clone(), });
        });

    let struct_name = Ident::new(&format!("Insertable{}", ident), Span::call_site());

    let output = quote! {

        use crate::util::*;
        use crate::db_connection::*;
        use diesel::prelude::*;

        #[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Insertable, AsChangeset, TS)]
        #[ts(export)]
        #table_name_attr
        #changeset_options_attr
        pub struct #struct_name {
            #filtered_field_declarations
        } impl Into<#struct_name> for #ident {
            fn into(self) -> #struct_name {
                #struct_name {
                    #into_field_declaration
                }
            }
        } impl Into<#struct_name> for &#ident {
            fn into(self) -> #struct_name {
                #struct_name {
                    #into_ref_field_declaration
                }
            }
        }
    };

    output.into()
}
