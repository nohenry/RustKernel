use std::{iter::FromIterator, str::FromStr};

use proc_macro::{Delimiter, Group, Ident, Span, TokenStream, TokenTree};

#[proc_macro]
pub fn create_guid(items: TokenStream) -> TokenStream {
    let mut iter = items.into_iter();
    let a = iter.next().unwrap().to_string();
    if a.len() != 8 {
        panic!()
    }
    if let Some(TokenTree::Punct(p)) = iter.next() {
        if p.to_string() != "-" {
            panic!()
        }
    } else {
        panic!();
    }
    let b = iter.next().unwrap().to_string();
    if b.len() != 4 {
        panic!()
    }
    if let Some(TokenTree::Punct(p)) = iter.next() {
        if p.to_string() != "-" {
            panic!()
        }
    } else {
        panic!();
    }
    let c = iter.next().unwrap().to_string();
    if c.len() != 4 {
        panic!()
    }
    if let Some(TokenTree::Punct(p)) = iter.next() {
        if p.to_string() != "-" {
            panic!()
        }
    } else {
        panic!();
    }
    let d1 = iter.next().unwrap().to_string();
    if d1.len() != 4 {
        panic!()
    }
    if let Some(TokenTree::Punct(p)) = iter.next() {
        if p.to_string() != "-" {
            panic!()
        }
    } else {
        panic!();
    }
    let d2 = iter.next().unwrap().to_string();
    if d2.len() != 12 {
        panic!()
    }

    let a = format!("0x{}", a);
    let b = format!("0x{}", b);
    let c = format!("0x{}", c);
    let mut dstr = String::from("[");

    d1.as_bytes()
        .chunks(2)
        .map(|buf| unsafe { std::str::from_utf8_unchecked(buf) })
        .for_each(|s| dstr.push_str(format!("0x{}, ", s).as_str()));
    d2.as_bytes()
        .chunks(2)
        .map(|buf| unsafe { std::str::from_utf8_unchecked(buf) })
        .for_each(|s| dstr.push_str(format!("0x{}, ", s).as_str()));
    dstr.push(']');

    let a = TokenStream::from_str(format!("a: {},", a).as_str()).unwrap();
    let b = TokenStream::from_str(format!("b: {},", b).as_str()).unwrap();
    let c = TokenStream::from_str(format!("c: {},", c).as_str()).unwrap();
    let d = TokenStream::from_str(format!("d: {},", dstr).as_str()).unwrap();
    let tt = vec![
        TokenTree::Ident(Ident::new("GUID", Span::call_site())),
        TokenTree::Group(Group::new(
            Delimiter::Brace,
            TokenStream::from_iter(vec![a, b, c, d]),
        )),
    ];
    let res = TokenStream::from_iter(tt);
    res
}

#[proc_macro]
pub fn wchar(tokens: TokenStream) -> TokenStream {
    let mut iter = tokens.into_iter();
    let chr = iter.next();
    match chr {
        Some(s) => match s {
            TokenTree::Literal(l) => {
                let rstr = l.to_string();

                let mut chars = rstr.chars();
                chars.next();
                chars.next_back();
                let rstr = chars.as_str();

                let estr = unescape::unescape(rstr).unwrap();
                
                let mut dstr = String::from("&[");
                for s in estr.chars() {
                    let mut b = [0; 2];
                    s.encode_utf8(&mut b);
                    dstr.push_str(&format!("0x{:x}{:x}, ", b[1], b[0]));
                }
                dstr.push_str("0x00u16]");

                return TokenStream::from_str(&dstr).unwrap();
            }
            _ => return TokenStream::from(s),
        },
        None => TokenStream::new(),
    }
}
