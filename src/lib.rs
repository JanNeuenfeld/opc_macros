//! This is the official proc_macro crate for the [`OUTLINE_CLI`].
//! 
//! The result will only work with certain traits defined by [`OUTLINE_CLI`] and should therefore only be used in said crate.
//! 
//!
//! This [`crate`] mainly provides [`new_opc_command!`], which is used for defining structs that represent the given command.
//! [`serve_opc!`] is used to then provide a new `HelpCommand` struct to handle `opc help <command>` calls and serve any other given command.



use proc_macro;

use syn::{parse_macro_input, DeriveInput};
use quote::{quote, format_ident};
use proc_macro2::{TokenStream, TokenTree, Delimiter, token_stream::IntoIter};

#[proc_macro]
pub fn new_opc_command(body: proc_macro::TokenStream) -> proc_macro::TokenStream {

    let body: TokenStream = body.into();

    let mut item_iter = body.into_iter();

    let name;
    if let Some(TokenTree::Literal(c)) = item_iter.next() {
        let mut s: Vec<char> = c.to_string().trim_matches('\"').chars().collect();
        s[0] = s[0].to_uppercase().nth(0).unwrap();
        name = format_ident!("{}Command", s.into_iter().collect::<String>());
    }
    else {panic!("Please provide the command name as literal")}

    let res = parse_fn(item_iter);
    quote!(
        #[derive(Debug, Clone, opc_macros::SuperOpcCommand, Default)]
        pub struct #name {
            #res
        }
    ).into()
}

fn parse_fn(mut body: IntoIter) -> TokenStream {

    let mut out = Vec::new();

    match body.next() {
        Some(TokenTree::Group(list)) => {
            if list.delimiter() != Delimiter::Bracket {panic!("Please use brackets to limit your list of arguments '[]'")}
            let mut tt = list.stream().into_iter();
            let prefix = if let Some(TokenTree::Literal(pre)) = tt.next() {pre.to_string()} else {panic!("Please provide a prefix in literal form as first argument")};
            if prefix == "\"\"" {panic!("Please provide a non-empty prefix")}

            while let Some(n) = tt.next() {
                match n {
                    TokenTree::Ident(field) => {
                        out.push(quote!(
                            #[prefix(#prefix)]
                            pub #field: bool,
                        ));
                    }
                    _ => panic!("Please only use identifiers as argument names"),
                }
            }    
        }
        Some(TokenTree::Ident(field)) => {
            out.push(quote!(
                pub #field: String,
            ));
        }
        Some(_) => panic!("Please provide a list of arguments in brackets '[arg arg2 ...]'"),
        None => return quote!(#(#out)*),
    }
    let res = parse_fn(body);
    quote!(
        #(#out)*
        #res
    )
}

#[proc_macro_derive(SuperOpcCommand, attributes(prefix))]
pub fn sopc_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
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
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let c_name = name.clone().to_string().strip_suffix("Command").unwrap().to_ascii_lowercase();

    quote! {
        impl #impl_generics SuperOpcCommand for #name #ty_generics #where_clause {
            fn parse(args: Vec<String>) -> Option<anyhow::Result<#name>> {
                let mut out = #name::default();
                let mut args = args.into_iter();
                if let Some(c) = args.next() {
                    if c != #c_name {return None}
                }
                #(
                    if let Some(arg) = args.next() {
                        if arg.chars().collect::<Vec<char>>()[0].is_ascii_alphabetic() {
                            out.#no_prefix = arg;
                        } else {return Some(Err(anyhow::anyhow!("missing or invalid argument")))}
                    }
                )*

                for rest in args {
                    #(
                        if rest == #prefixes.to_string() + #fields {
                            out.#field_tokens = true
                        } else
                    )*
                    {
                        return Some(Err(anyhow::anyhow!("Unknown optional value: {}", rest)))
                    }
                }
                Some(Ok(out))
            }
        }
    }.into()
}

#[proc_macro]
pub fn serve_opc(body: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let body: TokenStream = body.into();
    let item_iter = body.into_iter();

    let mut commands = Vec::new();
    let mut command_names = Vec::new();

    for tt in item_iter {
        match tt {
            TokenTree::Ident(cmd) => {
                commands.push(proc_macro2::Ident::new(&cmd.to_string(), cmd.span().into()));
                command_names.push(cmd.to_string().strip_suffix("Command").unwrap().to_ascii_lowercase())
            }
            _ => panic!("Please only provide identifiers")
        }
    }

    quote!(
        let mut args = env::args();
        args.next();
        let args = args.collect::<Vec<String>>();
    
        if args.is_empty() {println!("OUTLINE Plugin Creator {} installed", version); return}

        new_opc_command!("help" cmd);

        impl OpcCommand for HelpCommand {
            fn run(&self) -> String {
                match self.cmd.as_str() {
                    "help" => Self::help(),
                    #(#command_names => {#commands::help()})*
                    _ => {"Unknown Command!".to_string()}
                }
            }

            fn help() -> String {"Available commands:".to_string()#(+ "\n" + #command_names)*}
        }

        if args.len() == 1 && args[0] == "help".to_string() {
            println!("{}", HelpCommand::help())
        } else if let Some(res) = HelpCommand::parse(args.clone()) {
            if let Err(err) = res {println!("{}", err)}
            else {println!("{}", res.unwrap().run())}
        }
        #(else if let Some(res) = #commands::parse(args.clone()) {
            if let Err(err) = res {println!("{}", err)}
            else {println!("{}", res.unwrap().run())}
        })*
        else {println!("Unknown command! Use 'opc help' for further information")}
    ).into()
}