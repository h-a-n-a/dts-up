use log::debug;
use std::path::Path;
use std::sync::Arc;

use once_cell::sync::Lazy;
use smol_str::SmolStr;
use tokio::fs::File;

use swc::{config::IsModule, Compiler};
use swc_common::{errors, FileName, FilePathMapping, SourceMap};
use swc_ecma_parser::Syntax;
use tokio::io::AsyncReadExt;

use crate::result::{DtsupErrorType, Error};

pub(crate) static COMPILER: Lazy<Arc<Compiler>> = Lazy::new(|| {
  let source_map = Arc::new(SourceMap::new(FilePathMapping::empty()));
  Arc::new(Compiler::new(source_map))
});

pub async fn parse_file(file_name: SmolStr) -> Result<swc_ecma_ast::Module, Error> {
  let compiler = COMPILER.clone();

  let is_dts = file_name.ends_with(".d.ts");

  if is_dts {
    let path = Path::new(file_name.as_str());
    let mut file = File::open(path).await.unwrap();
    debug!(
      "[Parse]: parsing file {}",
      path.to_string_lossy().to_string()
    );
    let mut buf: Vec<u8> = Default::default();
    file.read_to_end(&mut buf).await;

    let fm = compiler.cm.new_source_file(
      FileName::Custom(file_name.into()),
      String::from_utf8(buf).unwrap_or_default(),
    );

    let syntax = Syntax::Typescript(swc_ecma_parser::TsConfig {
      dts: true,
      ..Default::default()
    });

    swc::try_with_handler(
      Arc::new(SourceMap::new(FilePathMapping::empty())),
      Default::default(),
      |handler| {
        compiler.parse_js(
          fm,
          handler,
          swc_ecma_ast::EsVersion::Es2022,
          syntax,
          IsModule::Bool(true),
          None,
        )
      },
    )
    .and_then(|program| Ok(program.expect_module()))
    .or_else(|error| {
      Err(Error::new_with_reason(
        DtsupErrorType::ParseFileError,
        error.to_string().as_str(),
      ))
    })
  } else {
    let reason = format!(
      "Unable to parse non TypeScript declaration file {}",
      file_name,
    );
    Err(Error::new_with_reason(
      DtsupErrorType::ParseFileError,
      reason.as_str(),
    ))
  }
}
