import 'dart:convert';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:fixnum/fixnum.dart';
import 'package:kilikili/grpc/bilibili/app/dynamic/v2.pb.dart';
import 'package:kilikili/grpc/bilibili/pagination.pb.dart';

/// gRPC 需设备元信息与登录凭据，不能匿名取得真实响应。
/// 用基线编码器生成脱敏边界样本，再冻结旧页面实际读取字段的投影。
/// parity 测试只读冻结 JSON，不依赖 lib/ 或 Flutter SDK 补丁。
void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/space_tabs';
  final coverCases = <MdlDynDrawItem?>[
    MdlDynDrawItem(
      src: 'https://example.invalid/long.jpg',
      width: Int64(100),
      height: Int64(900),
    ),
    MdlDynDrawItem(
      src: 'https://example.invalid/wide.jpg',
      width: Int64(900),
      height: Int64(100),
    ),
    MdlDynDrawItem(
      src: 'https://example.invalid/zero.jpg',
      width: Int64(0),
      height: Int64(1),
    ),
    MdlDynDrawItem(
      src: 'https://example.invalid/nan.jpg',
      width: Int64(0),
      height: Int64(0),
    ),
    MdlDynDrawItem(
      src: 'https://example.invalid/height-zero.jpg',
      width: Int64(2),
      height: Int64(0),
    ),
    MdlDynDrawItem(src: 'https://example.invalid/unknown.jpg'),
    null,
  ];
  final reply = OpusSpaceFlowResp(
    itemList: [
      for (var n = 0; n < coverCases.length; n++)
        OpusFlowItem(
          oid: Int64(100 + n),
          extend: n == 0
              ? Extend(
                  origDesc: [
                    Description(text: '第一段'),
                    Description(text: '第二段'),
                  ],
                )
              : n == 1
              ? Extend()
              : null,
          flowItemOpus: FlowItemOpus(
            coverPic: coverCases[n],
            bottomLeftText2: CoverIconWithText(text: '1.2万'),
          ),
        ),
      OpusFlowItem(),
    ],
    nextPage: PaginationReply(next: 'next-cursor'),
    hostUpOpusCollection: SectionOpusCollection(
      title: '文集',
      expand: true,
      allCollections: [
        OpusCollectionWithCover(),
        OpusCollectionWithCover(collectionInfo: OpusCollection()),
        OpusCollectionWithCover(
          collectionInfo: OpusCollection(
            collectionId: Int64(200),
            title: OneLineText(
              texts: [
                TextWithPriority(text: '文集'),
                TextWithPriority(text: '标题'),
              ],
            ),
          ),
          coverPic: 'https://example.invalid/collection.jpg',
          coverBottomText: CoverIconWithText(text: '共12篇'),
          subTitleText: '副标题',
        ),
      ],
    ),
  );
  final bytes = reply.writeToBuffer();
  File('$dir/opus.raw.pb').writeAsBytesSync(bytes);
  final decoded = OpusSpaceFlowResp.fromBuffer(bytes);
  final c = decoded.hostUpOpusCollection;
  final baseline = {
    'wire_sha256': sha256.convert(bytes).toString(),
    'data': {
      'next': decoded.nextPage.next.isNotEmpty ? decoded.nextPage.next : null,
      'items': [
        for (final i in decoded.itemList)
          {
            'oid': i.oid.toInt(),
            'content': i.hasExtend()
                ? i.extend.origDesc.map((e) => e.text).join()
                : null,
            'cover': !i.flowItemOpus.hasCoverPic()
                ? null
                : (() {
                    final pic = i.flowItemOpus.coverPic;
                    final ratio = pic.hasWidth() && pic.hasHeight()
                        ? clampDouble(
                            pic.height.toInt() / pic.width.toInt(),
                            0.68,
                            2.7,
                          )
                        : 1.0;
                    return {
                      'src': pic.src,
                      'ratio': ratio,
                      'is_long_pic': ratio >= 2.6,
                    };
                  })(),
            'like_text': i.flowItemOpus.bottomLeftText2.text,
          },
      ],
      'collection': {
        'title': c.title,
        'expand': c.expand,
        'items': [
          for (final i in c.allCollections)
            {
              'collection_id': i.collectionInfo.collectionId.toInt(),
              'title': i.collectionInfo.title.texts.map((e) => e.text).join(),
              'cover': i.coverPic,
              'cover_bottom_text': i.coverBottomText.text,
              'sub_title': i.subTitleText,
            },
        ],
      },
    },
  };
  File('$dir/opus.dart.json').writeAsStringSync(
    '${const JsonEncoder.withIndent('  ').convert(baseline)}\n',
  );
}

// Flutter foundation clampDouble 的基线分支，独立工具避免依赖 dart:ui。
double clampDouble(double x, double min, double max) {
  if (x < min) return min;
  if (x > max) return max;
  if (x.isNaN) return max;
  return x;
}
