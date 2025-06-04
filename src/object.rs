//! オブジェクトファイル関連。

use xo65::{section::Section, Xo65};

use crate::index::{ObjImportIdx, ObjSectIdx, ObjStrIdx};

#[derive(Debug)]
pub struct Object<'data> {
    name: String,
    xo65: Xo65<'data>,
}

impl<'data> Object<'data> {
    pub fn new<S: Into<String>>(name: S, xo65: Xo65<'data>) -> Self {
        Self {
            name: name.into(),
            xo65,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn xo65(&self) -> &Xo65<'data> {
        &self.xo65
    }

    pub fn section(&self, i: ObjSectIdx) -> &Section<'data> {
        self.xo65
            .section_table()
            .get(i.get())
            .unwrap_or_else(|| panic!("'{}': section index out of range: {i}", self.name()))
    }

    pub fn enumerate_sections(
        &self,
    ) -> impl ExactSizeIterator<Item = (ObjSectIdx, &Section<'data>)> + std::iter::FusedIterator + Clone
    {
        self.xo65
            .section_table()
            .iter()
            .enumerate()
            .map(|(i, x)| (ObjSectIdx::new(i), x))
    }

    pub fn query_segment_name(&self, i: ObjSectIdx) -> &'data str {
        let obj_sect = self
            .xo65
            .section_table()
            .get(i.get())
            .unwrap_or_else(|| panic!("'{}': section index out of range: {i}", self.name()));

        self.query_string(ObjStrIdx::new(obj_sect.segment_name() as usize))
    }

    pub fn query_import_name(&self, i: ObjImportIdx) -> &'data str {
        let obj_imp = self
            .xo65
            .import_table()
            .get(i.get())
            .unwrap_or_else(|| panic!("'{}': import index out of range: {i}", self.name()));

        self.query_string(ObjStrIdx::new(obj_imp.name() as usize))
    }

    pub fn query_string(&self, i: ObjStrIdx) -> &'data str {
        let s = self
            .xo65
            .string_table()
            .get(i.get())
            .unwrap_or_else(|| panic!("'{}': string index out of range: {i}", self.name()));

        std::str::from_utf8(s)
            .unwrap_or_else(|e| panic!("'{}': string is not utf-8: {s:?}: {e}", self.name()))
    }
}
