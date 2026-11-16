use base64::{Engine, engine::general_purpose::STANDARD};
use prost::Message;
use prost_types::{DescriptorProto, EnumDescriptorProto, FileDescriptorProto, FileDescriptorSet};
use regex::Regex;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

const ROOTS: &[&str] = &[
    ".bilibili.metadata.Metadata",
    ".bilibili.metadata.device.Device",
    ".bilibili.metadata.fawkes.FawkesReq",
    ".bilibili.metadata.locale.Locale",
    ".bilibili.metadata.locale.LocaleIds",
    ".bilibili.metadata.network.Network",
    ".bilibili.community.service.dm.v1.DmSegMobileReq",
    ".bilibili.community.service.dm.v1.DmViewReq",
];

fn expected_package(name: &str) -> Result<String, String> {
    name.strip_suffix(".proto")
        .map(|s| s.replace('/', "."))
        .ok_or_else(|| format!("invalid proto file name: {name}"))
}

fn messages<'a>(
    items: &'a [DescriptorProto],
    prefix: &str,
    out: &mut BTreeMap<String, &'a DescriptorProto>,
) {
    for item in items {
        let name = format!("{prefix}.{}", item.name());
        out.insert(name.clone(), item);
        messages(&item.nested_type, &name, out);
    }
}

fn definitions(files: &[FileDescriptorProto]) -> BTreeMap<String, usize> {
    let mut result = BTreeMap::new();
    for (index, file) in files.iter().enumerate() {
        let prefix = format!(".{}", file.package());
        let mut all = BTreeMap::new();
        messages(&file.message_type, &prefix, &mut all);
        for (name, message) in all {
            result.insert(name.clone(), index);
            for en in &message.enum_type {
                result.insert(format!("{name}.{}", en.name()), index);
            }
        }
        for en in &file.enum_type {
            result.insert(format!("{prefix}.{}", en.name()), index);
        }
    }
    result
}

fn validate(files: &[FileDescriptorProto]) -> Result<(), String> {
    let known = definitions(files);
    for file in files {
        if file.package() != expected_package(file.name())? {
            return Err(format!(
                "package does not match source path: {}",
                file.name()
            ));
        }
        let mut all = BTreeMap::new();
        messages(
            &file.message_type,
            &format!(".{}", file.package()),
            &mut all,
        );
        for (name, message) in all {
            for field in &message.field {
                if field.label == Some(2) {
                    return Err(format!("required field: {name}.{}", field.name()));
                }
                if let Some(target) = &field.type_name {
                    if !known.contains_key(target) {
                        return Err(format!("unknown type {target} in {name}.{}", field.name()));
                    }
                }
            }
        }
    }
    Ok(())
}

fn parse(source: &str) -> Result<FileDescriptorProto, String> {
    let header =
        Regex::new(r"(?m)^// Generated from ([^\r\n]+\.proto)\.").map_err(|e| e.to_string())?;
    let name = header
        .captures(source)
        .ok_or("missing Generated from header")?[1]
        .to_owned();
    let pattern = Regex::new(r"(?s)/// Descriptor for `([^`]+)`\. Decode as a `google\.protobuf\.(DescriptorProto|EnumDescriptorProto)`\.\s*final[^;]*?\$convert\s*\.\s*base64Decode\((.*?)\);").map_err(|e| e.to_string())?;
    let literal = Regex::new(r"'([^']*)'").map_err(|e| e.to_string())?;
    let mut file = FileDescriptorProto {
        package: Some(expected_package(&name)?),
        name: Some(name),
        syntax: Some("proto3".into()),
        ..Default::default()
    };
    for capture in pattern.captures_iter(source) {
        let encoded: String = literal
            .captures_iter(&capture[3])
            .map(|c| c[1].to_owned())
            .collect();
        let bytes = STANDARD.decode(encoded).map_err(|e| e.to_string())?;
        if &capture[2] == "DescriptorProto" {
            let descriptor =
                DescriptorProto::decode(bytes.as_slice()).map_err(|e| e.to_string())?;
            if descriptor.name() != &capture[1] {
                return Err(format!(
                    "descriptor name mismatch: {} versus {}",
                    descriptor.name(),
                    &capture[1]
                ));
            }
            file.message_type.push(descriptor);
        } else {
            let descriptor =
                EnumDescriptorProto::decode(bytes.as_slice()).map_err(|e| e.to_string())?;
            if descriptor.name() != &capture[1] {
                return Err(format!(
                    "descriptor name mismatch: {} versus {}",
                    descriptor.name(),
                    &capture[1]
                ));
            }
            file.enum_type.push(descriptor);
        }
    }
    if file.message_type.is_empty() && file.enum_type.is_empty() {
        return Err("no descriptors found".into());
    }
    Ok(file)
}

