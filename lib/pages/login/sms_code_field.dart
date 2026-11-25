import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';

class SmsCodeField extends StatefulWidget {
  const SmsCodeField({
    required this.controller,
    required this.focusNode,
    required this.onSubmit,
    super.key,
  });

  final TextEditingController controller;
  final FocusNode focusNode;
  final Future<void> Function() onSubmit;

  @override
  State<SmsCodeField> createState() => _SmsCodeFieldState();
}

class _SmsCodeFieldState extends State<SmsCodeField> {
  bool _submitting = false;

  Future<void> _submit(String value) async {
    // 六位自动提交暂留宿主输入事件，复用现有登录流程；随 #65 迁移。
    if (_submitting || value.length != 6) return;
    _submitting = true;
    try {
      await widget.onSubmit();
    } finally {
      _submitting = false;
    }
  }

  @override
  Widget build(BuildContext context) => TextField(
    controller: widget.controller,
    focusNode: widget.focusNode,
    decoration: const InputDecoration(
      prefixIcon: Icon(Icons.sms_outlined),
      border: InputBorder.none,
      // 外面是胶囊壳（登录页 `_pillShell`），标签上浮会顶到壳的上边缘。
      floatingLabelBehavior: FloatingLabelBehavior.never,
      labelText: '验证码',
    ),
    keyboardType: TextInputType.number,
    textInputAction: TextInputAction.done,
    autofillHints: const [AutofillHints.oneTimeCode],
    inputFormatters: [
      FilteringTextInputFormatter.digitsOnly,
      LengthLimitingTextInputFormatter(6),
    ],
    onChanged: _submit,
    onSubmitted: _submit,
  );
}
