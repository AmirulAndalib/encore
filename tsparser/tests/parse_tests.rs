use std::fs;
use std::path::Path;
use std::rc::Rc;

use anyhow::Result;
use common::js_runtime_path;
use insta::glob;
use swc_common::errors::{Handler, HANDLER};
use swc_common::{Globals, SourceMap, GLOBALS};
use tempdir::TempDir;

use encore_tsparser::builder::Builder;
use encore_tsparser::parser::parser::ParseContext;
use encore_tsparser::{app, builder};

mod common;

#[test]
fn test_parser() {
    env_logger::init();
    glob!("testdata/*.txt", |path| {
        let input = fs::read_to_string(path).unwrap();
        let ar = txtar::from_str(&input);
        let tmp_dir = TempDir::new("parse").unwrap();
        ar.materialize(&tmp_dir).unwrap();
        match parse_txtar(tmp_dir.path()) {
            Ok(_) => {}
            Err(e) => {
                panic!("{:#?}\n{}", e, e.backtrace());
            }
        }
    });
}

fn parse_txtar(app_root: &Path) -> Result<app::AppDesc> {
    let globals = Globals::new();
    let cm: Rc<SourceMap> = Default::default();
    let errs = Rc::new(Handler::with_tty_emitter(
        swc_common::errors::ColorConfig::Auto,
        true,
        false,
        Some(cm.clone()),
    ));

    GLOBALS.set(&globals, || -> Result<app::AppDesc> {
        HANDLER.set(&errs, || -> Result<app::AppDesc> {
            let builder = Builder::new()?;
            let pc = ParseContext::new(
                app_root.to_path_buf(),
                Some(js_runtime_path()),
                cm,
                errs.clone(),
            )?;

            let app = builder::App {
                root: app_root.to_path_buf(),
                platform_id: None,
                local_id: "test".to_string(),
            };
            let pp = builder::ParseParams {
                app: &app,
                pc: &pc,
                working_dir: app_root,
                parse_tests: false,
            };

            builder.parse(&pp).ok_or(anyhow::anyhow!("parse failed"))
        })
    })
}
