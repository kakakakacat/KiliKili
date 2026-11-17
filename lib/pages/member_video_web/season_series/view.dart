import 'package:kilikili/models/common/member/archive_sort_type_app.dart';
import 'package:kilikili/models/common/member/web_ss_type.dart';
import 'package:kilikili/pages/member_video_web/base/view.dart';
import 'package:kilikili/pages/member_video_web/season_series/controller.dart';
import 'package:kilikili/services/rust/rust_space.dart' as rust;
import 'package:kilikili/services/rust/space_web_compat.dart';
import 'package:get/get.dart';
import 'package:material_ui/material_ui.dart';

class MemberSSWeb extends StatefulWidget {
  const MemberSSWeb({super.key});

  @override
  State<MemberSSWeb> createState() => _MemberSSWebState();

  static Future<void>? toMemberSSWeb({
    required WebSsType type,
    required int id,
    required int mid,
    required String name,
  }) {
    return Get.toNamed(
      '/ssWeb',
      arguments: {
        'type': type,
        'id': id,
        'mid': mid,
        'name': name,
      },
    );
  }
}

class _MemberSSWebState
    extends
        BaseVideoWebState<
          MemberSSWeb,
          rust.SpaceSeasonWebPage,
          SpaceWebVideoModel,
          ArchiveSortTypeApp
        > {
  @override
  late final MemberSSWebCtr controller;

  @override
  void initState() {
    super.initState();
    controller = Get.put(MemberSSWebCtr(), tag: name);
  }

  @override
  List<ArchiveSortTypeApp> get values => ArchiveSortTypeApp.values;
}