fn read_sources(
    path: &Path,
    files: &mut BTreeMap<String, FileDescriptorProto>,
) -> Result<(), String> {
    for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            read_sources(&path, files)?;
        } else if path.to_string_lossy().ends_with(".pbjson.dart") {
            let file = parse(&fs::read_to_string(&path).map_err(|e| e.to_string())?)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            files.insert(file.name().to_owned(), file);
        }
    }
    Ok(())
}

fn select(
    files: Vec<FileDescriptorProto>,
    roots: &[&str],
) -> Result<Vec<FileDescriptorProto>, String> {
    let known = definitions(&files);
    let mut pending: Vec<String> = roots.iter().map(|s| s.to_string()).collect();
    let mut selected = BTreeSet::new();
    // 选择顶层描述符时保留其嵌套定义；依赖闭包不牵连同文件无关的响应树。
    while let Some(name) = pending.pop() {
        if !selected.insert(name.clone()) {
            continue;
        }
        let index = *known
            .get(&name)
            .ok_or_else(|| format!("unknown type {name}"))?;
        let file = &files[index];
        let prefix = format!(".{}", file.package());
        for message in &file.message_type {
            let top = format!("{prefix}.{}", message.name());
            if name == top || name.starts_with(&format!("{top}.")) {
                if name != top {
                    pending.push(top.clone());
                }
                let mut nested = BTreeMap::new();
                messages(std::slice::from_ref(message), &prefix, &mut nested);
                for (_, message) in nested {
                    pending.extend(message.field.iter().filter_map(|f| f.type_name.clone()));
                }
            }
        }
    }
    let mut result = Vec::new();
    for mut file in files {
        let prefix = format!(".{}", file.package());
        file.message_type
            .retain(|m| selected.contains(&format!("{prefix}.{}", m.name())));
        file.enum_type
            .retain(|e| selected.contains(&format!("{prefix}.{}", e.name())));
        if !file.message_type.is_empty() || !file.enum_type.is_empty() {
            result.push(file);
        }
    }
    validate(&result)?;
    let selected_known = definitions(&result);
    let names: Vec<_> = result.iter().map(|f| f.name().to_owned()).collect();
    for (index, file) in result.iter_mut().enumerate() {
        let mut nested = BTreeMap::new();
        messages(
            &file.message_type,
            &format!(".{}", file.package()),
            &mut nested,
        );
        let dependencies: BTreeSet<_> = nested
            .values()
            .flat_map(|m| &m.field)
            .filter_map(|f| f.type_name.as_ref())
            .filter_map(|n| selected_known.get(n))
            .filter(|i| **i != index)
            .map(|i| names[*i].clone())
            .collect();
        file.dependency = dependencies.into_iter().collect();
    }
    Ok(result)
}

