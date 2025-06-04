use xo65::expr::{Expr, ExprBinary, ExprUnary};

use crate::index::{ObjIdx, ObjImportIdx, ObjSectIdx, ObjStrIdx};
use crate::object::Object;

use super::graph::LinkGraph;
use super::layout::LinkLayout;

/// 解決済みのシンボルテーブル。
///
/// 全オブジェクトファイルの全インポートシンボルに対する値を保持する。
#[derive(Debug)]
pub struct SymbolTable {
    table: Box<[Box<[SymbolEntry]>]>,
}

impl SymbolTable {
    pub fn get(&self, obj_i: ObjIdx, imp_i: ObjImportIdx) -> &SymbolEntry {
        &self.table[obj_i.get()][imp_i.get()]
    }

    pub fn new(objs: &[Object], graph: &LinkGraph, layout: &LinkLayout) -> Self {
        let exports = build_exports(objs);

        Resolver::new(objs, graph, layout, exports).solve()
    }
}

/// 解決済みのシンボルテーブルエントリ。
#[derive(Debug)]
pub struct SymbolEntry {
    #[allow(dead_code)]
    addr_size: u8,
    value: i64,
}

impl SymbolEntry {
    #[allow(dead_code)]
    pub fn addr_size(&self) -> u8 {
        self.addr_size
    }

    pub fn value(&self) -> i64 {
        self.value
    }
}

type Exports<'objs, 'data> = indexmap::IndexMap<&'data str, ExportDesc<'objs>>;

/// エクスポートシンボルの内容。
#[derive(Clone, Debug, Eq, PartialEq)]
struct ExportDesc<'objs> {
    obj_i: ObjIdx,
    addr_size: u8,
    expr: &'objs Expr,
}

/// 全オブジェクトファイルを通じたエクスポートテーブルを構築する。
fn build_exports<'objs, 'data>(objs: &'objs [Object<'data>]) -> Exports<'objs, 'data> {
    let mut exports = Exports::new();

    for (obj_i, obj) in objs.iter().enumerate() {
        let obj_i = ObjIdx::new(obj_i);

        for export in obj.xo65().export_table().iter() {
            let name = ObjStrIdx::new(export.name() as usize);
            let name = obj.query_string(name);
            let desc = ExportDesc {
                obj_i,
                addr_size: export.addr_size(),
                expr: export.expr(),
            };
            let old = exports.insert(name, desc);
            assert_eq!(old, None, "duplicate export: '{name}'");
        }
    }

    exports
}

/// 全オブジェクトファイルのインポートシンボルを即値に解決するソルバー。
#[derive(Debug)]
struct Resolver<'objs, 'data, 'graph, 'layout> {
    objs: &'objs [Object<'data>],
    graph: &'graph LinkGraph,
    layout: &'layout LinkLayout,
    exports: Exports<'objs, 'data>,
}

