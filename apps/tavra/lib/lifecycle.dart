import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

class LifecyclePreference {
  static const channel = MethodChannel('dev.tavra/lifecycle');
  Future<bool> read() async =>
      await channel.invokeMethod<bool>('getBackgroundMode') ?? true;
  Future<bool> write(bool value) async {
    final result = await channel.invokeMethod<bool>('setBackgroundMode', value);
    if (result == null) throw PlatformException(code: 'no_result');
    return result;
  }
}

class BackgroundSetting extends StatefulWidget {
  const BackgroundSetting({super.key});
  @override
  State<BackgroundSetting> createState() => _BackgroundSettingState();
}

class _BackgroundSettingState extends State<BackgroundSetting> {
  bool value = true, available = false, busy = false;
  String? error;
  @override
  void initState() {
    super.initState();
    read();
  }

  Future<void> read() async {
    if (!Platform.isMacOS && !Platform.isWindows) {
      setState(
        () => error =
            'Фоновый режим Android ожидает нативный foreground service.',
      );
      return;
    }
    try {
      final result = await LifecyclePreference().read();
      if (mounted)
        setState(() {
          value = result;
          available = true;
        });
    } catch (_) {
      if (mounted)
        setState(
          () => error = 'Настройка фонового режима недоступна в этой сборке.',
        );
    }
  }

  Future<void> change(bool selected) async {
    setState(() => busy = true);
    try {
      final result = await LifecyclePreference().write(selected);
      if (mounted) setState(() => value = result);
    } catch (_) {
      if (mounted)
        setState(() => error = 'Не удалось сохранить фоновый режим.');
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  @override
  Widget build(BuildContext context) => SwitchListTile(
    contentPadding: EdgeInsets.zero,
    value: value,
    onChanged: available && !busy ? change : null,
    title: const Text('Продолжать работу при закрытии окна'),
    subtitle: Text(
      error ??
          'Закрытие окна оставит приложение в меню системы. Для завершения используйте «Выйти» в меню приложения.',
    ),
  );
}
