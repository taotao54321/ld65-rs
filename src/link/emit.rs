use xo65::{
    expr::{Expr, ExprBinary, ExprUnary},
    section::SectionFragmentBody,
};

use crate::index::{MemIdx, ObjIdx, ObjImportIdx, ObjSectIdx, OutFileIdx, SegIdx};
use crate::object::Object;

use super::graph::LinkGraph;
use super::layout::LinkLayout;
use super::symbol::SymbolTable;

pub fn emit_file(
    objs: &[Object],
    graph: &LinkGraph,
    layout: &LinkLayout,
    sym_table: &SymbolTable,
    file_i: OutFileIdx,
) -> Box<[u8]> {
    Emitter {
        objs,
        graph,
        layout,
        sym_table,
    }
    .emit_file(file_i)
}

#[derive(Debug)]
struct Emitter<'objs, 'data, 'graph, 'layout, 'sym_table> {
    objs: &'objs [Object<'data>],
    graph: &'graph LinkGraph,
    layout: &'layout LinkLayout,
    sym_table: &'sym_table SymbolTable,
}

impl<'objs, 'data, 'graph, 'layout, 'sym_table> Emitter<'objs, 'data, 'graph, 'layout, 'sym_table> {
    fn emit_file(&self, file_i: OutFileIdx) -> Box<[u8]> {
        let mut buf = vec![0_u8; self.layout.file(file_i).len()];

        for mem_i in self.graph.file_to_mems(file_i) {
            let layout_mem = self.layout.memory(mem_i);
            if layout_mem.output_is_empty() {
                continue;
            }

            let off = layout_mem.file_offset();
            let len = layout_mem.output_len();
            let buf = &mut buf[off..][..len];

            buf.fill(layout_mem.fill_byte());
            self.emit_memory(buf, mem_i);
        }

        buf.into()
    }

    fn emit_memory(&self, buf: &mut [u8], mem_i: MemIdx) {
        let layout_mem = self.layout.memory(mem_i);

        for seg_i in self.graph.mem_to_segs(mem_i) {
            let layout_seg = self.layout.segment(seg_i);
            if layout_seg.output_is_empty() {
                continue;
            }

            let off = layout_seg.start() - layout_mem.start();
            let len = layout_seg.output_len();
            let buf = &mut buf[off..][..len];

            let fill_byte = if let Some(b) = layout_seg.fill_byte() {
                buf.fill(b);
                b
            } else {
                layout_mem.fill_byte()
            };
            self.emit_segment(buf, seg_i, fill_byte);
        }
    }

    fn emit_segment(&self, buf: &mut [u8], seg_i: SegIdx, fill_byte: u8) {
        let layout_seg = self.layout.segment(seg_i);

        for sect_i in self.graph.seg_to_sects(seg_i) {
            let layout_sect = self.layout.section(sect_i);
            if layout_sect.output_is_empty() {
                continue;
            }

            let (obj_i, obj_sect_i) = self.graph.sect_to_obj_sect(sect_i);

            let off = layout_sect.start() - layout_seg.start();
            let len = layout_sect.output_len();
            let buf = &mut buf[off..][..len];

            self.emit_section(buf, obj_i, obj_sect_i, fill_byte);
        }
    }

    fn emit_section(&self, buf: &mut [u8], obj_i: ObjIdx, obj_sect_i: ObjSectIdx, fill_byte: u8) {
        let obj = &self.objs[obj_i.get()];

        let mut off = 0;

        macro_rules! emit_expr {
            ($ty:ty, $expr:expr) => {{
                let value = self.eval_expr(obj_i, $expr);
                let value: $ty = value.try_into().expect("expr value overflow");
                value.emit_at(buf, &mut off);
            }};
        }

        for frag in obj.section(obj_sect_i).fragments() {
            match frag.body() {
                SectionFragmentBody::Literal(lit) => lit.emit_at(buf, &mut off),
                SectionFragmentBody::Fill(len) => {
                    emit_fill(buf, &mut off, *len as usize, fill_byte)
                }
                SectionFragmentBody::ExprU8(expr) => emit_expr!(u8, expr),
                SectionFragmentBody::ExprU16(expr) => emit_expr!(u16, expr),
                SectionFragmentBody::ExprU24(expr) => emit_expr!(U24, expr),
                SectionFragmentBody::ExprU32(expr) => emit_expr!(u32, expr),
                SectionFragmentBody::ExprI8(expr) => emit_expr!(i8, expr),
                SectionFragmentBody::ExprI16(expr) => emit_expr!(i16, expr),
                SectionFragmentBody::ExprI24(expr) => emit_expr!(I24, expr),
                SectionFragmentBody::ExprI32(expr) => emit_expr!(i32, expr),
            }
        }
    }

