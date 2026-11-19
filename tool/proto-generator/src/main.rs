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
    ".bilibili.app.interfaces.v1.SearchArchiveReq",
    ".bilibili.main.community.reply.v1.MainListReq",
    ".bilibili.main.community.reply.v1.MainListReply",
    ".bilibili.main.community.reply.v1.DetailListReq",
    ".bilibili.main.community.reply.v1.DetailListReply",
    ".bilibili.main.community.reply.v1.FoldListReq",
    ".bilibili.main.community.reply.v1.FoldListResp",
    ".bilibili.main.community.reply.v1.DialogListReq",
    ".bilibili.main.community.reply.v1.DialogListReply",
    ".bilibili.main.community.reply.v1.SearchItemReq",
    ".bilibili.main.community.reply.v1.SearchItemReply",
    ".bilibili.main.community.reply.v1.TranslateReplyReq",
    ".bilibili.main.community.reply.v1.TranslateReplyResp",
];

/// 每条消息**只保留界面真的读的那些字段**，写成
/// `("<消息全限定名>", &["字段名", …])`。没列在这里的消息整条保留。
///
/// ## 为什么要有这个
///
/// ADR-012 决策四说「按切片生成，不一次性生成整棵树」，并给了一条硬
/// 线：单片体积超过 +1 MiB 就回到「只生成用到的那些」重新评估。评论这
/// 一片第一次落地时实测 **+1.41 MiB**（`libbridge_flutter.dylib`，
/// macOS arm64 release），超了。超出的来源不是消息条数本身，而是几个
/// 字段把**整棵别的树**拖了进来：
///
/// - `MemberV2` 的 `basic`/`official`/`vip`/`medal`/`nft`/`senior`/
///   `contractor`/`user_sailing` → `dagw/component/avatar`（694 行）、
///   `vas/garb`、`account/service`；界面只读 `garb`；
/// - `Content.rich_text` 的 opus 分支 → `app/dynamic/v2`（5 500 行），
///   界面只读 `opus_id`；
/// - `MainListReply` 的活动、抽奖、问卷、运营位等十几棵子树：一个
///   消费者都没有。
///
/// prost 对没生成的字段按未知字段跳过，**解码不受影响**。
///
/// ## 维护约定
///
/// 留一个字段就要能说出谁读它；去掉一个就要能说出答案是「没有」。
/// 列进来的字段一旦在上游消失就**失败**（见 [`keep_only`]）——留着死
/// 规则，下次上游新加的同名字段会被悄悄放行或剪掉，两种都不是我们想要
/// 的。要显示新字段时在这里加一行即可，解析层加字段向后兼容。
const KEEP: &[(&str, &[&str])] = &[
    // ── 列表响应：只有分页、评论本体、评论区状态与两张卡有消费者 ──
    (
        ".bilibili.main.community.reply.v1.MainListReply",
        &[
            "cursor",
            "replies",
            "subject_control",
            "up_top",
            "pagination_reply",
            "vote_card",
            "mixed_cards",
        ],
    ),
    (
        ".bilibili.main.community.reply.v1.DetailListReply",
        &[
            "cursor",
            "subject_control",
            "root",
            "pagination_reply",
            "mixed_cards",
        ],
    ),
    (
        ".bilibili.main.community.reply.v1.DialogListReply",
        &["cursor", "subject_control", "replies", "pagination_reply"],
    ),
    (
        ".bilibili.main.community.reply.v1.FoldListResp",
        &["replies", "pagination_reply", "title", "subject_control"],
    ),
    (
        ".bilibili.main.community.reply.v1.SearchItemReply",
        &["cursor", "items"],
    ),
    // ── 混合卡：只认折叠卡，热点与问卷没有消费者 ──
    (
        ".bilibili.main.community.reply.v1.MixedCard",
        &["type", "fold"],
    ),
    (
        ".bilibili.main.community.reply.v1.SearchItem",
        &["url", "video", "article"],
    ),
    // ── 评论本体 ──
    (
        ".bilibili.main.community.reply.v1.ReplyInfo",
        &[
            "replies",
            "id",
            "oid",
            "type",
            "mid",
            "root",
            "parent",
            "dialog",
            "like",
            "ctime",
            "count",
            "content",
            "member",
            "reply_control",
            "member_v2",
            "translated_content",
            "parent_reply_member",
        ],
    ),
    (
        ".bilibili.main.community.reply.v1.Content",
        &[
            "message",
            "members",
            "emotes",
            "topics",
            "urls",
            "vote",
            "at_name_to_mid",
            "rich_text",
            "pictures",
        ],
    ),
    (
        ".bilibili.main.community.reply.v1.ReplyControl",
        &[
            "action",
            "up_reply",
            "is_up_top",
            "is_note",
            "card_labels",
            "location",
            "is_note_v2",
            "vote_option",
            "translation_switch",
            "show_translation",
        ],
    ),
    (
        ".bilibili.main.community.reply.v1.ReplyControl.VoteOption",
        &["label_kind", "desc"],
    ),
    (
        ".bilibili.main.community.reply.v1.ReplyCardLabel",
        &["text_content"],
    ),
    (
        ".bilibili.main.community.reply.v1.Member",
        &[
            "mid",
            "name",
            "face",
            "level",
            "official_verify_type",
            "vip_type",
            "vip_status",
            "garb_pendant_image",
            "fans_medal_name",
            "fans_medal_level",
            "fans_medal_color",
            "fans_medal_color_name",
            "is_senior_member",
        ],
    ),
    // 界面只画头像右侧那张装扮卡，`MemberV2` 的其余八棵子树各自通向
    // dagw / vas.garb / account.service。
    (".bilibili.main.community.reply.v1.MemberV2", &["garb"]),
    (
        ".bilibili.main.community.reply.v1.MemberV2.Garb",
        &[
            "card_image",
            "card_number",
            "card_fan_color",
            "fan_num_prefix",
        ],
    ),
    (
        ".bilibili.main.community.reply.v1.Emote",
        &["size", "url", "jump_url", "gif_url", "webp_url"],
    ),
    (
        ".bilibili.main.community.reply.v1.Url",
        &[
            "title",
            "prefix_icon",
            "app_url_schema",
            "click_report",
            "extra",
        ],
    ),
    (
        ".bilibili.main.community.reply.v1.Url.Extra",
        &[
            "goods_item_id",
            "goods_prefetched_cache",
            "is_word_search",
            "goods_cm_control",
        ],
    ),
    (
        ".bilibili.main.community.reply.v1.Picture",
        &["img_src", "img_width", "img_height", "img_size"],
    ),
    (".bilibili.main.community.reply.v1.Vote", &["id", "title"]),
    (
        ".bilibili.main.community.reply.v1.VoteCard",
        &["vote_id", "title", "count"],
    ),
    // 评论区状态：开关与文案之外的那二十来个字段没有消费者。
    (
        ".bilibili.main.community.reply.v1.SubjectControl",
        &[
            "up_mid",
            "show_title",
            "input_disable",
            "root_text",
            "count",
            "title",
        ],
    ),
    // `RichText` 的两个分支都只读一个字段；opus 那边不跟进动态树。
    (
        ".bilibili.main.community.reply.v1.RichTextNote",
        &["click_url"],
    ),
    (".bilibili.app.dynamic.v2.OpusItem", &["opus_id"]),
    // `CM.source_content` 是 `google.protobuf.Any`，没有消费者。留着它就要把
    // `prost-types` 链进发布产物，换来一个谁都不读的字段。
    (".bilibili.main.community.reply.v1.CM", &[]),
];

