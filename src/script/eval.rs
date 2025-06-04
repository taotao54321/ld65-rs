use anyhow::{anyhow, bail, ensure, Context as _};
use indexmap::{indexset, IndexMap, IndexSet};

use crate::index::{MemIdx, OutFileIdx};
use crate::range::NonemptyRange;

use super::{
    ast, LinkScript, LinkScriptMemory, LinkScriptMemoryBuilder, LinkScriptSegment,
    LinkScriptSegmentBuilder, LinkScriptSegmentStart,
};

/// リンカスクリプトの AST を評価し、結果を返す。
pub fn eval(script: &ast::Script, main_outfile: &str) -> anyhow::Result<LinkScript> {
    // 先に重複定義チェックを済ませてしまう。
    check_dup(script)?;

    let mut ctx = EvalContext::new(main_outfile);

    eval_blocks(&mut ctx, &script.blocks)?;

    Ok(ctx.into_script())
}

/// リンカスクリプト内の重複定義チェック。
fn check_dup(script: &ast::Script) -> anyhow::Result<()> {
    // ブロック名に重複があってはならない。
    if let Some(name) = find_dup_str(script.blocks.iter().map(|block| block.name.as_str())) {
        bail!("duplicate block: '{name}'");
    }

    for block in &script.blocks {
        // ブロック内の要素名に重複があってはならない。
        if let Some(name) = find_dup_str(block.elems.iter().map(|elem| elem.name.as_str())) {
            bail!("block '{}': duplicate element: '{name}'", block.name);
        }

        for elem in &block.elems {
            // 要素内の属性キーに重複があってはならない。
            if let Some(key) = find_dup_str(elem.attrs.iter().map(|attr| attr.key.as_str())) {
                bail!(
                    "block '{}': element '{}': duplicate attribute: '{key}'",
                    block.name,
                    elem.name
                );
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
struct EvalContext {
    // 先頭要素はメイン出力ファイル。
    outfiles: IndexSet<String>,
    mems: IndexMap<String, LinkScriptMemory>,
    segs: IndexMap<String, LinkScriptSegment>,
}

impl EvalContext {
    fn new(main_outfile: &str) -> Self {
        Self {
            outfiles: indexset! { main_outfile.to_owned() },
            mems: IndexMap::new(),
            segs: IndexMap::new(),
        }
    }

    fn main_outfile(&self) -> &str {
        self.outfiles.first().unwrap()
    }

    fn into_script(self) -> LinkScript {
        let outfiles: Box<[_]> = self.outfiles.into_iter().collect();
        let mems: Box<_> = self.mems.into_values().collect();
        let segs: Box<_> = self.segs.into_values().collect();

        LinkScript {
            outfiles,
            mems,
            segs,
        }
    }
}

fn eval_blocks(ctx: &mut EvalContext, blocks: &[ast::Block]) -> anyhow::Result<()> {
    for block in blocks {
        eval_block(ctx, block).with_context(|| format!("block '{}' eval error", block.name))?;
    }

    Ok(())
}

fn eval_block(ctx: &mut EvalContext, block: &ast::Block) -> anyhow::Result<()> {
    match block.name.as_str() {
        "memory" => eval_memory(ctx, block),
        "segments" => eval_segments(ctx, block),
        unknown => bail!("unknown block: '{unknown}'"),
    }
}

fn eval_memory(ctx: &mut EvalContext, block: &ast::Block) -> anyhow::Result<()> {
    for elem in &block.elems {
        let mem = eval_memory_elem(ctx, elem)
            .with_context(|| format!("memory '{}' eval error", elem.name))?;
        let old = ctx.mems.insert(elem.name.clone(), mem);
        assert_eq!(old, None); // 重複はないはず
    }

    Ok(())
}

fn eval_memory_elem(
    ctx: &mut EvalContext,
    elem: &ast::Element,
) -> anyhow::Result<LinkScriptMemory> {
    let mut builder = LinkScriptMemoryBuilder::default();
    builder.name(&elem.name);

    let mut start = None::<usize>;
    let mut size = None::<usize>;

    for attr in &elem.attrs {
        let ast::Attribute { key, value } = attr;
        match key.as_str() {
            "start" => {
                let value = value
                    .as_uint()
                    .ok_or_else(|| anyhow!("invalid value for memory start address: {value:?}"))?;
                start = Some(value as usize);
            }
            "size" => {
                let value = value
                    .as_uint()
                    .ok_or_else(|| anyhow!("invalid value for memory size: {value:?}"))?;
                size = Some(value as usize);
            }
            "type" => {
                // 文脈依存キーワード。小文字に統一する。
                let value = value
                    .as_ident()
                    .ok_or_else(|| anyhow!("invalid value for memory type: {value:?}"))?
                    .to_ascii_lowercase();
                match value.as_str() {
                    // ro, rw は単に無視する。
                    "ro" | "rw" => {}
                    // その他の値は無効 (特に、memory に zp/bss を指定することはできない)。
                    invalid => bail!("invalid value for memory type: {invalid}"),
                }
            }
            "fill" => {
                let value = value.as_bool().ok_or_else(|| {
                    anyhow!("invalid value for memory attribute 'fill': {value:?}")
                })?;
                builder.filled(value);
            }
            "fillval" => {
                let value = value
                    .as_uint()
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or_else(|| {
                        anyhow!("invalid value for memory attribute 'fillval': {value:?}")
                    })?;
                builder.fill_byte(value);
            }
            "file" => {
                let value = value.as_string().ok_or_else(|| {
                    anyhow!("invalid value for memory attribute 'file': {value:?}")
                })?;
                let outfile = value.format(ctx.main_outfile());
                ensure!(!outfile.is_empty(), "output filename is empty");
                let (outfile_i, _) = ctx.outfiles.insert_full(outfile);
                builder.outfile_i(OutFileIdx::new(outfile_i));
            }
            key @ ("bank" | "define") => bail!("attribute '{key}' is not supported"),
            unknown => bail!("unknown memory attribute: '{unknown}'"),
        }
    }

    let Some(start) = start else {
        bail!("start address not found");
    };
    let Some(size) = size else {
        bail!("size not found");
    };
    ensure!(size > 0, "size must be positive");
    builder.range(NonemptyRange::from_start_len(start, size));

    builder.build().context("failed to build memory")
}

fn eval_segments(ctx: &mut EvalContext, block: &ast::Block) -> anyhow::Result<()> {
    for elem in &block.elems {
        let seg = eval_segments_elem(ctx, elem)
            .with_context(|| format!("segment '{}' eval error", elem.name))?;
        // 開始アドレスが絶対アドレスで指定されている場合、それはメモリ領域内になければならない。
        if let LinkScriptSegmentStart::Addr(start) = seg.start {
            let mem = ctx.mems.get_index(seg.mem_i.get()).unwrap().1;
            ensure!(
                mem.range.contains(start),
                "segment '{}': start address is out of memory '{}'",
                seg.name,
                mem.name
            );
        }
        let old = ctx.segs.insert(elem.name.clone(), seg);
        assert_eq!(old, None); // 重複はないはず
    }

    Ok(())
}

fn eval_segments_elem(
    ctx: &mut EvalContext,
    elem: &ast::Element,
) -> anyhow::Result<LinkScriptSegment> {
    let mut builder = LinkScriptSegmentBuilder::default();
    builder.name(&elem.name);

    let mut start_specified = false;

    for attr in &elem.attrs {
        let ast::Attribute { key, value } = attr;
        match key.as_str() {
            "load" => {
                let value = value.as_ident().ok_or_else(|| {
                    anyhow!("invalid value for segment attribute 'load': {value:?}")
                })?;
                let mem_i = ctx
                    .mems
                    .get_index_of(value)
                    .ok_or_else(|| anyhow!("unknown memory: '{value}'"))?;
                builder.mem_i(MemIdx::new(mem_i));
            }
            "type" => {
                // 文脈依存キーワード。小文字に統一する。
                let value = value
                    .as_ident()
                    .ok_or_else(|| anyhow!("invalid value for segment type: {value:?}"))?
                    .to_ascii_lowercase();
                match value.as_str() {
                    // ro, rw は単に無視する。
                    "ro" | "rw" => {}
                    // zp と bss は実質同じ (オリジナルでは o65 形式への出力時のみ違いがあるらしい)。
                    "zp" | "bss" => {
                        builder.bss(true);
                    }
                    "overwrite" => bail!("segment type 'overwrite' is not supported"),
                    invalid => bail!("invalid segment type: '{invalid}'"),
                }
            }
            "start" => {
                if start_specified {
                    bail!("attribute 'start'/'align' appeared twice");
                }
                let value = value
                    .as_uint()
                    .ok_or_else(|| anyhow!("invalid value for segment start address: {value:?}"))?;
                builder.start(LinkScriptSegmentStart::Addr(value as usize));
                start_specified = true;
            }
            "align" => {
                if start_specified {
                    bail!("attribute 'start'/'align' appeared twice");
                }
                let value = value
                    .as_uint()
                    .ok_or_else(|| anyhow!("invalid value for segment alignment: {value:?}"))?;
                builder.start(LinkScriptSegmentStart::Align(value as usize));
                start_specified = true;
            }
            "fillval" => {
                let value = value
                    .as_uint()
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or_else(|| {
                        anyhow!("invalid value for segment attribute 'fillval': {value:?}")
                    })?;
                builder.fill_byte(value);
            }
            key @ ("align_load" | "define" | "offset" | "optional" | "run") => {
                bail!("attribute '{key}' is not supported")
            }
            unknown => bail!("unknown segment attribute: '{unknown}'"),
        }
    }

    builder.build().context("failed to build segment")
}

/// 文字列のリストから重複した要素を探す。
fn find_dup_str<'a, I>(it: I) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut xs = std::collections::HashSet::<&str>::new();

    it.into_iter().find(|&x| !xs.insert(x))
}
