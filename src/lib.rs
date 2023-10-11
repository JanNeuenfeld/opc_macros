extern crate proc_macro;

use proc_macro::{TokenStream, TokenTree, Delimiter, token_stream::IntoIter};
use syn::{parse_macro_input, DeriveInput};
use quote::quote;
use proc_macro2;

#[proc_macro]
pub fn new_opc_command(body: TokenStream) -> TokenStream {

    let mut item_iter = body.into_iter();

    let mut start = String::new();
    if let Some(TokenTree::Literal(c)) = item_iter.next() {
        let mut s: Vec<char> = c.to_string().trim_matches('\"').chars().collect();
        s[0] = s[0].to_uppercase().nth(0).unwrap();
        start += stringify!(
            #[derive(Debug, Clone, opc_macros::SuperOpcCommand, Default)]
            pub struct 
        );
        start += &(" ".to_string() + &s.iter().collect::<String>() + "Command {");
    }
    else {panic!("Please provide the command name as literal")}

    (start + &parse_fn(item_iter) + "}").parse().unwrap()
}

fn parse_fn(body: IntoIter) -> String {

    let mut out = String::new();
    let mut item_iter = body.into_iter();

    match item_iter.next() {
        Some(TokenTree::Group(list)) => {
            if list.delimiter() != Delimiter::Bracket {panic!("Please use brackets to limit your list of arguments '[]'")}
            let mut tt = list.stream().into_iter();
            let prefix = if let Some(TokenTree::Literal(pre)) = tt.next() {pre.to_string()} else {panic!("Please provide a prefix in literal form as first argument")};
            if prefix == "\"\"" {panic!("Please provide a non-empty prefix")}

            while let Some(n) = tt.next() {
                match n {
                    TokenTree::Ident(p) => {
                        out += &("#[prefix(".to_string() + &prefix + ")] pub " + &p.to_string() + ": bool,");
                    }
                    _ => panic!("Please only use identifiers as argument names"),
                }
            }    
        }
        Some(TokenTree::Ident(arg)) => {
            out += &("pub ".to_string() + &arg.to_string() + ": String,");
        }
        Some(_) => panic!("Please provide a list of arguments in brackets '[arg arg2 ...]'"),
        None => return out,
    }
    return out + &parse_fn(item_iter);
}

#[proc_macro_derive(SuperOpcCommand, attributes(prefix))]
pub fn sopc_derive(input: TokenStream) -> TokenStream {
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
    let min_len = no_prefix.len();
    let expanded = quote! {
        impl #impl_generics SuperOpcCommand for #name #ty_generics #where_clause {
            fn parse(args: Vec<String>) -> Option<anyhow::Result<#name>> {
                let mut out = #name::default();
                let mut args = args.into_iter();
                if args.clone().count() < #min_len + 1 {return Some(Err(anyhow::anyhow!("missing argument")))}
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
    };

    TokenStream::from(expanded)
}

#[proc_macro]
pub fn serve_opc(body: TokenStream) -> TokenStream {
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

    let mut out = Vec::new();

    out.push(quote!(
        new_opc_command!("help" cmd);

        impl OpcCommand for HelpCommand {
            fn run(&self) -> String {
                match self.cmd.as_str() {
                    "help" => Self::help(),
                    #(
                        #command_names => {#commands::help()}
                    )*
                    _ => {"Unknown Command!".to_string()}
                }
            }

            fn help() -> String {
                "Available commands:".to_string()#(+ "\n" + #command_names)*
            }
        }
    ));

    out.push(quote!(
            if let Some(res) = HelpCommand::parse(args.clone()) {
                if let Err(err) = res {
                    println!("{}", err)
                } else {
                    println!("{}", res.unwrap().run())
                }
            }
        )
    );

    for cmd in commands {
        out.push(quote!(
            else if let Some(res) = #cmd::parse(args.clone()) {
                if let Err(err) = res {
                    println!("{}", err)
                } else {
                    println!("{}", res.unwrap().run())
                }
            }
        ))
    }

    out.push(quote!(
        else {
            println!("Unknown command! Use 'opc help help' for further information")
        }
    ));

    proc_macro2::TokenStream::from_iter(out).into()
}