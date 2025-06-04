use anyhow::anyhow;
use winnow::{
    ascii::{multispace1 as ws1, Caseless},
    combinator::{alt, cut_err, delimited, dispatch, fail, opt, peek, preceded, repeat, seq},
    token::{any, one_of, take_till, take_while},
    Parser as _,
};

use super::ast;

/// リンカスクリプトをパースし、AST を返す。
pub fn parse(s: &str) -> anyhow::Result<ast::Script> {
    script.parse(s).map_err(|e| anyhow!("{e}"))
}

type ParseResult<T> = winnow::ModalResult<T>;

fn script(input: &mut &str) -> ParseResult<ast::Script> {
    delimited(ign, block0, ign)
        .map(|blocks| ast::Script { blocks })
        .parse_next(input)
}

fn block0(input: &mut &str) -> ParseResult<Box<[ast::Block]>> {
    let mut blocks = Vec::<ast::Block>::new();

    match opt(block).parse_next(input) {
        Ok(Some(first)) => blocks.push(first),
        Ok(None) => return Ok(blocks.into()),
        Err(e) => return Err(e),
    }

    let remain: Vec<_> = repeat(0.., preceded(ign, block)).parse_next(input)?;
    blocks.extend(remain);

    Ok(blocks.into())
}

fn block(input: &mut &str) -> ParseResult<ast::Block> {
    seq! { ast::Block {
        name: identifier.map(|s| s.to_ascii_lowercase()), // 小文字に統一
        _: cut_err(delimited(ign, '{', ign)),
        elems: cut_err(element0),
        _: cut_err(preceded(ign, '}')),
    }}
    .parse_next(input)
}

fn element0(input: &mut &str) -> ParseResult<Box<[ast::Element]>> {
    let mut elems = Vec::<ast::Element>::new();

    match opt(element).parse_next(input) {
        Ok(Some(first)) => elems.push(first),
        Ok(None) => return Ok(elems.into()),
        Err(e) => return Err(e),
    }

    let remain: Vec<_> = repeat(0.., preceded(ign, element)).parse_next(input)?;
    elems.extend(remain);

    Ok(elems.into())
}

fn element(input: &mut &str) -> ParseResult<ast::Element> {
    seq! { ast::Element {
        name: identifier,
        _: cut_err(delimited(ign, ':', ign)),
        attrs: cut_err(attribute1),
        _: cut_err(preceded(ign, ';')),
    }}
    .parse_next(input)
}

fn attribute1(input: &mut &str) -> ParseResult<Box<[ast::Attribute]>> {
    let mut attrs = vec![attribute.parse_next(input)?];

    let remain: Vec<_> = repeat(0.., preceded(attributes_sep, attribute)).parse_next(input)?;
    attrs.extend(remain);

    Ok(attrs.into())
}

fn attributes_sep(input: &mut &str) -> ParseResult<()> {
    alt((delimited(ign, ',', ign).void(), ws1.void())).parse_next(input)
}

fn attribute(input: &mut &str) -> ParseResult<ast::Attribute> {
    seq! { ast::Attribute {
        key: identifier.map(|s| s.to_ascii_lowercase()), // 小文字に統一
        _: cut_err(attribute_kv_sep),
        value: cut_err(attribute_value),
    }}
    .parse_next(input)
}

fn attribute_kv_sep(input: &mut &str) -> ParseResult<()> {
    alt((delimited(ign, '=', ign).void(), ws1.void())).parse_next(input)
}

fn attribute_value(input: &mut &str) -> ParseResult<ast::Value> {
    alt((
        boolean.map(ast::Value::Bool),
        identifier.map(ast::Value::Ident),
        output_file.map(ast::Value::String),
        string.map(ast::Value::String),
        uint.map(ast::Value::Uint),
    ))
    .parse_next(input)
}

fn boolean(input: &mut &str) -> ParseResult<bool> {
    alt((
        Caseless("yes").value(true),
        Caseless("no").value(false),
        Caseless("true").value(true),
        Caseless("false").value(false),
    ))
    .parse_next(input)
}

fn identifier(input: &mut &str) -> ParseResult<String> {
    (
        one_of(|ch: char| ch.is_ascii_alphabetic() || ch == '_'),
        take_while(0.., |ch: char| ch.is_ascii_alphanumeric() || ch == '_'),
    )
        .take()
        .map(str::to_owned)
        .parse_next(input)
}

fn output_file(input: &mut &str) -> ParseResult<ast::FormatString> {
    "%O".value(ast::FormatString {
        parts: [ast::FormatStringPart::MainOutFile].into(),
    })
    .parse_next(input)
}

fn string(input: &mut &str) -> ParseResult<ast::FormatString> {
    delimited('"', string_inner, '"').parse_next(input)
}

fn string_inner(input: &mut &str) -> ParseResult<ast::FormatString> {
    let parts: Vec<_> = repeat(0.., string_part).parse_next(input)?;

    Ok(ast::FormatString {
        parts: parts.into(),
    })
}

fn string_part(input: &mut &str) -> ParseResult<ast::FormatStringPart> {
    alt((
        "%O".value(ast::FormatStringPart::MainOutFile),
        "%%".value(ast::FormatStringPart::EscapedPercent),
        string_part_literal,
    ))
    .parse_next(input)
}

fn string_part_literal(input: &mut &str) -> ParseResult<ast::FormatStringPart> {
    take_till(1.., ['"', '%'])
        .map(|s: &str| ast::FormatStringPart::Literal(s.to_owned()))
        .parse_next(input)
}

fn uint(input: &mut &str) -> ParseResult<u32> {
    dispatch! { peek(any);
        '%' => preceded('%', uint_bin_digits),
        '$' => preceded('$', uint_hex_digits),
        '0'..='9' => uint_dec_digits,
        _ => fail,
    }
    .parse_next(input)
}

fn uint_bin_digits(input: &mut &str) -> ParseResult<u32> {
    take_while(1.., '0'..='1')
        .try_map(|s| u32::from_str_radix(s, 2))
        .parse_next(input)
}

fn uint_dec_digits(input: &mut &str) -> ParseResult<u32> {
    take_while(1.., '0'..='9')
        .try_map(str::parse)
        .parse_next(input)
}

fn uint_hex_digits(input: &mut &str) -> ParseResult<u32> {
    take_while(1.., ('0'..='9', 'A'..='F', 'a'..='f'))
        .try_map(|s| u32::from_str_radix(s, 16))
        .parse_next(input)
}

/// コメントと空白文字を読み飛ばす。
fn ign(input: &mut &str) -> ParseResult<()> {
    repeat(0.., alt((comment, ws1.void())))
        .map(|()| ())
        .parse_next(input)
}

fn comment(input: &mut &str) -> ParseResult<()> {
    ('#', take_till(1.., ['\n', '\r'])).void().parse_next(input)
}