/// 按 [`KEEP`] 裁字段；有规则没命中、或规则点名的字段不存在就失败。
fn keep_only(files: &mut [FileDescriptorProto]) -> Result<(), String> {
    fn walk(
        items: &mut [DescriptorProto],
        prefix: &str,
        hit: &mut BTreeSet<String>,
        missing: &mut Vec<String>,
    ) {
        for item in items {
            let name = format!("{prefix}.{}", item.name());
            if let Some((_, fields)) = KEEP.iter().find(|(target, _)| *target == name) {
                hit.insert(name.clone());
                for field in *fields {
                    if !item.field.iter().any(|f| f.name() == *field) {
                        missing.push(format!("{name}.{field}"));
                    }
                }
                item.field.retain(|f| fields.contains(&f.name()));
            }
            walk(&mut item.nested_type, &name, hit, missing);
        }
    }
    let mut hit = BTreeSet::new();
    let mut missing = Vec::new();
    for file in files {
        let prefix = format!(".{}", file.package());
        walk(&mut file.message_type, &prefix, &mut hit, &mut missing);
    }
    if let Some(field) = missing.first() {
        return Err(format!("keep rule names a field that is gone: {field}"));
    }
    for (message, _) in KEEP {
        if !hit.contains(*message) {
            return Err(format!("keep rule matched no message: {message}"));
        }
    }
    Ok(())
}

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

