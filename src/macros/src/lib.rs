use std::{str::FromStr, iter::FromIterator};

use proc_macro::{TokenStream, TokenTree, Ident, Span, Group, Delimiter};

#[proc_macro]
pub fn create_guid(items: TokenStream) -> TokenStream {
    let mut iter = items.into_iter();
    let a = iter.next().unwrap().to_string();
    if a.len() != 8 {panic!()}
    if let Some(TokenTree::Punct(p)) = iter.next() {
        if p.to_string() != "-" {panic!()}
    } else {
        panic!();
    }
    let b = iter.next().unwrap().to_string();
    if b.len() != 4 {panic!()}
    if let Some(TokenTree::Punct(p)) = iter.next() {
        if p.to_string() != "-" {panic!()}
    } else {
        panic!();
    }
    let c = iter.next().unwrap().to_string();
    if c.len() != 4 {panic!()}
    if let Some(TokenTree::Punct(p)) = iter.next() {
        if p.to_string() != "-" {panic!()}
    } else {
        panic!();
    }
    let d1 = iter.next().unwrap().to_string();
    if d1.len() != 4 {panic!()}
    if let Some(TokenTree::Punct(p)) = iter.next() {
        if p.to_string() != "-" {panic!()}
    } else {
        panic!();
    }
    let d2 = iter.next().unwrap().to_string();
    if d2.len() != 12 {panic!()}

    let a = format!("0x{}", a);
    let b = format!("0x{}", b);
    let c = format!("0x{}", c);
    let mut dstr = String::from("[");

    d1.as_bytes()
        .chunks(2)
        .map(|buf| unsafe { std::str::from_utf8_unchecked(buf) })
        .for_each(|s| {dstr.push_str(format!("0x{}, ", s).as_str())});
    d2.as_bytes()
        .chunks(2)
        .map(|buf| unsafe { std::str::from_utf8_unchecked(buf) })
        .for_each(|s| {dstr.push_str(format!("0x{}, ", s).as_str())});
    dstr.push(']');

    let a = TokenStream::from_str(format!("a: {},", a).as_str()).unwrap();
    let b = TokenStream::from_str(format!("b: {},", b).as_str()).unwrap();
    let c = TokenStream::from_str(format!("c: {},", c).as_str()).unwrap();
    let d = TokenStream::from_str(format!("d: {},", dstr).as_str()).unwrap();
    let tt = vec![TokenTree::Ident(Ident::new("GUID", Span::call_site())),
        TokenTree::Group(Group::new(Delimiter::Brace, TokenStream::from_iter(vec![a, b, c, d])))
    ];
    let res = TokenStream::from_iter(tt);
    res
}
