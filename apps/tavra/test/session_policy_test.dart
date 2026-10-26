import 'dart:convert';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:tavra/api.dart';
import 'package:tavra/local_settings.dart';

Map<String, dynamic> policy(String dns) => {
  'bypass_lan': true,
  'excluded_sites': <String>[],
  'excluded_networks': <String>[],
  'protection': {
    'dns_server': dns,
    'filter_lists': <String>[],
    'allow_domains': <String>[],
    'block_domains': <String>[],
  },
};
void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  test('401 clears authenticated session before reporting failure', () async {
    final api = TavraApi(
      'http://localhost:8787',
      client: MockClient(
        (r) async => http.Response('{"error":{"code":"unauthorized"}}', 401),
      ),
    )..token = 'expired';
    await expectLater(
      api.request('GET', '/overview'),
      throwsA(isA<ApiFailure>()),
    );
    expect(api.token, isNull);
  });
  test(
    'invalid local and remote policy never reaches storage or HTTP',
    () async {
      SharedPreferences.setMockInitialValues({});
      var calls = 0;
      final api = TavraApi(
        'http://localhost:8787',
        client: MockClient((r) async {
          calls++;
          return http.Response('{}', 200);
        }),
      );
      final invalid = [
        for (final dns in [
          'http://dns.test',
          'https://user:pass@dns.test',
          'https://dns.test?a=1',
          'tls://dns.test#x',
          'quic://dns.test:0',
          'https://dns.test/' + ('x' * 2048),
        ])
          policy(dns),
        {
          ...policy(''),
          'excluded_sites': List.filled(129, 'example.com'),
          'excluded_networks': List.filled(128, '10.0.0.0/8'),
        },
        {
          ...policy(''),
          'protection': {
            'dns_server': '',
            'filter_lists': List.filled(33, 'https://filter.test/list'),
          },
        },
      ];
      for (final value in invalid) {
        await expectLater(LocalSettings().save(value), throwsA(anything));
        await expectLater(
          api.request('PUT', '/settings', value),
          throwsA(anything),
        );
      }
      expect(calls, 0);
      expect(
        (await SharedPreferences.getInstance()).getString(LocalSettings.key),
        isNull,
      );
    },
  );
  test(
    'encrypted DNS and bounded lists persist locally and remotely',
    () async {
      SharedPreferences.setMockInitialValues({});
      final api = TavraApi(
        'http://localhost:8787',
        client: MockClient((r) async => http.Response(r.body, 200)),
      );
      for (final dns in [
        'https://dns.test/dns-query',
        'tls://dns.test:853',
        'quic://dns.test:853',
        '',
      ]) {
        final value = policy(dns);
        await LocalSettings().save(value);
        expect(
          await api.request('PUT', '/settings', value),
          jsonDecode(jsonEncode(value)),
        );
      }
    },
  );
}
