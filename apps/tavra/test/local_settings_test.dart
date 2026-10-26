import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:tavra/local_settings.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  test('device policy persists without credentials', () async {
    SharedPreferences.setMockInitialValues({});
    final storage = LocalSettings();
    await storage.save({
      'bypass_lan': false,
      'excluded_sites': ['example.com'],
      'excluded_networks': [],
      'protection': {},
    });
    expect(
      await LocalSettings().load(),
      containsPair('excluded_sites', ['example.com']),
    );
  });
}
