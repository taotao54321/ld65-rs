//! リンカスクリプト関連。

use anyhow::Context as _;

use crate::index::{MemIdx, OutFileIdx, SegIdx};
use crate::range::NonemptyRange;

mod ast;
mod eval;
mod parse;

/// 評価済みのリンカスクリプト。
#[derive(Debug)]
pub struct LinkScript {
    outfiles: Box<[String]>,
    mems: Box<[LinkScriptMemory]>,
    segs: Box<[LinkScriptSegment]>,
}

impl LinkScript {
    pub fn outfile_count(&self) -> usize {
        self.outfiles.len()
    }

    pub fn iter_outfiles(
        &self,
    ) -> impl ExactSizeIterator<Item = &str> + std::iter::FusedIterator + Clone {
        self.outfiles.iter().map(String::as_str)
    }

    pub fn enumerate_outfiles(
        &self,
    ) -> impl ExactSizeIterator<Item = (OutFileIdx, &str)> + std::iter::FusedIterator + Clone {
        self.iter_outfiles()
            .enumerate()
            .map(|(i, x)| (OutFileIdx::new(i), x))
    }

    pub fn outfile(&self, outfile_i: OutFileIdx) -> &str {
        &self.outfiles[outfile_i.get()]
    }

    pub fn memory_count(&self) -> usize {
        self.mems.len()
    }

    pub fn iter_memorys(
        &self,
    ) -> impl ExactSizeIterator<Item = &LinkScriptMemory> + std::iter::FusedIterator + Clone {
        self.mems.iter()
    }

    pub fn enumerate_memorys(
        &self,
    ) -> impl ExactSizeIterator<Item = (MemIdx, &LinkScriptMemory)> + std::iter::FusedIterator + Clone
    {
        self.iter_memorys()
            .enumerate()
            .map(|(i, x)| (MemIdx::new(i), x))
    }

    pub fn memory(&self, mem_i: MemIdx) -> &LinkScriptMemory {
        &self.mems[mem_i.get()]
    }

    pub fn segment_count(&self) -> usize {
        self.segs.len()
    }

    pub fn iter_segments(
        &self,
    ) -> impl ExactSizeIterator<Item = &LinkScriptSegment> + std::iter::FusedIterator + Clone {
        self.segs.iter()
    }

    pub fn enumerate_segments(
        &self,
    ) -> impl ExactSizeIterator<Item = (SegIdx, &LinkScriptSegment)> + std::iter::FusedIterator + Clone
    {
        self.iter_segments()
            .enumerate()
            .map(|(i, x)| (SegIdx::new(i), x))
    }

    pub fn segment(&self, seg_i: SegIdx) -> &LinkScriptSegment {
        &self.segs[seg_i.get()]
    }

    pub fn load(script: &str, main_outfile: &str) -> anyhow::Result<Self> {
        let script = self::parse::parse(script).context("linker script parse error")?;
        let script = self::eval::eval(&script, main_outfile).context("linker script eval error")?;

        Ok(script)
    }
}

/// リンカスクリプトで定義されたメモリ領域。
#[derive(Debug, Eq, PartialEq, derive_builder::Builder)]
pub struct LinkScriptMemory {
    #[builder(setter(into))]
    name: String,
    range: NonemptyRange,
    #[builder(default = false)]
    filled: bool,
    #[builder(default = 0)]
    fill_byte: u8,
    // file 属性がない場合、メインの出力ファイルを指す。
    #[builder(default = OutFileIdx::new(0))]
    outfile_i: OutFileIdx,
}

impl LinkScriptMemory {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn range(&self) -> NonemptyRange {
        self.range
    }

    pub fn start(&self) -> usize {
        self.range.min()
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.range.len()
    }

    pub fn is_filled(&self) -> bool {
        self.filled
    }

    pub fn fill_byte(&self) -> u8 {
        self.fill_byte
    }

    pub fn outfile_idx(&self) -> OutFileIdx {
        self.outfile_i
    }
}

/// リンカスクリプトで定義されたセグメント。
#[derive(Debug, Eq, PartialEq, derive_builder::Builder)]
pub struct LinkScriptSegment {
    #[builder(setter(into))]
    name: String,
    #[builder(default = LinkScriptSegmentStart::Unspecified)]
    start: LinkScriptSegmentStart,
    #[builder(default = false)]
    bss: bool,
    #[builder(default = None, setter(strip_option))]
    fill_byte: Option<u8>,
    mem_i: MemIdx,
}

impl LinkScriptSegment {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn start(&self) -> LinkScriptSegmentStart {
        self.start
    }

    pub fn is_bss(&self) -> bool {
        self.bss
    }

    pub fn fill_byte(&self) -> Option<u8> {
        self.fill_byte
    }

    pub fn memory_idx(&self) -> MemIdx {
        self.mem_i
    }
}

/// リンカスクリプトで定義されたセグメントの開始アドレス指定。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkScriptSegmentStart {
    /// 開始アドレス指定なし。
    Unspecified,
    /// 絶対アドレス指定。ロード対象メモリ領域の範囲内であることが保証される。
    Addr(usize),
    /// アラインメント指定。
    Align(usize),
}
