import 'package:flutter/material.dart';

/// Keeps field controllers alive until the dialog route actually unmounts,
/// including the reverse transition after Navigator.pop completes its future.
class DialogFields extends StatefulWidget {
  final Map<String, String> initialValues;
  final Widget Function(BuildContext, Map<String, TextEditingController>)
  builder;
  const DialogFields({
    super.key,
    required this.initialValues,
    required this.builder,
  });
  @override
  State<DialogFields> createState() => _DialogFieldsState();
}

class _DialogFieldsState extends State<DialogFields> {
  late final Map<String, TextEditingController> controllers;
  @override
  void initState() {
    super.initState();
    controllers = {
      for (final entry in widget.initialValues.entries)
        entry.key: TextEditingController(text: entry.value),
    };
  }

  @override
  void dispose() {
    for (final controller in controllers.values) {
      controller.dispose();
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => widget.builder(context, controllers);
}
