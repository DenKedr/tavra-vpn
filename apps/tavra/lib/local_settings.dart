import 'dart:convert';
import 'policy.dart';
import 'package:shared_preferences/shared_preferences.dart';

class LocalSettings {
  static const key = 'tavra.device_policy.v1';
  Future<void> save(Map<String, dynamic> config) async {
    final policy = validatedPolicy({
      for (final field in [
        'bypass_lan',
        'excluded_sites',
        'excluded_networks',
        'protection',
      ])
        field: config[field],
    });
    if (!await (await SharedPreferences.getInstance()).setString(
      key,
      jsonEncode(policy),
    ))
      throw StateError('Persistence failed');
  }

  Future<Map<String, dynamic>?> load() async {
    final raw = (await SharedPreferences.getInstance()).getString(key);
    return raw == null ? null : Map<String, dynamic>.from(jsonDecode(raw));
  }
}
