//! リンカスクリプトの AST。

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Script {
    pub blocks: Box<[Block]>,
}

/// `BLOCK_NAME { ... }`
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    /// ブロック名。全てのアルファベットは小文字に置換されている。
    pub name: String,
    pub elems: Box<[Element]>,
}

/// `ELEMENT_NAME: key=value, ...;`
///
/// '=', ',' は省略可。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Element {
    pub name: String,
    pub attrs: Box<[Attribute]>,
}

/// `key=value`
///
/// '=' は省略可。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    /// キー。全てのアルファベットは小文字に置換されている。
    pub key: String,
    pub value: Value,
}

/// リンカスクリプト内の値。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Uint(u32),
    Bool(bool),
    String(FormatString),
    // NOTE: 便宜上 "zp", "bss" などもここに含める。
    // オリジナルではこれらは文脈依存キーワードになっている (ので、メモリ名に ZP を使ったりできる)。
    Ident(String),
}

impl Value {
    pub fn as_uint(&self) -> Option<u32> {
        if let Self::Uint(x) = self {
            Some(*x)
        } else {
            None
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(x) = self {
            Some(*x)
        } else {
            None
        }
    }

    pub fn as_string(&self) -> Option<&FormatString> {
        if let Self::String(s) = self {
            Some(s)
        } else {
            None
        }
    }

    pub fn as_ident(&self) -> Option<&str> {
        if let Self::Ident(ident) = self {
            Some(ident)
        } else {
            None
        }
    }
}

/// リンカスクリプト内の文字列。
///
/// * "%O" はメイン出力ファイル名に置換される。
/// * "%%" は '%' に置換される。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormatString {
    pub parts: Box<[FormatStringPart]>,
}

impl FormatString {
    pub fn format(&self, main_outfile: &str) -> String {
        let mut res = String::new();

        for part in &self.parts {
            match part {
                FormatStringPart::Literal(s) => res.push_str(s),
                FormatStringPart::MainOutFile => res.push_str(main_outfile),
                FormatStringPart::EscapedPercent => res.push('%'),
            }
        }

        res
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FormatStringPart {
    Literal(String),
    /// "%O"
    MainOutFile,
    /// "%%"
    EscapedPercent,
}
