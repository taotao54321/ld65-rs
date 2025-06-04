use anyhow::Context as _;
use clap::{builder::NonEmptyStringValueParser, Parser};

use xo65::Xo65;

use ld65::{object::Object, script::LinkScript};

#[derive(Debug, Parser)]
struct Cli {
    /// リンカスクリプトファイル名。
    #[arg(
        required = true,
        short = 'C',
        long = "config",
        value_parser = NonEmptyStringValueParser::new()
    )]
    path_script: String,

    /// メイン出力ファイル名。
    #[arg(
        required = true,
        short = 'o',
        long = "output",
        value_parser = NonEmptyStringValueParser::new()
    )]
    path_out: String,

    /// オブジェクトファイル名のリスト。
    // required = true を付けることで 0 個のケースをエラーにできる
    #[arg(
        required = true,
        value_parser = NonEmptyStringValueParser::new()
    )]
    paths_obj: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let script = std::fs::read_to_string(&cli.path_script)
        .with_context(|| format!("cannot read linker script '{}'", cli.path_script))?;
    let script = LinkScript::load(&script, &cli.path_out)?;

    let objs: Box<_> = cli
        .paths_obj
        .iter()
        .map(|path| {
            std::fs::read(path).with_context(|| format!("cannot read object file '{path}'"))
        })
        .collect::<Result<_, _>>()?;
    let objs: Box<_> = objs
        .iter()
        .enumerate()
        .map(|(i, obj)| {
            let path = &cli.paths_obj[i];
            Xo65::parse(obj)
                .with_context(|| format!("cannot parse object file '{}'", path))
                .map(|xo65| Object::new(path, xo65))
        })
        .collect::<Result<_, _>>()?;

    let outputs = ld65::link::link(&script, &objs);

    for output in outputs.iter() {
        let path = output.path();
        std::fs::write(path, output.body())
            .with_context(|| format!("cannot write output file '{path}'"))?;
    }

    Ok(())
}
