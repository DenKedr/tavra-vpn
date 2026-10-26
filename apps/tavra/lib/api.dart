import 'dart:async';
import 'policy.dart';
import 'dart:convert';
import 'dart:math';
import 'package:http/http.dart' as http;
import 'package:cryptography/cryptography.dart';

class ApiFailure implements Exception {
  final int status;
  final String message;
  const ApiFailure(this.status, [this.message = 'Запрос не выполнен']);
  @override
  String toString() => message;
}

String encodeHex(List<int> bytes) =>
    bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
List<int> decodeHex(String value) => List.generate(
  value.length ~/ 2,
  (i) => int.parse(value.substring(i * 2, i * 2 + 2), radix: 16),
);
String generateSecret() =>
    encodeHex(List.generate(32, (_) => Random.secure().nextInt(256)));

String? adminKeyError(String value) {
  final length = utf8.encode(value).length;
  if (length < 32 || length > 256)
    return 'Ключ должен содержать от 32 до 256 байт.';
  if (RegExp(r'[\x00-\x1f\x7f-\x9f]').hasMatch(value))
    return 'Ключ не должен содержать управляющие символы.';
  return null;
}

class TavraApi {
  final Uri base;
  final http.Client client;
  final Duration timeout;
  String? token;
  void Function()? onUnauthorized;
  SimpleKeyPair? _deviceKey;
  TavraApi(
    String url, {
    http.Client? client,
    this.timeout = const Duration(seconds: 15),
  }) : base = validateUrl(url),
       client = client ?? http.Client();
  static Uri validateUrl(String value) {
    final uri = Uri.tryParse(value.trim());
    if (uri == null ||
        !uri.hasAuthority ||
        uri.userInfo.isNotEmpty ||
        uri.hasQuery ||
        uri.hasFragment ||
        (uri.path.isNotEmpty && uri.path != '/') ||
        !(uri.scheme == 'https' ||
            (uri.scheme == 'http' &&
                ['localhost', '127.0.0.1', '::1'].contains(uri.host)))) {
      throw const ApiFailure(
        0,
        'Укажите HTTPS адрес API; HTTP разрешён только на этом компьютере.',
      );
    }
    return uri;
  }

  Future<dynamic> request(
    String method,
    String path, [
    Map<String, dynamic>? body,
  ]) async {
    try {
      if (method == 'PUT' && path == '/settings' && body != null) {
        body = validatedPolicy(body);
      }
      final r = http.Request(method, base.resolve('/v1$path'))
        ..followRedirects = false;
      r.headers['Content-Type'] = 'application/json';
      if (token != null) r.headers['Authorization'] = 'Bearer $token';
      if (body != null) r.body = jsonEncode(body);
      final response = await (() async => http.Response.fromStream(
        await client.send(r),
      ))().timeout(timeout);
      if (response.statusCode == 401 &&
          token != null &&
          !path.startsWith('/enroll')) {
        token = null;
        onUnauthorized?.call();
      }
      dynamic value;
      try {
        value = jsonDecode(response.body);
      } catch (_) {
        throw ApiFailure(
          response.statusCode,
          'API вернул неверный формат ответа.',
        );
      }
      if (response.statusCode < 200 || response.statusCode >= 300) {
        // Never display arbitrary server text, which could contain credentials.
        final messages = {
          400: 'Проверьте введённые данные.',
          401: 'Доступ отклонён или срок ключа истёк.',
          404: 'Запись не найдена.',
          409:
              'Действие конфликтует с текущим состоянием. Проверьте лимиты и статус.',
          429: 'Слишком много запросов. Повторите позже.',
        };
        throw ApiFailure(
          response.statusCode,
          messages[response.statusCode] ??
              'Сервер не выполнил запрос (${response.statusCode}).',
        );
      }
      return value;
    } on FormatException catch (e) {
      throw ApiFailure(400, e.message);
    } on ApiFailure {
      rethrow;
    } on TimeoutException {
      throw const ApiFailure(
        0,
        'API не ответил за 15 секунд. Проверьте адрес и доступность сервера.',
      );
    } catch (_) {
      throw const ApiFailure(
        0,
        'Нет связи с API. Проверьте адрес и запущен ли сервер управления.',
      );
    }
  }

  Future<void> login(String key, {String? bootstrap}) async {
    final validation = adminKeyError(key);
    if (validation != null) throw ApiFailure(400, validation);
    final data = await request(
      'POST',
      bootstrap == null ? '/admin/session' : '/admin/bootstrap',
      {
        'admin_secret': key,
        if (bootstrap != null) 'bootstrap_secret': bootstrap,
      },
    );
    token = data['token'] as String;
  }

  Future<Map<String, dynamic>> enroll(String invitation) async {
    if (!RegExp(r'^[a-f0-9]{64}$').hasMatch(invitation))
      throw const ApiFailure(
        400,
        'Приглашение должно содержать 64 символа из 0–9 и a–f.',
      );
    final algorithm = Ed25519();
    _deviceKey ??= await algorithm.newKeyPair();
    final key = _deviceKey!;
    final publicKey = encodeHex((await key.extractPublicKey()).bytes);
    final challenge = await request('POST', '/enroll/challenge', {
      'invitation': invitation,
      'public_key': publicKey,
    });
    final signature = await algorithm.sign(
      utf8.encode(
        'tavra-enroll-v1\n${challenge['challenge_id']}\n${challenge['nonce']}\n$publicKey',
      ),
      keyPair: key,
    );
    return Map<String, dynamic>.from(
      await request('POST', '/enroll', {
        'challenge_id': challenge['challenge_id'],
        'public_key': publicKey,
        'signature': encodeHex(signature.bytes),
      }),
    );
  }

  void close() {
    token = null;
    _deviceKey = null;
    client.close();
  }
}
