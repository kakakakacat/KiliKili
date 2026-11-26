import 'dart:io' show File;

import 'package:kilikili/common/style.dart';
import 'package:kilikili/common/widgets/button/icon_button.dart';
import 'package:kilikili/common/widgets/button/toolbar_icon_button.dart';
import 'package:kilikili/common/widgets/flutter/text_field/controller.dart';
import 'package:kilikili/common/widgets/flutter/text_field/text_field.dart';
import 'package:kilikili/http/loading_state.dart';
import 'package:kilikili/models/common/image_preview_type.dart';
import 'package:kilikili/models/common/publish_panel_type.dart';
import 'package:kilikili/models/dynamics/result.dart'
    show PicModel, FilePicModel, OpusPicModel;
import 'package:kilikili/models_new/dynamic/dyn_mention/item.dart';
import 'package:kilikili/services/rust/rust_live.dart' show LiveEmoticon;
import 'package:kilikili/pages/common/publish/common_publish_page.dart';
import 'package:kilikili/pages/dynamics_mention/view.dart';
import 'package:kilikili/services/rust/rust_reply.dart' as e;
import 'package:kilikili/services/rust/rust_upload.dart';
import 'package:kilikili/utils/cache_manager.dart';
import 'package:kilikili/utils/extension/file_ext.dart';
import 'package:kilikili/utils/extension/num_ext.dart';
import 'package:kilikili/utils/extension/string_ext.dart';
import 'package:kilikili/utils/extension/theme_ext.dart';
import 'package:kilikili/utils/feed_back.dart';
import 'package:kilikili/utils/image_utils.dart';
import 'package:kilikili/utils/page_utils.dart';
import 'package:kilikili/utils/platform_utils.dart';
import 'package:cached_network_image_ce/cached_network_image.dart'
    hide CacheManager;
import 'package:easy_debounce/easy_throttle.dart';
import 'package:flutter/foundation.dart' show kDebugMode;
import 'package:flutter/services.dart';
import 'package:flutter_smart_dialog/flutter_smart_dialog.dart';
import 'package:get/get.dart';
import 'package:image_cropper/image_cropper.dart';
import 'package:image_picker/image_picker.dart';
import 'package:material_ui/material_ui.dart';

abstract class CommonRichTextPubPage
    extends CommonPublishPage<List<RichTextItem>> {
  const CommonRichTextPubPage({
    super.key,
    this.items,
    this.pics,
    super.onSave,
    super.autofocus,
    super.panelType,
    super.imageLengthLimit,
  });

  final List<RichTextItem>? items;
  final List<PicModel>? pics;
}

