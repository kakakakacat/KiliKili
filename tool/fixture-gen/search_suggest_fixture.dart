import 'dart:convert';
import 'dart:io';

import 'package:kilikili/models/search/suggest.dart';

const fixtureDir = 'rust/crates/bili-api/tests/fixtures/search_suggest';

void main() {
  final raws =
      Directory(fixtureDir)
          .listSync()
          .whereType<File>()
          .where((f) => f.path.endsWith('.raw.json'))
          .toList()
        ..sort((a, b) => a.path.compareTo(b.path));
  if (raws.isEmpty) {
    stderr.writeln('没有搜索建议 fixture');
    exit(1);
  }

  for (final raw in raws) {
    final body = jsonDecode(raw.readAsStringSync()) as Map<String, dynamic>;
    final model = SearchSuggestModel.fromJson(
      body['data']['result'] as Map<String, dynamic>,
    );
    final normalized = {
      'tag': model.tag
          ?.map(
            (item) => {
              'term': item.term,
              'name': item.textRich,
            },
          )
          .toList(),
    };
    final target = raw.path.replaceFirst('.raw.json', '.normalized.json');
    File(target).writeAsStringSync(
      '${const JsonEncoder.withIndent('  ').convert(normalized)}\n',
    );
  }
}
