import 'dart:convert';
import 'dart:io';

import 'package:PiliPlus/services/rust/search_suggest_result.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const dir = 'rust/crates/bili-api/tests/fixtures/search_suggest';
  final raws = Directory(dir)
      .listSync()
      .whereType<File>()
      .where((f) => f.path.endsWith('.raw.json'))
      .toList();

  test('Rust normalized suggestions preserve Dart model fields', () {
    expect(raws, isNotEmpty);
    for (final raw in raws) {
      final body = jsonDecode(raw.readAsStringSync()) as Map<String, dynamic>;
      final golden = File(
        raw.path.replaceFirst('.raw.json', '.normalized.json'),
      ).readAsStringSync();
      final actual = searchSuggestFromRustOrDart(body, golden);
      final baseline = searchSuggestFromRustOrDart(body, null);
      expect(actual.tag?.length, baseline.tag?.length);
      for (var i = 0; i < (actual.tag?.length ?? 0); i++) {
        expect(actual.tag![i].term, baseline.tag![i].term);
        expect(actual.tag![i].textRich, baseline.tag![i].textRich);
      }
    }
  });

  test('malformed Rust result falls back to the original response', () {
    final body = jsonDecode(
      File('$dir/rich.raw.json').readAsStringSync(),
    ) as Map<String, dynamic>;
    final result = searchSuggestFromRustOrDart(body, '{bad json');
    expect(result.tag?.first.term, '测试视频');
  });
}
