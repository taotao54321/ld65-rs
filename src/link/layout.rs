use crate::index::{MemIdx, OutFileIdx, SectIdx, SegIdx};
use crate::object::Object;
use crate::range::NonemptyRange;
use crate::script::{LinkScript, LinkScriptSegmentStart};

use super::graph::LinkGraph;

/// リンクに関与する要素のレイアウトを保持する。
#[derive(Debug)]
pub struct LinkLayout {
    files: Box<[LinkLayoutFile]>,
    mems: Box<[LinkLayoutMemory]>,
    segs: Box<[LinkLayoutSegment]>,
    sects: Box<[LinkLayoutSection]>,
}

impl LinkLayout {
    pub fn file(&self, file_i: OutFileIdx) -> &LinkLayoutFile {
        &self.files[file_i.get()]
    }

    pub fn memory(&self, mem_i: MemIdx) -> &LinkLayoutMemory {
        &self.mems[mem_i.get()]
    }

    pub fn segment(&self, seg_i: SegIdx) -> &LinkLayoutSegment {
        &self.segs[seg_i.get()]
    }

    pub fn section(&self, sect_i: SectIdx) -> &LinkLayoutSection {
        &self.sects[sect_i.get()]
    }

    pub fn new(script: &LinkScript, objs: &[Object], graph: &LinkGraph) -> Self {
        let mut files = vec![None::<LinkLayoutFile>; graph.file_count()];
        let mut mems = vec![None::<LinkLayoutMemory>; graph.mem_count()];
        let mut segs = vec![None::<LinkLayoutSegment>; graph.seg_count()];
        let mut sects = vec![None::<LinkLayoutSection>; graph.sect_count()];

        // 各ファイルを根とする木を辿り、レイアウトを決定する。
        for file_i in graph.files() {
            let mut file_off = 0;

            for mem_i in graph.file_to_mems(file_i) {
                let script_mem = script.memory(mem_i);
                let mut addr = script_mem.start();
                let mut layout_mem = LinkLayoutMemory {
                    file_off,
                    range: script_mem.range(),
                    output_len: 0, // 未計算
                    filled: script_mem.is_filled(),
                    fill_byte: script_mem.fill_byte(),
                };

                for seg_i in graph.mem_to_segs(mem_i) {
                    let script_seg = script.segment(seg_i);
                    let bss = script_seg.is_bss();
                    // セグメントの開始アドレスを決定。
                    match script_seg.start() {
                        LinkScriptSegmentStart::Unspecified => {}
                        LinkScriptSegmentStart::Addr(start) => {
                            // 前のセグメントと重なってはならない。
                            assert!(
                                addr <= start,
                                "segment '{}' overwrites another segment",
                                graph.seg_name(seg_i)
                            );
                            addr = start;
                        }
                        LinkScriptSegmentStart::Align(align) => {
                            assert_eq!(
                                align,
                                1,
                                "segment '{}': alignment is not supported",
                                graph.seg_name(seg_i)
                            );
                        }
                    }
                    let mut layout_seg = LinkLayoutSegment {
                        start: addr,
                        output_len: 0, // 未計算
                        fill_byte: script_seg.fill_byte(),
                    };

                    for sect_i in graph.seg_to_sects(seg_i) {
                        let (obj_i, obj_sect_i) = graph.sect_to_obj_sect(sect_i);
                        let obj = &objs[obj_i.get()];
                        let obj_sect = obj.section(obj_sect_i);
                        assert_eq!(
                            obj_sect.align(),
                            1,
                            "'{}': section {obj_sect_i}: alignment is not supported",
                            obj.name()
                        );

                        // NOTE: BSS の場合、実際の出力サイズは 0 (アドレス加算のみ行うことになる)。
                        let sect_len = obj_sect.len() as usize;
                        let output_len = if bss { 0 } else { sect_len };
                        let layout_sect = LinkLayoutSection {
                            start: addr,
                            output_len,
                        };
                        sects[sect_i.get()] = Some(layout_sect);

                        layout_seg.output_len += output_len;
                        layout_mem.output_len += output_len;

                        assert!(
                            layout_mem.output_len <= script_mem.len(),
                            "memory '{}' overflows",
                            graph.mem_name(mem_i)
                        );

                        addr += sect_len;
                    }

                    segs[seg_i.get()] = Some(layout_seg);
                }

                if layout_mem.filled {
                    layout_mem.output_len = script_mem.len();
                }

                file_off += layout_mem.output_len;

                mems[mem_i.get()] = Some(layout_mem);
            }

            files[file_i.get()] = Some(LinkLayoutFile { len: file_off });
        }

        let files: Box<[_]> = files.into_iter().map(Option::unwrap).collect();
        let mems: Box<[_]> = mems.into_iter().map(Option::unwrap).collect();
        let segs: Box<[_]> = segs.into_iter().map(Option::unwrap).collect();
        let sects: Box<[_]> = sects.into_iter().map(Option::unwrap).collect();

        Self {
            files,
            mems,
            segs,
            sects,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkLayoutFile {
    /// 実際の出力ファイルサイズ (0 のことも一応ある)。
    len: usize,
}

impl LinkLayoutFile {
    pub fn len(&self) -> usize {
        self.len
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkLayoutMemory {
    /// 出力ファイル内オフセット。
    file_off: usize,
    /// アドレス範囲 (リンカスクリプトで指定されたもの)。
    range: NonemptyRange,
    /// 実際にファイルへ出力されるサイズ (0 のこともある)。
    output_len: usize,
    filled: bool,
    fill_byte: u8,
}

impl LinkLayoutMemory {
    pub fn file_offset(&self) -> usize {
        self.file_off
    }

    #[allow(dead_code)]
    pub fn range(&self) -> NonemptyRange {
        self.range
    }

    pub fn start(&self) -> usize {
        self.range.min()
    }

    pub fn output_len(&self) -> usize {
        self.output_len
    }

    pub fn output_is_empty(&self) -> bool {
        self.output_len == 0
    }

    #[allow(dead_code)]
    pub fn is_filled(&self) -> bool {
        self.filled
    }

    pub fn fill_byte(&self) -> u8 {
        self.fill_byte
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkLayoutSegment {
    /// 開始アドレス。
    start: usize,
    /// 実際にファイルへ出力されるサイズ (セクション間のパディングなども含む。0 のこともある)。
    output_len: usize,
    fill_byte: Option<u8>,
}

impl LinkLayoutSegment {
    pub fn start(&self) -> usize {
        self.start
    }

    pub fn output_len(&self) -> usize {
        self.output_len
    }

    pub fn output_is_empty(&self) -> bool {
        self.output_len == 0
    }

    pub fn fill_byte(&self) -> Option<u8> {
        self.fill_byte
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkLayoutSection {
    /// 開始アドレス。
    start: usize,
    /// 実際にファイルへ出力されるサイズ (オブジェクトファイル内の値と同じ。0 のこともある)。
    output_len: usize,
}

impl LinkLayoutSection {
    pub fn start(&self) -> usize {
        self.start
    }

    pub fn output_len(&self) -> usize {
        self.output_len
    }

    pub fn output_is_empty(&self) -> bool {
        self.output_len == 0
    }
}
