use std::ops::Deref;

use anyhow::Result;
use litparser::LitParser;
use litparser_derive::LitParser;
use swc_common::sync::Lrc;
use swc_ecma_ast as ast;

use crate::parser::resourceparser::bind::ResourceOrPath;
use crate::parser::resourceparser::bind::{BindData, BindKind};
use crate::parser::resourceparser::paths::PkgPath;
use crate::parser::resourceparser::resource_parser::ResourceParser;
use crate::parser::resources::parseutil::{iter_references, TrackedNames};
use crate::parser::resources::parseutil::{NamedClassResourceOptionalConfig, NamedStaticMethod};
use crate::parser::resources::Resource;
use crate::parser::resources::ResourcePath;
use crate::parser::types::{Generic, Type};
use crate::parser::usageparser::{MethodCall, ResolveUsageData, Usage, UsageExprKind};
use crate::parser::Range;
use crate::span_err::ErrReporter;

#[derive(Debug, Clone)]
pub struct Bucket {
    pub name: String,
    pub doc: Option<String>,
    pub versioned: bool,
}

#[derive(LitParser, Default)]
struct DecodedBucketConfig {
    pub versioned: Option<bool>,
}

pub const OBJECTS_PARSER: ResourceParser = ResourceParser {
    name: "objects",
    interesting_pkgs: &[PkgPath("encore.dev/storage/objects")],

    run: |pass| {
        let names = TrackedNames::new(&[("encore.dev/storage/objects", "Bucket")]);

        let module = pass.module.clone();
        {
            type Res = NamedClassResourceOptionalConfig<DecodedBucketConfig>;
            for r in iter_references::<Res>(&module, &names) {
                let r = r?;
                let cfg = r.config.unwrap_or_default();

                let object = match &r.bind_name {
                    None => None,
                    Some(id) => pass
                        .type_checker
                        .resolve_obj(pass.module.clone(), &ast::Expr::Ident(id.clone())),
                };

                let resource = Resource::Bucket(Lrc::new(Bucket {
                    name: r.resource_name,
                    doc: r.doc_comment,
                    versioned: cfg.versioned.unwrap_or(false),
                }));
                pass.add_resource(resource.clone());
                pass.add_bind(BindData {
                    range: r.range,
                    resource: ResourceOrPath::Resource(resource),
                    object,
                    kind: BindKind::Create,
                    ident: r.bind_name,
                });
            }
        }

        {
            for r in iter_references::<NamedStaticMethod>(&module, &names) {
                let r = r?;
                let object = match &r.bind_name {
                    None => None,
                    Some(id) => pass
                        .type_checker
                        .resolve_obj(pass.module.clone(), &ast::Expr::Ident(id.clone())),
                };

                pass.add_bind(BindData {
                    range: r.range,
                    resource: ResourceOrPath::Path(ResourcePath::Bucket {
                        name: r.resource_name,
                    }),
                    object,
                    kind: BindKind::Reference,
                    ident: r.bind_name,
                });
            }
        }

        Ok(())
    },
};

pub fn resolve_bucket_usage(data: &ResolveUsageData, bucket: Lrc<Bucket>) -> Result<Option<Usage>> {
    Ok(match &data.expr.kind {
        UsageExprKind::MethodCall(call) => {
            if call.method.as_ref() == "ref" {
                let Some(type_args) = call.call.type_args.as_deref() else {
                    call.call
                        .span
                        .err("expected a type argument in call to Bucket.ref");
                    return Ok(None);
                };

                let Some(type_arg) = type_args.params.first() else {
                    call.call
                        .span
                        .err("expected a type argument in call to Bucket.ref");
                    return Ok(None);
                };

                return parse_bucket_ref(data, bucket, call, type_arg);
            }

            let op = match call.method.as_ref() {
                "list" => Operation::ListObjects,
                "exists" | "attrs" => Operation::GetObjectMetadata,
                "upload" => Operation::WriteObject,
                "download" => Operation::ReadObjectContents,
                "remove" => Operation::DeleteObject,
                _ => {
                    call.method.err("unsupported bucket operation");
                    return Ok(None);
                }
            };

            Some(Usage::Bucket(BucketUsage {
                range: data.expr.range,
                bucket,
                ops: vec![op],
            }))
        }

        _ => {
            data.expr
                .range
                .to_span()
                .err("invalid use of bucket resource");
            None
        }
    })
}

fn parse_bucket_ref(
    data: &ResolveUsageData,
    bucket: Lrc<Bucket>,
    _call: &MethodCall,
    type_arg: &ast::TsType,
) -> Result<Option<Usage>> {
    fn process_type(
        data: &ResolveUsageData,
        sp: &swc_common::Span,
        t: &Type,
        depth: usize,
    ) -> Option<Vec<Operation>> {
        if depth > 10 {
            // Prevent infinite recursion.
            return None;
        }

        match t {
            Type::Named(named) => {
                let ops = match named.obj.name.as_deref() {
                    Some("Lister") => vec![Operation::ListObjects],
                    Some("Attrser") => vec![Operation::GetObjectMetadata],
                    Some("Uploader") => vec![Operation::WriteObject],
                    Some("Downloader") => vec![Operation::ReadObjectContents],
                    Some("Remover") => vec![Operation::DeleteObject],
                    _ => {
                        let underlying = data.type_checker.resolve_obj_type(&named.obj);
                        return process_type(data, sp, &underlying, depth + 1);
                    }
                };

                Some(ops)
            }

            Type::Class(cls) => {
                let ops = cls
                    .methods
                    .iter()
                    .filter_map(|method| {
                        let op = match method.as_str() {
                            "list" => Operation::ListObjects,
                            "exists" | "attrs" => Operation::GetObjectMetadata,
                            "upload" => Operation::WriteObject,
                            "download" => Operation::ReadObjectContents,
                            "remove" => Operation::DeleteObject,
                            _ => {
                                // Ignore other methods.
                                return None;
                            }
                        };

                        Some(op)
                    })
                    .collect();
                Some(ops)
            }

            Type::Generic(Generic::Intersection(int)) => {
                let mut result = Vec::new();
                for t in &[&int.x, &int.y] {
                    if let Some(ops) = process_type(data, sp, t, depth + 1) {
                        result.extend(ops);
                    }
                }

                if result.is_empty() {
                    None
                } else {
                    Some(result)
                }
            }

            _ => {
                sp.err(&format!("unsupported bucket permission type {:#?}", t));
                None
            }
        }
    }

    let typ = data
        .type_checker
        .resolve_type(data.module.clone(), type_arg);

    if let Some(ops) = process_type(data, &typ.span(), typ.deref(), 0) {
        Ok(Some(Usage::Bucket(BucketUsage {
            range: data.expr.range,
            bucket,
            ops,
        })))
    } else {
        typ.err("no bucket permissions found in type argument");
        Ok(None)
    }
}

#[derive(Debug)]
pub struct BucketUsage {
    pub range: Range,
    pub bucket: Lrc<Bucket>,
    pub ops: Vec<Operation>,
}

#[derive(Debug)]
pub enum Operation {
    /// Listing objects and accessing their metadata during list operations.
    ListObjects,

    /// Reading the contents of an object.
    ReadObjectContents,

    /// Creating or updating an object, with contents and metadata.
    WriteObject,

    /// Updating the metadata of an object, without reading or writing its contents.
    UpdateObjectMetadata,

    /// Reading the metadata of an object, or checking for its existence.
    GetObjectMetadata,

    /// Deleting an object.
    DeleteObject,
}