/// 按 [`KEEP`] 裁过之后，某条消息可能整条不可达了（例如 `MemberV2.basic`
/// 一去，`MemberV2.Basic` 就没人引用，而它正是 `dagw/component/avatar`
/// 那 694 行的入口）。可达性要**按消息**算，不能按顶层消息算：嵌套消息
/// 跟着外层整棵带进来，等于白裁。
///
/// 不可达但仍有可达后代的外层消息留成一个**空壳**（字段清掉），因为
/// prost 把嵌套类型生成在外层的模块里，外层整条删掉后代就没处放了。
fn retain_selected(items: &mut Vec<DescriptorProto>, prefix: &str, selected: &BTreeSet<String>) {
    items.retain_mut(|message| {
        let name = format!("{prefix}.{}", message.name());
        retain_selected(&mut message.nested_type, &name, selected);
        message
            .enum_type
            .retain(|e| selected.contains(&format!("{name}.{}", e.name())));
        if selected.contains(&name) {
            return true;
        }
        if message.nested_type.is_empty() && message.enum_type.is_empty() {
            return false;
        }
        message.field.clear();
        message.oneof_decl.clear();
        true
    });
}

fn select(
    files: Vec<FileDescriptorProto>,
    roots: &[&str],
) -> Result<Vec<FileDescriptorProto>, String> {
    let known = definitions(&files);
    let mut pending: Vec<String> = roots.iter().map(|s| s.to_string()).collect();
    let mut selected = BTreeSet::new();
    // 逐条消息算依赖闭包：只跟着**留下来的**字段走。
    while let Some(name) = pending.pop() {
        if !selected.insert(name.clone()) {
            continue;
        }
        let index = *known
            .get(&name)
            .ok_or_else(|| format!("unknown type {name}"))?;
        let file = &files[index];
        let mut all = BTreeMap::new();
        messages(
            &file.message_type,
            &format!(".{}", file.package()),
            &mut all,
        );
        // 枚举没有字段，查不到就是枚举，不是错。
        if let Some(message) = all.get(&name) {
            pending.extend(message.field.iter().filter_map(|f| f.type_name.clone()));
        }
    }
    let mut result = Vec::new();
    for mut file in files {
        let prefix = format!(".{}", file.package());
        retain_selected(&mut file.message_type, &prefix, &selected);
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
    let mut sources: Vec<_> = files.into_values().collect();
    keep_only(&mut sources)?;
    let mut file = select(sources, ROOTS)?;
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

    /// 死规则必须报错：上游改名或删掉消息之后，留着的规则会悄悄失效。
    #[test]
    fn rejects_stale_keep_rules() {
        let mut files = vec![file()];
        files[0].message_type[0].field[0].label = Some(1);
        assert!(
            keep_only(&mut files)
                .unwrap_err()
                .contains("keep rule matched no message")
        );
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
