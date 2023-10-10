extern crate proc_macro;

use proc_macro::{TokenStream, TokenTree, Delimiter};
use syn::{parse_macro_input, DeriveInput};
use quote::quote;
use proc_macro2;

#[derive(Debug, Clone)]
enum Info {
    Start,
    Params
}

#[proc_macro_attribute]
pub fn prefix(_: TokenStream, _: TokenStream) -> TokenStream {TokenStream::new()}

#[proc_macro]
pub fn new_opc_command(body: TokenStream) -> TokenStream {

    let res = parse_fn(body, Info::Start);
    return (res + "}").parse().unwrap();
}

fn parse_fn(body: TokenStream, info: Info, ) -> String {

    let mut out = String::new();
    let mut item_iter = body.into_iter();
    let mut info = info;

    match &info {
        Info::Start => {
            if let Some(TokenTree::Literal(c)) = item_iter.next() {
                let mut s: Vec<char> = c.to_string().trim_matches('\"').chars().collect();
                s[0] = s[0].to_uppercase().nth(0).unwrap();
                out += stringify!(
                    #[derive(Debug, Clone, opc_macros::SuperOpcCommand, Default)]
                    pub struct 
                );
                out += &(" ".to_string() + &s.iter().collect::<String>() + "Command {");
                info = Info::Params;
            }
            else {panic!("Please provide the command name as literal")}
        }
        Info::Params => {
            match item_iter.next() {
                Some(TokenTree::Group(list)) => {
                    if list.delimiter() != Delimiter::Bracket {panic!("Please use brackets to limit your list of arguments '[]'")}
                    let mut tt = list.stream().into_iter();
                    let prefix = if let Some(TokenTree::Literal(pre)) = tt.next() {pre.to_string()} else {"".to_string()}; 

                    while let Some(n) = tt.next() {
                        eprintln!("{}", n.to_string());
                        match n {
                            TokenTree::Ident(p) => {
                                if prefix != "\"\"" {out += &("#[prefix(".to_string() + &prefix + ")]");}
                                out += &("pub ".to_string() + &p.to_string() + {if prefix != "\"\"" {": bool,"} else {": String,"}} );
                            }
                            _ => panic!("Please only use identifiers as argument names (no '-', '+', ...) ('_' allowed)"),
                        }
                    }    
                }
                Some(_) => panic!("Please provide a list of arguments in brackets '[arg arg2 ...]'"),
                None => return out,
            }
        }
    }
    return out + &parse_fn(item_iter.collect(), info);
}

#[proc_macro_derive(SuperOpcCommand, attributes(prefix))]
pub fn writable_template_derive(input: TokenStream) -> TokenStream {
    eprintln!("{}", input.to_string());
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = &input.generics;
    let mut fields = Vec::new();
    let mut no_prefix = Vec::new();
    let mut prefixes = Vec::new();
    let mut field_tokens = Vec::new();
    match &input.data {
        syn::Data::Struct(ref data_struct) => {
            match data_struct.fields {
                syn::Fields::Named(ref fields_named) => {
                    for field in fields_named.named.iter() {
                        let mut prefix = String::new();
                        for i in field.attrs.iter() {
                            match i.parse_args::<proc_macro2::TokenTree>() {
                                Ok(proc_macro2::TokenTree::Literal(pre)) => prefix = pre.to_string(),
                                _ => panic!("invalid attribute syntax")
                            };
                        }
                        if !prefix.is_empty() {
                            fields.push(field.ident.clone().unwrap().to_string());
                            field_tokens.push(field.ident.clone());
                            prefixes.push(prefix.trim_matches('\"').to_string());
                        } else {
                            no_prefix.push(field.ident.clone())
                        }
                    }
                },
                _ => (),
            }
        },
        _ => panic!("Must be a struct"),
    }
    eprintln!("{:?}", fields);
    eprintln!("{:?}", prefixes);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let c_name = name.clone().to_string().strip_suffix("Command").unwrap().to_ascii_lowercase();
    let min_len = no_prefix.len();
    let expanded = quote! {
        impl #impl_generics SuperOpcCommand for #name #ty_generics #where_clause {
            fn parse(args: std::env::Args) -> anyhow::Result<#name> {
                let mut out = #name::default();
                let mut a = args.collect::<Vec<String>>().into_iter();
                if a.clone().count() < #min_len + 1 {bail!("missing argument")}
                if let Some(c) = a.next() {
                    if c != #c_name {bail!("wrong command")}
                }
                #(
                    if let Some(arg) = a.next() {
                        if arg.chars().collect::<Vec<char>>()[0].is_ascii_alphabetic() {
                            out.#no_prefix = arg;
                        } else {bail!("missing argument")}
                    }
                )*
                for rest in a {
                    #(
                        if rest == #prefixes.to_string() + #fields {
                            out.#field_tokens = true
                        }
                    )*
                }
                Ok(out)
            }
        }
    };

    TokenStream::from(expanded)
}