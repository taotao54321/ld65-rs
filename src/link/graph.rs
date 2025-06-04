use crate::index::{MemIdx, ObjIdx, ObjSectIdx, OutFileIdx, SectIdx, SegIdx};
use crate::link::LinkScript;
use crate::object::Object;

type FileToMems = Box<[Box<[MemIdx]>]>;
type MemToSegs = Box<[Box<[SegIdx]>]>;
type SegToSects = Box<[Box<[SectIdx]>]>;
type ObjToSects = Box<[Box<[SectIdx]>]>;

type MemToFile = Box<[OutFileIdx]>;
type SegToMem = Box<[MemIdx]>;
type SectToSeg = Box<[SegIdx]>;

type ObjSectToSect = Box<[Box<[Option<SectIdx>]>]>;
type SectToObjSect = Box<[(ObjIdx, ObjSectIdx)]>;

/// リンクに関与する要素 (ファイル、メモリ領域、セグメント...) 間の関係を保持する。
///
/// ついでに要素の名前もここに持つ。
#[derive(Debug)]
pub struct LinkGraph {
    file_to_mems: FileToMems,
    mem_to_segs: MemToSegs,
    seg_to_sects: SegToSects,
    #[allow(dead_code)]
    obj_to_sects: ObjToSects,

    #[allow(dead_code)]
    mem_to_file: MemToFile,
    #[allow(dead_code)]
    seg_to_mem: SegToMem,
    sect_to_seg: SectToSeg,

    obj_sect_to_sect: ObjSectToSect,
    sect_to_obj_sect: SectToObjSect,

    file_names: Box<[String]>,
    mem_names: Box<[String]>,
    seg_names: Box<[String]>,
}

impl LinkGraph {
    pub fn file_count(&self) -> usize {
        self.file_names.len()
    }

    pub fn mem_count(&self) -> usize {
        self.mem_names.len()
    }

    pub fn seg_count(&self) -> usize {
        self.seg_names.len()
    }

    #[allow(dead_code)]
    pub fn obj_count(&self) -> usize {
        self.obj_to_sects.len()
    }

    pub fn sect_count(&self) -> usize {
        self.sect_to_seg.len()
    }

    pub fn file_name(&self, file_i: OutFileIdx) -> &str {
        &self.file_names[file_i.get()]
    }

    pub fn mem_name(&self, mem_i: MemIdx) -> &str {
        &self.mem_names[mem_i.get()]
    }

    pub fn seg_name(&self, seg_i: SegIdx) -> &str {
        &self.seg_names[seg_i.get()]
    }

    pub fn files(
        &self,
    ) -> impl ExactSizeIterator<Item = OutFileIdx> + std::iter::FusedIterator + Clone {
        (0..self.file_count()).map(OutFileIdx::new)
    }

    pub fn file_to_mems(
        &self,
        file_i: OutFileIdx,
    ) -> impl ExactSizeIterator<Item = MemIdx> + std::iter::FusedIterator + Clone {
        self.file_to_mems[file_i.get()].iter().copied()
    }

    pub fn mem_to_segs(
        &self,
        mem_i: MemIdx,
    ) -> impl ExactSizeIterator<Item = SegIdx> + std::iter::FusedIterator + Clone {
        self.mem_to_segs[mem_i.get()].iter().copied()
    }

    pub fn seg_to_sects(
        &self,
        seg_i: SegIdx,
    ) -> impl ExactSizeIterator<Item = SectIdx> + std::iter::FusedIterator + Clone {
        self.seg_to_sects[seg_i.get()].iter().copied()
    }

    #[allow(dead_code)]
    pub fn obj_to_sects(
        &self,
        obj_i: ObjIdx,
    ) -> impl ExactSizeIterator<Item = SectIdx> + std::iter::FusedIterator + Clone {
        self.obj_to_sects[obj_i.get()].iter().copied()
    }

    #[allow(dead_code)]
    pub fn mem_to_file(&self, mem_i: MemIdx) -> OutFileIdx {
        self.mem_to_file[mem_i.get()]
    }

    #[allow(dead_code)]
    pub fn seg_to_mem(&self, seg_i: SegIdx) -> MemIdx {
        self.seg_to_mem[seg_i.get()]
    }

    #[allow(dead_code)]
    pub fn sect_to_seg(&self, sect_i: SectIdx) -> SegIdx {
        self.sect_to_seg[sect_i.get()]
    }

    pub fn obj_sect_to_sect(&self, obj_i: ObjIdx, obj_sect_i: ObjSectIdx) -> Option<SectIdx> {
        self.obj_sect_to_sect[obj_i.get()][obj_sect_i.get()]
    }

    pub fn sect_to_obj_sect(&self, sect_i: SectIdx) -> (ObjIdx, ObjSectIdx) {
        self.sect_to_obj_sect[sect_i.get()]
    }

    pub fn new(script: &LinkScript, objs: &[Object]) -> Self {
        let (file_to_mems, mem_to_file) = Self::build_file_mem(script);
        let (mem_to_segs, seg_to_mem) = Self::build_mem_seg(script);
        let (seg_to_sects, obj_to_sects, sect_to_seg, obj_sect_to_sect, sect_to_obj_sect) =
            Self::build_seg_obj_sect(script, objs);

        let file_names: Box<[_]> = script.iter_outfiles().map(str::to_owned).collect();
        let mem_names: Box<[_]> = script
            .iter_memorys()
            .map(|mem| mem.name().to_owned())
            .collect();
        let seg_names: Box<[_]> = script
            .iter_segments()
            .map(|seg| seg.name().to_owned())
            .collect();

        Self {
            file_to_mems,
            mem_to_segs,
            seg_to_sects,
            obj_to_sects,

            mem_to_file,
            seg_to_mem,
            sect_to_seg,

            obj_sect_to_sect,
            sect_to_obj_sect,

            file_names,
            mem_names,
            seg_names,
        }
    }

