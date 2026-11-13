import 'package:kilikili/common/widgets/animated_multi_height.dart';
import 'package:material_ui/material_ui.dart';

/// 简介区展开 / 折叠的动画时长。
///
/// 标题高度、简介高度与标题旁的三角指示共用同一个值：三者是同一次点击的
/// 反馈，分开调会在动画中途看出错位。原为 300ms，按 #141 调快。
const Duration kIntroExpandDuration = Duration(milliseconds: 180);

/// 标题旁的展开 / 折叠三角指示。
///
/// 刻意不画在 [ExpandablePanel] 内部：面板内部是折叠态与展开态两份
/// 内容的即时换位，放进去会被整块换掉，方向看着是瞬间跳变而不是
/// 转过去。放在外面由隐式动画独立驱动，连点时能从当前角度接着转（#141）。
class ExpandIndicator extends StatelessWidget {
  const ExpandIndicator({
    super.key,
    required this.expand,
    this.color,
    this.size = 22,
  });

  final bool expand;
  final Color? color;
  final double size;

  @override
  Widget build(BuildContext context) {
    return AnimatedRotation(
      turns: expand ? 0.5 : 0,
      duration: kIntroExpandDuration,
      curve: Curves.easeInOut,
      child: Icon(Icons.arrow_drop_down, size: size, color: color),
    );
  }
}

class ExpandablePanel extends StatelessWidget {
  final bool expand;

  final Widget collapsed;

  final Widget expanded;

  const ExpandablePanel({
    super.key,
    required this.expand,
    required this.collapsed,
    required this.expanded,
  });

  @override
  Widget build(BuildContext context) {
    return _AnimatedCross(
      alignment: .topLeft,
      firstChild: collapsed,
      secondChild: expanded,
      sizeCurve: Curves.linear,
      crossFadeState: expand ? .showSecond : .showFirst,
      duration: kIntroExpandDuration,
    );
  }
}

/// ref [AnimatedCrossFade]
class _AnimatedCross extends StatefulWidget {
  const _AnimatedCross({
    required this.firstChild,
    required this.secondChild,
    this.sizeCurve = Curves.linear,
    this.alignment = Alignment.topCenter,
    required this.crossFadeState,
    required this.duration,
  });

  final Widget firstChild;

  final Widget secondChild;

  final CrossFadeState crossFadeState;

  final Duration duration;

  final Curve sizeCurve;

  final AlignmentGeometry alignment;

  @override
  State<_AnimatedCross> createState() => _AnimatedCrossState();
}

class _AnimatedCrossState extends State<_AnimatedCross> {
  late bool _showFirst;
  AnimationStatus? _status;

  @override
  void initState() {
    super.initState();
    switch (widget.crossFadeState) {
      case .showFirst:
        _showFirst = true;
      case .showSecond:
        _showFirst = false;
    }
  }

  @override
  void didUpdateWidget(_AnimatedCross oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.crossFadeState != oldWidget.crossFadeState) {
      switch (widget.crossFadeState) {
        case .showFirst:
          _status = .reverse;
        case .showSecond:
          _status = .forward;
      }
      _showFirst = false;
    }
  }

  void _onEnd() {
    if (_status == .reverse) {
      _showFirst = true;
    }
    setState(() {});
  }

  Widget get firstChild => _widgetBuilder(_showFirst, widget.firstChild);

  Widget get secondChild => _widgetBuilder(!_showFirst, widget.secondChild);

  static Widget _widgetBuilder(bool visible, Widget child) =>
      Opacity(opacity: visible ? 1.0 : 0.0, child: child);

  @override
  Widget build(BuildContext context) {
    const Key kFirstChildKey = ValueKey<CrossFadeState>(.showFirst);
    const Key kSecondChildKey = ValueKey<CrossFadeState>(.showSecond);

    final bool expand;
    final Key topKey;
    Widget topChild;
    final Key bottomKey;
    Widget bottomChild;

    switch (widget.crossFadeState) {
      case .showFirst:
        expand = false;
        topKey = kFirstChildKey;
        topChild = firstChild;
        bottomKey = kSecondChildKey;
        bottomChild = secondChild;
      case .showSecond:
        expand = true;
        topKey = kSecondChildKey;
        topChild = secondChild;
        bottomKey = kFirstChildKey;
        bottomChild = firstChild;
    }

    return AnimatedMultiHeight(
      duration: widget.duration,
      curve: widget.sizeCurve,
      onEnd: _onEnd,
      expand: expand,
      child: AnimatedCrossFade.defaultLayoutBuilder(
        topChild,
        topKey,
        bottomChild,
        bottomKey,
      ),
    );
  }
}