    fn eval_expr(&self, obj_i: ObjIdx, expr: &Expr) -> i64 {
        match expr {
            Expr::Null => panic!("expr is null"),
            Expr::Literal { value } => *value,
            Expr::Symbol { import_idx } => {
                let obj_imp_i = ObjImportIdx::new(*import_idx as usize);
                self.sym_table.get(obj_i, obj_imp_i).value()
            }
            Expr::Section { section_idx } => {
                let obj_sect_i = ObjSectIdx::new(*section_idx as usize);
                let sect_i = self
                    .graph
                    .obj_sect_to_sect(obj_i, obj_sect_i)
                    .unwrap_or_else(|| {
                        panic!("unknown section: obj_i={obj_i}, obj_sect_i={obj_sect_i}")
                    });
                self.layout.section(sect_i).start() as i64
            }
            Expr::Unary(unary) => {
                let ExprUnary { op, expr } = unary.as_ref();
                let expr_value = self.eval_expr(obj_i, expr);
                op.apply(expr_value)
            }
            Expr::Binary(binary) => {
                let ExprBinary { op, lhs, rhs } = binary.as_ref();
                let lhs_value = self.eval_expr(obj_i, lhs);
                let rhs_value = self.eval_expr(obj_i, rhs);
                op.apply(lhs_value, rhs_value)
            }
        }
    }
}

trait EmitAt {
    fn emit_at(&self, buf: &mut [u8], off: &mut usize);
}

impl EmitAt for [u8] {
    fn emit_at(&self, buf: &mut [u8], off: &mut usize) {
        let buf = &mut buf[*off..][..self.len()];

        buf.copy_from_slice(self);

        *off += self.len();
    }
}

impl<const LEN: usize> EmitAt for [u8; LEN] {
    fn emit_at(&self, buf: &mut [u8], off: &mut usize) {
        let buf = &mut buf[*off..][..LEN];

        buf.copy_from_slice(self);

        *off += LEN;
    }
}

macro_rules! impl_emit_at_for_int {
    ($($ty:ty)*) => {
        $(
            impl EmitAt for $ty {
                fn emit_at(&self, buf: &mut [u8], off: &mut usize) {
                    self.to_le_bytes().emit_at(buf, off);
                }
            }
        )*
    };
}

impl_emit_at_for_int!(i8 i16 i32 u8 u16 u32);

impl EmitAt for I24 {
    fn emit_at(&self, buf: &mut [u8], off: &mut usize) {
        self.0.to_le_bytes()[..3].emit_at(buf, off);
    }
}

impl EmitAt for U24 {
    fn emit_at(&self, buf: &mut [u8], off: &mut usize) {
        self.0.to_le_bytes()[..3].emit_at(buf, off);
    }
}

fn emit_fill(buf: &mut [u8], off: &mut usize, len: usize, b: u8) {
    let buf = &mut buf[*off..][..len];

    buf.fill(b);

    *off += len;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct I24(i32);

impl TryFrom<i64> for I24 {
    type Error = ();

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        matches!(value, -0x800000..=0x7FFFFF)
            .then_some(Self(value as i32))
            .ok_or(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct U24(u32);

impl TryFrom<i64> for U24 {
    type Error = ();

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        matches!(value, 0..=0xFFFFFF)
            .then_some(Self(value as u32))
            .ok_or(())
    }
}