    pub fn build_file_mem(script: &LinkScript) -> (FileToMems, MemToFile) {
        let mut file_to_mems = vec![Vec::<MemIdx>::new(); script.outfile_count()];
        let mut mem_to_file = Vec::<OutFileIdx>::with_capacity(script.memory_count());

        for (mem_i, mem) in script.enumerate_memorys() {
            let file_i = mem.outfile_idx();
            file_to_mems[file_i.get()].push(mem_i);
            mem_to_file.push(file_i);
        }

        let file_to_mems = vecvec_to_boxbox(file_to_mems);
        let mem_to_file = mem_to_file.into_boxed_slice();

        (file_to_mems, mem_to_file)
    }

    pub fn build_mem_seg(script: &LinkScript) -> (MemToSegs, SegToMem) {
        let mut mem_to_segs = vec![Vec::<SegIdx>::new(); script.memory_count()];
        let mut seg_to_mem = Vec::<MemIdx>::with_capacity(script.segment_count());

        for (seg_i, seg) in script.enumerate_segments() {
            let mem_i = seg.memory_idx();
            mem_to_segs[mem_i.get()].push(seg_i);
            seg_to_mem.push(mem_i);
        }

        let mem_to_segs = vecvec_to_boxbox(mem_to_segs);
        let seg_to_mem = seg_to_mem.into_boxed_slice();

        (mem_to_segs, seg_to_mem)
    }

    pub fn build_seg_obj_sect(
        script: &LinkScript,
        objs: &[Object],
    ) -> (
        SegToSects,
        ObjToSects,
        SectToSeg,
        ObjSectToSect,
        SectToObjSect,
    ) {
        // ca65 がデフォルトで出力するセグメント名。
        const PREDEF_SEG_NAMES: &[&str] = &["BSS", "CODE", "DATA", "NULL", "RODATA", "ZEROPAGE"];

        let mut seg_to_sects = vec![Vec::<SectIdx>::new(); script.segment_count()];
        let mut obj_to_sects = vec![Vec::<SectIdx>::new(); objs.len()];
        let mut sect_to_seg = Vec::<SegIdx>::new();
        let mut obj_sect_to_sect = Vec::<Vec<Option<SectIdx>>>::with_capacity(objs.len());
        let mut sect_to_obj_sect = Vec::<(ObjIdx, ObjSectIdx)>::new();

        let seg_name_to_idx: std::collections::HashMap<&str, SegIdx> = script
            .enumerate_segments()
            .map(|(i, seg)| (seg.name(), i))
            .collect();

        let mut sect_i = SectIdx::new(0);

        // 各オブジェクトファイルの各セクションについて見ていく。
        for (obj_i, obj) in objs.iter().enumerate() {
            let obj_i = ObjIdx::new(obj_i);
            let mut obj_sect_to_sect_row =
                Vec::<Option<SectIdx>>::with_capacity(obj.xo65().section_table().count());

            for (obj_sect_i, obj_sect) in obj.enumerate_sections() {
                // このセクションが属するセグメントを得る (セグメント名から逆引き)。
                // このとき、ca65 がデフォルトで生成するセグメント (CODE など) は以下のように扱う:
                //
                // * リンカスクリプトに記述があれば一般のセグメントと同様に扱う。
                // * リンカスクリプトに記述がなく、かつサイズが 0 ならば単に無視する。
                // * リンカスクリプトに記述がなく、かつサイズが 0 でなければエラーとする。
                let seg_name = obj.query_segment_name(obj_sect_i);
                let seg_i = if let Some(&seg_i) = seg_name_to_idx.get(seg_name) {
                    seg_i
                } else if PREDEF_SEG_NAMES.contains(&seg_name) {
                    if obj_sect.is_empty() {
                        obj_sect_to_sect_row.push(None);
                        continue;
                    } else {
                        panic!("'{}': cannot handle segment '{seg_name}'", obj.name());
                    }
                } else {
                    panic!("'{}': unknown segment: '{seg_name}'", obj.name());
                };

                seg_to_sects[seg_i.get()].push(sect_i);
                obj_to_sects[obj_i.get()].push(sect_i);
                sect_to_seg.push(seg_i);
                obj_sect_to_sect_row.push(Some(sect_i));
                sect_to_obj_sect.push((obj_i, obj_sect_i));

                sect_i = SectIdx::new(sect_i.get() + 1);
            }

            obj_sect_to_sect.push(obj_sect_to_sect_row);
        }

        let seg_to_sects = vecvec_to_boxbox(seg_to_sects);
        let obj_to_sects = vecvec_to_boxbox(obj_to_sects);
        let sect_to_seg = sect_to_seg.into_boxed_slice();
        let obj_sect_to_sect = vecvec_to_boxbox(obj_sect_to_sect);
        let sect_to_obj_sect = sect_to_obj_sect.into_boxed_slice();

        (
            seg_to_sects,
            obj_to_sects,
            sect_to_seg,
            obj_sect_to_sect,
            sect_to_obj_sect,
        )
    }
}

fn vecvec_to_boxbox<T>(vv: Vec<Vec<T>>) -> Box<[Box<[T]>]> {
    vv.into_iter().map(Vec::into_boxed_slice).collect()
}