fn generate(repo: &Path, output: &Path) -> Result<(), String> {
    let mut files = BTreeMap::new();
    // 已删除 Dart 子树从机械快照恢复；仍存在的上游描述符优先，避免快照遮住更新。
    read_sources(&repo.join("tool/proto-generator/inputs"), &mut files)?;
    read_sources(&repo.join("lib/grpc/bilibili"), &mut files)?;
    let mut file = select(files.into_values().collect(), ROOTS)?;
    // protobuf.dart 区分未设置与显式零值；prost 默认 proto3 标量会丢掉这一位。
    // synthetic optional 只恢复存在性，不改变字段号、类型或实际 wire 格式。
    fn preserve_presence(message: &mut DescriptorProto) {
        for field in &mut message.field {
            if field.label == Some(1)
                && !matches!(field.r#type, Some(10 | 11))
                && field.oneof_index.is_none()
            {
                field.proto3_optional = Some(true);
                field.oneof_index = Some(message.oneof_decl.len() as i32);
                message.oneof_decl.push(prost_types::OneofDescriptorProto {
                    name: Some(format!("_{}", field.name())),
                    ..Default::default()
                });
            }
        }
        for nested in &mut message.nested_type {
            preserve_presence(nested);
        }
    }
    for descriptor in &mut file {
        for message in &mut descriptor.message_type {
            preserve_presence(message);
        }
    }
    fs::create_dir_all(output).map_err(|e| e.to_string())?;
    prost_build::Config::new()
        .out_dir(output)
        .include_file("mod.rs")
        .btree_map(["."])
        .compile_fds(FileDescriptorSet { file })
        .map_err(|e| e.to_string())
}

fn main() -> Result<(), String> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("rust/crates/bili-grpc/src/generated"));
    generate(&repo, &output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost_types::FieldDescriptorProto;

    fn file() -> FileDescriptorProto {
        FileDescriptorProto {
            name: Some("example/v1.proto".into()),
            package: Some("example.v1".into()),
            message_type: vec![DescriptorProto {
                name: Some("Request".into()),
                field: vec![FieldDescriptorProto {
                    name: Some("value".into()),
                    number: Some(1),
                    label: Some(2),
                    r#type: Some(9),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn generates_identically_twice_without_protoc() {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        generate(&repo, first.path()).unwrap();
        generate(&repo, second.path()).unwrap();
        let names: BTreeSet<_> = fs::read_dir(first.path())
            .unwrap()
            .map(|p| p.unwrap().file_name())
            .collect();
        assert!(names.len() > 1);
        for name in names {
            assert_eq!(
                fs::read(first.path().join(&name)).unwrap(),
                fs::read(second.path().join(name)).unwrap()
            );
        }
        let metadata = fs::read_to_string(first.path().join("bilibili.metadata.rs")).unwrap();
        assert!(metadata.contains("optional"), "显式空串必须保留存在性");
    }

    #[test]
    fn parser_accepts_formatter_line_break_before_base64() {
        let bytes = DescriptorProto {
            name: Some("Empty".into()),
            ..Default::default()
        }
        .encode_to_vec();
        let source = format!(
            "// Generated from example/v1.proto.\n/// Descriptor for `Empty`. Decode as a `google.protobuf.DescriptorProto`.\nfinal bytes = $convert\n .base64Decode('{}');",
            STANDARD.encode(bytes)
        );
        assert_eq!(parse(&source).unwrap().message_type[0].name(), "Empty");
    }

    #[test]
    fn rejects_required_fields() {
        assert!(validate(&[file()]).unwrap_err().contains("required"));
    }

    #[test]
    fn rejects_package_mismatch() {
        let mut file = file();
        file.package = Some("wrong.package".into());
        assert!(validate(&[file]).unwrap_err().contains("package"));
    }

    #[test]
    fn rejects_unknown_types() {
        let mut file = file();
        let field = &mut file.message_type[0].field[0];
        field.label = Some(1);
        field.r#type = Some(11);
        field.type_name = Some(".example.v1.Missing".into());
        assert!(validate(&[file]).unwrap_err().contains("unknown"));
    }
}
