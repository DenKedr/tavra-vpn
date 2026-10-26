import 'dart:convert';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:cryptography/cryptography.dart';
import 'package:tavra/api.dart';

void main() {
  test('accepts persistent base64url key and preserves its bytes', () async {
    const key = 'vN0P4J2dYq7Sx9CkL3Hb6ZtR8Wm5AfEeU1I_o-GhBQs';
    final api = TavraApi(
      'http://127.0.0.1:8787',
      client: MockClient((r) async {
        expect(jsonDecode(r.body)['admin_secret'], key);
        return http.Response('{"token":"session"}', 200);
      }),
    );
    expect(adminKeyError(key), isNull);
    await api.login(key);
    expect(api.token, 'session');
  });
  test(
    'rejects short oversized and control-containing keys before request',
    () async {
      final api = TavraApi(
        'http://127.0.0.1:8787',
        client: MockClient((r) async => throw StateError('must not send')),
      );
      for (final key in [
        'x' * 31,
        'x' * 257,
        'x' * 40 + '\n',
        'x' * 40 + '\u007f',
      ]) {
        expect(adminKeyError(key), isNotNull);
        await expectLater(api.login(key), throwsA(isA<ApiFailure>()));
      }
      expect(adminKeyError('x' * 32), isNull);
      expect(adminKeyError('x' * 256), isNull);
    },
  );
  test('session sends key and authenticates subsequent overview', () async {
    final requests = <http.Request>[];
    final api = TavraApi(
      'http://127.0.0.1:8787',
      client: MockClient((r) async {
        requests.add(r);
        return http.Response(
          jsonEncode(
            r.url.path.endsWith('session')
                ? {'token': 'session'}
                : {'users': 2},
          ),
          200,
        );
      }),
    );
    await api.login('a' * 64);
    expect(await api.request('GET', '/overview'), {'users': 2});
    expect(jsonDecode(requests.first.body), {'admin_secret': 'a' * 64});
    expect(requests.last.headers['Authorization'], 'Bearer session');
  });
  test('API surfaces typed errors without echoing credentials', () async {
    final api = TavraApi(
      'http://localhost:8787',
      client: MockClient(
        (r) async => http.Response(
          '{"error":{"code":"unauthorized","message":"denied"}}',
          401,
        ),
      ),
    );
    expect(
      api.request('GET', '/users'),
      throwsA(isA<ApiFailure>().having((e) => e.status, 'status', 401)),
    );
  });
  test('enrollment proof verifies exact server challenge payload', () async {
    String? publicKey;
    final api = TavraApi(
      'http://localhost:8787',
      client: MockClient((r) async {
        final body = jsonDecode(r.body);
        if (r.url.path.endsWith('challenge')) {
          publicKey = body['public_key'];
          expect(body['invitation'], 'a' * 64);
          return http.Response(
            '{"challenge_id":"id-1","nonce":"nonce-1","expires_at":100}',
            200,
          );
        }
        expect(
          await Ed25519().verify(
            utf8.encode('tavra-enroll-v1\nid-1\nnonce-1\n$publicKey'),
            signature: Signature(
              decodeHex(body['signature']),
              publicKey: SimplePublicKey(
                decodeHex(publicKey!),
                type: KeyPairType.ed25519,
              ),
            ),
          ),
          true,
        );
        return http.Response(
          '{"device_id":"d1","status":"registered","vpn_lease":null}',
          200,
        );
      }),
    );
    expect((await api.enroll('a' * 64))['device_id'], 'd1');
  });
  test('invalid invitation never reaches transport', () async {
    final api = TavraApi(
      'http://localhost:8787',
      client: MockClient((r) async => throw StateError('must not send')),
    );
    expect(api.enroll('invalid'), throwsA(isA<ApiFailure>()));
  });
}
