import 'package:kilikili/pages/dynamics/widgets/vote.dart';
import 'package:kilikili/services/rust/rust_reply_grpc.dart';
import 'package:kilikili/utils/num_utils.dart';
import 'package:material_ui/material_ui.dart';

Widget buildVoteCard(
  BuildContext context,
  ColorScheme colorScheme,
  ReplyVoteCard voteCard,
) {
  return InkWell(
    onTap: () => showVoteDialog(context, voteCard.voteId),
    child: Padding(
      padding: const .symmetric(horizontal: 12, vertical: 6),
      child: Row(
        spacing: 10,
        children: [
          Container(
            decoration: BoxDecoration(
              color: colorScheme.onInverseSurface,
              borderRadius: const .all(.circular(8)),
            ),
            width: 60,
            height: 42,
            child: Icon(
              Icons.bar_chart_rounded,
              color: colorScheme.onSurfaceVariant,
            ),
          ),
          Expanded(
            child: Column(
              crossAxisAlignment: .start,
              children: [
                Text(voteCard.title, maxLines: 1, overflow: .ellipsis),
                Text(
                  '${NumUtils.numFormat(voteCard.count)}人参与',
                  maxLines: 1,
                  overflow: .ellipsis,
                  style: TextStyle(fontSize: 13, color: colorScheme.outline),
                ),
              ],
            ),
          ),
        ],
      ),
    ),
  );
}