impl<'objs, 'data, 'graph, 'layout> Resolver<'objs, 'data, 'graph, 'layout> {
    fn new(
        objs: &'objs [Object<'data>],
        graph: &'graph LinkGraph,
        layout: &'layout LinkLayout,
        exports: Exports<'objs, 'data>,
    ) -> Self {
        Self {
            objs,
            graph,
            layout,
            exports,
        }
    }

    fn solve(&self) -> SymbolTable {
        let mut table = Vec::<Vec<ResolveEntry>>::with_capacity(self.objs.len());

        // 全オブジェクトファイルのインポートテーブルを走査し、
        // 全インポートシンボルを未解決とする (このとき参照すべき Exports 内インデックスを求めておく)。
        for obj in self.objs {
            let mut table_row =
                Vec::<ResolveEntry>::with_capacity(obj.xo65().import_table().count());

            for import in obj.xo65().import_table().iter() {
                let name = ObjStrIdx::new(import.name() as usize);
                let name = obj.query_string(name);
                let export_i = self
                    .exports
                    .get_index_of(name)
                    .unwrap_or_else(|| panic!("'{}': symbol '{name}' is not exported", obj.name()));
                let entry = ResolveEntry {
                    addr_size: import.addr_size(),
                    state: ResolveState::Unresolved { export_i },
                };
                table_row.push(entry);
            }

            table.push(table_row);
        }

        // 全オブジェクトファイルの全インポートシンボルを解決する。
        // table を用いたメモ化再帰。
        for obj_i in (0..self.objs.len()).map(ObjIdx::new) {
            for imp_i in (0..table[obj_i.get()].len()).map(ObjImportIdx::new) {
                self.resolve_import(&mut table, obj_i, imp_i);
            }
        }

        let table: Box<[_]> = table
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|entry| {
                        let addr_size = entry.addr_size;
                        let ResolveState::Done(value) = entry.state else {
                            unreachable!();
                        };
                        SymbolEntry { addr_size, value }
                    })
                    .collect::<Box<[_]>>()
            })
            .collect();

        SymbolTable { table }
    }

    /// 指定されたインポートシンボルを解決する (メモ化再帰)。
    fn resolve_import(&self, table: &mut ResolveTable, obj_i: ObjIdx, imp_i: ObjImportIdx) -> i64 {
        // これはエラーが起きない限り参照されない (最適化で消える、はず)。
        let name = self.objs[obj_i.get()].query_import_name(imp_i);

        let entry = table[obj_i][imp_i];

        let value = match entry.state {
            ResolveState::Done(value) => value,
            ResolveState::Resolving => panic!("circular reference for symbol '{name}'",),
            ResolveState::Unresolved { export_i } => {
                table[obj_i][imp_i].state = ResolveState::Resolving;
                let export = &self.exports[export_i];
                assert_eq!(
                    entry.addr_size, export.addr_size,
                    "address size mismatch for symbol '{name}'",
                );
                self.resolve_expr(table, export_i, export.expr)
            }
        };

        table[obj_i][imp_i].state = ResolveState::Done(value);

        value
    }

    /// 指定された式を解決する (再帰関数)。
    fn resolve_expr(&self, table: &mut ResolveTable, export_i: usize, expr: &Expr) -> i64 {
        // TODO: unary, binary の式の中では addr_size は統一されてる?特にチェック不要?

        let export = &self.exports[export_i];

        match expr {
            Expr::Null => panic!("expr is null"),
            Expr::Literal { value } => *value,
            Expr::Symbol { import_idx } => {
                let imp_i_nxt = ObjImportIdx::new(*import_idx as usize);
                let entry_nxt = table[export.obj_i][imp_i_nxt];
                assert_eq!(
                    export.addr_size, entry_nxt.addr_size,
                    "address size mismatch"
                );
                self.resolve_import(table, export.obj_i, imp_i_nxt)
            }
            Expr::Section { section_idx } => {
                let sect_i = self
                    .graph
                    .obj_sect_to_sect(export.obj_i, ObjSectIdx::new(*section_idx as usize))
                    .expect("invalid section expr");
                self.layout.section(sect_i).start() as i64
            }
            Expr::Unary(unary) => {
                let ExprUnary { op, expr } = unary.as_ref();
                let expr_value = self.resolve_expr(table, export_i, expr);
                op.apply(expr_value)
            }
            Expr::Binary(binary) => {
                let ExprBinary { op, lhs, rhs } = binary.as_ref();
                let lhs_value = self.resolve_expr(table, export_i, lhs);
                let rhs_value = self.resolve_expr(table, export_i, rhs);
                op.apply(lhs_value, rhs_value)
            }
        }
    }
}

type ResolveTable = Vec<ResolveTableRow>;
type ResolveTableRow = Vec<ResolveEntry>;

impl std::ops::Index<ObjIdx> for ResolveTable {
    type Output = ResolveTableRow;

    fn index(&self, obj_i: ObjIdx) -> &Self::Output {
        &self[obj_i.get()]
    }
}

impl std::ops::IndexMut<ObjIdx> for ResolveTable {
    fn index_mut(&mut self, obj_i: ObjIdx) -> &mut Self::Output {
        &mut self[obj_i.get()]
    }
}

impl std::ops::Index<ObjImportIdx> for ResolveTableRow {
    type Output = ResolveEntry;

    fn index(&self, imp_i: ObjImportIdx) -> &Self::Output {
        &self[imp_i.get()]
    }
}

impl std::ops::IndexMut<ObjImportIdx> for ResolveTableRow {
    fn index_mut(&mut self, imp_i: ObjImportIdx) -> &mut Self::Output {
        &mut self[imp_i.get()]
    }
}

/// 解決中のシンボルテーブルエントリ。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolveEntry {
    addr_size: u8,
    state: ResolveState,
}

/// シンボルの解決状態。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResolveState {
    /// 解決済み。
    Done(i64),
    /// 解決中 (循環参照検出用)。
    Resolving,
    /// 未解決 (`Exports` 内インデックスを保持)。
    Unresolved { export_i: usize },
}