abstract class CommonRichTextPubPageState<T extends CommonRichTextPubPage>
    extends CommonPublishPageState<T> {
  final key = GlobalKey<RichTextFieldState>();
  late final imagePicker = ImagePicker();
  late final RxList<PicModel> imageList;
  int get limit => widget.imageLengthLimit;

  @override
  late final RichTextEditingController editController;

  @override
  void initPubState() {
    editController = RichTextEditingController(
      items: widget.items,
      onMention: onMention,
    );
    if (editController.rawText.trim().isNotEmpty) {
      enablePublish.value = true;
    }
    imageList = RxList<PicModel>(widget.pics ?? <PicModel>[]);
  }

  /// 正在传的那一批配图的 id，与 [imageList] 一一对应（网络图那几位是 null）。
  ///
  /// 存在 state 上而不是 [_uploadPictures] 的局部变量里，是因为取消来自
  /// 两个它管不到的地方：用返回键关掉加载框、页面被销毁（#273）。
  List<int?> _uploadIds = const [];

  /// 这一批是不是已经被用户放弃了。放弃后不再弹 toast、不再关加载框
  /// （它已经关了，再调 `SmartDialog.dismiss` 会关掉别的东西），也不再发布。
  bool _uploadAborted = false;

  void _abortUploads() {
    _uploadAborted = true;
    for (final id in _uploadIds) {
      RustUpload.cancel(id);
    }
  }

  @override
  void dispose() {
    // 先停上传再删文件：后台上传可能还在读下面要删的那几个文件。
    _abortUploads();
    if (PlatformUtils.isMobile) {
      for (final img in imageList) {
        if (img is FilePicModel) {
          File(img.path).tryDel();
        }
      }
    }
    super.dispose();
  }

  @override
  void didChangeDependencies() {
    editController.richStyle = null;
    super.didChangeDependencies();
  }

  Widget buildImage(int index, double height) {
    final color = Theme.of(
      context,
    ).colorScheme.secondaryContainer.withValues(alpha: 0.5);

    void onClear() {
      final image = imageList.removeAt(index);
      if (PlatformUtils.isMobile) {
        if (image is FilePicModel) {
          File(image.path).tryDel();
        }
      }
      if (imageList.isEmpty && editController.rawText.trim().isEmpty) {
        enablePublish.value = false;
      }
    }

    final image = imageList[index];
    return Stack(
      clipBehavior: Clip.none,
      children: [
        GestureDetector(
          onTap: () => PageUtils.imageView(
            imgList: imageList
                .map(
                  (img) => switch (img) {
                    FilePicModel e => SourceModel(
                      url: e.path,
                      sourceType: .fileImage,
                    ),
                    OpusPicModel e => SourceModel(
                      url: e.url!,
                      sourceType: .networkImage,
                    ),
                  },
                )
                .toList(),
            initialPage: index,
          ),
          onLongPress: () {
            Feedback.forLongPress(context);
            onClear();
          },
          onSecondaryTap: PlatformUtils.isMobile ? null : onClear,
          child: ClipRRect(
            borderRadius: const BorderRadius.all(Radius.circular(4)),
            child: ConstrainedBox(
              constraints: const BoxConstraints(minWidth: 42),
              child: switch (image) {
                FilePicModel e => Image.file(
                  File(e.path),
                  height: height,
                  filterQuality: .low,
                  cacheHeight: height.cacheSize(context),
                ),
                OpusPicModel e => CachedNetworkImage(
                  imageUrl: ImageUtils.thumbnailUrl(e.url!),
                  height: height,
                  filterQuality: .low,
                  memCacheHeight: height.cacheSize(context),
                  fadeInDuration: .zero,
                  fadeOutDuration: .zero,
                  placeholder: (_, _) => const SizedBox(width: 42),
                ),
              },
            ),
          ),
        ),
        if (kDebugMode || PlatformUtils.isMobile)
          Positioned(
            top: 34,
            right: 5,
            child: iconButton(
              icon: const Icon(Icons.edit),
              onPressed: () => onCropImage(index, image),
              size: 24,
              iconSize: 14,
              bgColor: color,
            ),
          ),
        Positioned(
          top: 5,
          right: 5,
          child: iconButton(
            icon: const Icon(Icons.clear),
            onPressed: onClear,
            size: 24,
            iconSize: 14,
            bgColor: color,
          ),
        ),
      ],
    );
  }

  Future<void> onCropImage(int index, PicModel image) async {
    String? path;
    switch (image) {
      case FilePicModel e:
        path = e.path;
      case OpusPicModel e:
        SmartDialog.showLoading();
        path = (await CacheManager.manager.getSingleFile(
          e.url.http2https,
        )).path;
        await SmartDialog.dismiss();
    }
    if (!mounted || path.isEmpty) return;
    late final colorScheme = ColorScheme.of(context);
    final croppedFile = await ImageCropper.platform.cropImage(
      sourcePath: path,
      uiSettings: [
        AndroidUiSettings(
          toolbarTitle: '裁剪',
          toolbarColor: colorScheme.secondaryContainer,
          toolbarWidgetColor: colorScheme.onSecondaryContainer,
          statusBarLight: colorScheme.isLight,
        ),
        IOSUiSettings(title: '裁剪'),
      ],
    );
    if (croppedFile != null) {
      if (image is FilePicModel) {
        File(image.path).tryDel();
      }
      imageList[index] = FilePicModel(path: croppedFile.path);
    }
  }

  void onPickImageThrottle([VoidCallback? callback]) {
    EasyThrottle.throttle(
      'imagePicker',
      const Duration(milliseconds: 500),
      () => _onPickImage(callback),
    );
  }

  Future<void> _onPickImage([VoidCallback? callback]) async {
    try {
      final pickedFiles = await imagePicker.pickMultiImage(
        limit: limit,
        imageQuality: 100,
        requestFullMetadata: false,
      );
      if (pickedFiles.isNotEmpty) {
        for (int i = 0; i < pickedFiles.length; i++) {
          if (imageList.length == limit) {
            SmartDialog.showToast('最多选择$limit张图片');
            break;
          } else {
            imageList.add(FilePicModel(path: pickedFiles[i].path));
          }
        }
        callback?.call();
      }
    } catch (e) {
      SmartDialog.showToast(e.toString());
    }
  }

  void onChooseEmote(dynamic emote, double? width, double? height) {
    if (emote is e.Emote) {
      final isTextEmote = width == null;
      onInsertText(
        isTextEmote ? emote.text! : Style.placeHolder,
        .emoji,
        rawText: emote.text!,
        emote: isTextEmote
            ? null
            : Emote(
                url: emote.url!,
                width: width,
                height: height,
              ),
      );
    } else if (emote is LiveEmoticon) {
      onInsertText(
        Style.placeHolder,
        .emoji,
        rawText: emote.emoji!,
        emote: Emote(
          url: emote.url!,
          width: width!,
          height: height,
        ),
      );
    }
  }

  List<Map<String, dynamic>>? getRichContent() {
    if (editController.items.isEmpty) return null;
    final list = <Map<String, dynamic>>[];
    for (final e in editController.items) {
      switch (e.type) {
        case .text || .composing || .common || .latex:
          list.add({
            "raw_text": e.text,
            "type": 1,
            "biz_id": "",
          });
        case .at:
          list
            ..add({
              "raw_text": '@${e.rawText}',
              "type": 2,
              "biz_id": e.id,
            })
            ..add({
              "raw_text": ' ',
              "type": 1,
              "biz_id": "",
            });
        case .emoji:
          list.add({
            "raw_text": e.rawText,
            "type": 9,
            "biz_id": "",
          });
        case .vote:
          list
            ..add({
              "raw_text": e.rawText,
              "type": 4,
              "biz_id": e.id,
            })
            ..add({
              "raw_text": ' ',
              "type": 1,
              "biz_id": "",
            });
      }
    }
    return list;
  }

  late double _mentionOffset = 0;
  MentionItem? get topMentionItem => null;
  Future<void>? onMention([bool fromClick = false]) async {
    final res = await DynMentionPanel.onDynMention(
      context,
      offset: _mentionOffset,
      onCachePos: (offset) => _mentionOffset = offset,
      top: topMentionItem,
    );
    if (res != null) {
      if (res is MentionItem) {
        _onInsertUser(res, fromClick);
      } else if (res is Set<MentionItem>) {
        for (final e in res) {
          _onInsertUser(e..checked = false, fromClick);
          fromClick = true;
        }
        res.clear();
      }
    }
  }

  void _onInsertUser(MentionItem e, bool fromClick) {
    onInsertText(
      '@${e.name} ',
      .at,
      rawText: e.name,
      id: e.uid,
      fromClick: fromClick,
    );
  }

  void onInsertText(
    String text,
    RichTextType type, {
    String? rawText,
    Emote? emote,
    String? id,
    bool? fromClick,
  }) {
    if (text.isEmpty) {
      return;
    }

    enablePublish.value = true;

    final oldValue = editController.value;
    final selection = oldValue.selection;

    if (selection.isValid) {
      TextEditingDelta delta;

      if (selection.isCollapsed) {
        if (type == .at && fromClick == false) {
          delta = RichTextEditingDeltaReplacement(
            oldText: oldValue.text,
            replacementText: text,
            replacedRange: TextRange(
              start: selection.start - 1,
              end: selection.end,
            ),
            selection: TextSelection.collapsed(
              offset: selection.start - 1 + text.length,
            ),
            composing: TextRange.empty,
            rawText: rawText,
            type: type,
            emote: emote,
            id: id,
          );
        } else {
          delta = RichTextEditingDeltaInsertion(
            oldText: oldValue.text,
            textInserted: text,
            insertionOffset: selection.start,
            selection: TextSelection.collapsed(
              offset: selection.start + text.length,
            ),
            composing: TextRange.empty,
            rawText: rawText,
            type: type,
            emote: emote,
            id: id,
          );
        }
      } else {
        delta = RichTextEditingDeltaReplacement(
          oldText: oldValue.text,
          replacementText: text,
          replacedRange: selection,
          selection: TextSelection.collapsed(
            offset: selection.start + text.length,
          ),
          composing: TextRange.empty,
          rawText: rawText,
          type: type,
          emote: emote,
          id: id,
        );
      }

      final newValue = delta.apply(oldValue);

      if (oldValue == newValue) {
        return;
      }

      editController
        ..syncRichText(delta)
        ..value = newValue;
    } else {
      editController.items
        ..clear()
        ..add(
          RichTextItem(
            type: type,
            text: text,
            rawText: rawText,
            range: TextRange(
              start: 0,
              end: text.length,
            ),
            emote: emote,
            id: id,
          ),
        );
      editController.value = TextEditingValue(
        text: text,
        selection: TextSelection.collapsed(offset: text.length),
      );
    }

    key.currentState?.scheduleShowCaretOnScreen(withAnimation: true);
  }

  @override
  void onSave() => widget.onSave?.call(editController.items);

  Widget get emojiBtn => Obx(
    () {
      final isEmoji = panelType.value == PanelType.emoji;
      return ToolbarIconButton(
        tooltip: isEmoji ? '输入' : '表情',
        onPressed: () => updatePanelType(isEmoji ? .keyboard : .emoji),
        icon: isEmoji
            ? const Icon(Icons.keyboard, size: 22)
            : const Icon(Icons.emoji_emotions, size: 22),
        selected: isEmoji,
      );
    },
  );

  Widget get atBtn => ToolbarIconButton(
    onPressed: () => onMention(true),
    icon: const Icon(Icons.alternate_email, size: 22),
    tooltip: '@',
    selected: false,
  );

  Widget get moreBtn => Obx(
    () {
      final isMore = panelType.value == PanelType.more;
      return ToolbarIconButton(
        tooltip: isMore ? '输入' : '更多',
        onPressed: () => updatePanelType(isMore ? .keyboard : .more),
        icon: isMore
            ? const Icon(Icons.keyboard, size: 22)
            : const Icon(Icons.add_circle_outline, size: 22),
        selected: isMore,
      );
    },
  );

  @override
  Future<void> onPublish() async {
    feedBack();
    List<Map<String, dynamic>>? pictures;
    if (imageList.isNotEmpty) {
      pictures = await _uploadPictures();
      // null 表示失败或被取消，toast 已经发过（或刻意不发）。
      if (pictures == null) return;
      // 图刚好全传完、页面却已经退出：用户的意思是「不发了」，这里不能
      // 替用户发出去（#273）。
      if (!mounted) return;
    }
    return onCustomPublish(pictures: pictures);
  }

  /// 把本地图片传上去，和已有的网络图拼成发布要的那串描述。
  ///
  /// 失败返回 `null`。**先全部 begin 再逐个 await**：基线用
  /// `Future.wait(eagerError: true)` 并发传，同时拿一个 `CancelToken` 在出错
  /// 时把其余几张一起停掉。Rust 侧的取消是显式 id（ADR-015 决策三），所以
  /// 这里自己收着那串 id——并发度不变，出错时的行为也不变。
  ///
  /// 用返回键关掉加载框、或者退出页面，都会取消整批（#273）。基线两样都
  /// 不取消：图在后台传完，动态照样发出去，回到首页才弹「发布成功」。
  Future<List<Map<String, dynamic>>?> _uploadPictures() async {
    _uploadAborted = false;
    final ids = _uploadIds = <int?>[];
    // onDismiss 在代码自己关加载框时也会触发；那时这一批要么已全部结束
    // （取消是空操作），要么已经在失败分支里取消过，所以这里不必区分。
    SmartDialog.showLoading(msg: '正在上传图片...', onDismiss: _abortUploads);
    for (final img in imageList) {
      final id = switch (img) {
        FilePicModel e => await RustUpload.beginBfs(
          path: e.path,
          category: 'daily',
          biz: 'new_dyn',
        ),
        OpusPicModel _ => null,
      };
      ids.add(id);
      // 启动是逐张 await 的，放弃可能发生在中途：已经启动的那几张由
      // _abortUploads 取消了，刚启动的这一张它还没见过，这里补上。
      if (_uploadAborted) RustUpload.cancel(id);
    }

    final pictures = <Map<String, dynamic>>[];
    for (var i = 0; i < imageList.length; i++) {
      final img = imageList[i];
      if (img is OpusPicModel) {
        pictures.add(img.toJson());
        continue;
      }
      final state = await RustUpload.awaitBfs(ids[i]);
      if (state case Success(:final response)) {
        pictures.add({
          'img_width': response.width?.toInt(),
          'img_height': response.height?.toInt(),
          // 这里**不要** toInt()：基线收的就是 double，而且这个值原样
          // 发回发布接口，截断等于悄悄改了一个发给服务端的字段。
          'img_size': response.size,
          'img_src': response.url,
        });
        continue;
      }
      // 一张失败就把还没传完的几张停掉，与基线 eagerError + cancelToken
      // 的行为一致。已经传完的那几张在服务端留着——没有删除接口，基线
      // 也是这样，不要假装能回滚。
      for (final id in ids.skip(i + 1)) {
        RustUpload.cancel(id);
      }
      if (_uploadAborted) return null;
      SmartDialog.dismiss();
      if (!RustUpload.isCancelled(state)) state.toast();
      return null;
    }
    // 放弃发生在最后一张刚传完之后：图都在，但用户已经不要发了。
    if (_uploadAborted) return null;
    SmartDialog.dismiss();
    return pictures;
  }
}
