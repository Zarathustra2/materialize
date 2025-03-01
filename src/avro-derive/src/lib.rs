// Copyright Materialize, Inc. and contributors. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License in the LICENSE file at the
// root of this repository, or online at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

/// Derive decoders for Rust structs from Avro values.
/// Currently, only the simplest possible case is supported:
/// decoding an Avro record into a struct, each of whose fields
/// is named the same as the corresponding Avro record field
/// and which is in turn decodeable without external state.
///
/// Example:
///
/// ```ignore
/// fn make_complicated_decoder() -> impl AvroDecode<Out = SomeComplicatedType> {
///     unimplemented!()
/// }
/// #[derive(AvroDecodable)]
/// struct MyType {
///     x: i32,
///     y: u64,
///     #[decoder_factory(make_complicated_decoder)]
///     z: SomeComplicatedType
/// }
/// ```
///
/// This will create an Avro decoder that expects a record with fields "x", "y", and "z"
/// (and possibly others), where "x" and "y" are of Avro type Int or Long and their
/// values fit in an `i32` or `u64` respectively,
/// and where "z" can be decoded by the decoder returned from `make_complicated_decoder`.
///
/// This crate currently works by generating a struct named (following the example above)
/// MyType_DECODER which is used internally by the `AvroDecodable` implementation.
/// It also requires that the `mz-avro` crate be linked under its default name.
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse_macro_input;
use syn::ItemStruct;

#[proc_macro_derive(AvroDecodable, attributes(decoder_factory, state_type, state_expr))]
pub fn derive_decodeable(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let state_type = input
        .attrs
        .iter()
        .find(|a| &a.path.get_ident().as_ref().unwrap().to_string() == "state_type")
        .map(|a| a.tokens.clone())
        .unwrap_or(quote! {()});
    let name = input.ident;
    let base_fields: Vec<_> = input
        .fields
        .iter()
        .map(|f| f.ident.as_ref().unwrap())
        .collect();
    let fields: Vec<_> = input
        .fields
        .iter()
        .map(|f| {
            // The type of the field,
            // which must itself be AvroDecodable so that we can recursively
            // decode it.
            let ty = &f.ty;
            let id = f.ident.as_ref().unwrap();
            quote! {
                #id: Option<#ty>
            }
        })
        .collect();

    let field_state_exprs: Vec<_> = input
        .fields
        .iter()
        .map(|f| {
            f.attrs
                .iter()
                .find(|a| &a.path.get_ident().as_ref().unwrap().to_string() == "state_expr")
                .map(|a| a.tokens.clone())
                .unwrap_or(quote! {()})
        })
        .collect();

    let decode_blocks: Vec<_> = input
        .fields
        .iter()
        .zip(field_state_exprs.iter())
        .map(|(f, state_expr)| {
            // The type of the field,
            // which must itself be StatefulAvroDecodable so that we can recursively
            // decode it.
            let ty = &f.ty;
            let id = f.ident.as_ref().unwrap();
            let id_str = id.to_string();
            let found_twice = format!("field `{}` found twice", id);
            let make_decoder =
                if let Some(decoder_factory) = f.attrs.iter().find(|a| {
                    &a.path.get_ident().as_ref().unwrap().to_string() == "decoder_factory"
                }) {
                    let toks = &decoder_factory.tokens;
                    quote! {
                        self.#id = Some(#toks(field));
                    }
                } else {
                    if quote!(#ty).to_string() == "String" {
                        quote! {
                            let decoder = ValueDecoder {};
                            let res = field.decode_field(decoder)?;
                            match res {
                                mz_avro::types::Value::String(v) => self.#id = Some(v),
                                _ => panic!("Failed to decode field, expected value to be String, id {} res: {:?}", #id_str, res),
                            }
                        }
                    } else {
                        quote! {
                            let decoder = <#ty as ::mz_avro::StatefulAvroDecodable>::new_decoder(#state_expr);
                            self.#id = Some(field.decode_field(decoder)?);
                        }
                    }
                };
            quote! {
                #id_str => {
                    if self.#id.is_some() {
                        return Err(::mz_avro::error::Error::Decode(::mz_avro::error::DecodeError::Custom(#found_twice.to_string())));
                    }
                    #make_decoder
                }
            }
        })
        .collect();
    let check_blocks: Vec<_> = input
        .fields
        .iter()
        .map(|f| {
            let id = f.ident.as_ref().unwrap();
            let not_found = format!("field `{}` not found", id);
            quote! {
                let #id = if let Some(#id) = self.#id.take() {
                    #id
                } else {
                    return Err(::mz_avro::error::Error::Decode(::mz_avro::error::DecodeError::Custom(#not_found.to_string())));
                };
            }
        })
        .collect();
    let return_fields: Vec<_> = input
        .fields
        .iter()
        .map(|f| f.ident.as_ref().unwrap())
        .collect();
    let decoder_name = format_ident!("{}_DECODER", name);
    let out = quote! {
        #[derive(Debug)]
        #[allow(non_camel_case_types)]
        pub struct #decoder_name {
            _STATE: #state_type,
            #(#fields),*
        }
        impl ::mz_avro::AvroDecode for #decoder_name {
            type Out = #name;
            fn record<R: ::mz_avro::AvroRead, A: ::mz_avro::AvroRecordAccess<R>>(
                mut self,
                a: &mut A,
            ) -> ::std::result::Result<#name, ::mz_avro::error::Error> {
                while let Some((name, _idx, field)) = a.next_field()? {
                    match name {
                        #(#decode_blocks)*
                        _ => {
                            field.decode_field(::mz_avro::TrivialDecoder)?;
                        }
                    }
                }
                #(#check_blocks)*
                Ok(#name {
                    #(#return_fields),*
                })
            }
            ::mz_avro::define_unexpected! {
                union_branch, array, map, enum_variant, scalar, decimal, bytes, string, json, uuid, fixed
            }
        }
        impl ::mz_avro::StatefulAvroDecodable for #name {
            type Decoder = #decoder_name;
            type State = #state_type;
            fn new_decoder(state: #state_type) -> #decoder_name {
                #decoder_name {
                    _STATE: state,
                    #(#base_fields: None),*
                }
            }
        }
    };
    TokenStream::from(out)
}
