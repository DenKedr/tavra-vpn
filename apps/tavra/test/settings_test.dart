import 'package:flutter_test/flutter_test.dart';
import 'package:tavra/settings.dart';

void main() {
  test('normalizes URL host and rejects credentials and wildcard', () {
    expect(normalizeSite('https://Example.COM/path'), 'example.com');
    expect(
      () => normalizeSite('https://me:secret@example.com'),
      throwsFormatException,
    );
    expect(() => normalizeSite('*.example.com'), throwsFormatException);
  });
  test('validates IPv4 IPv6 literals and bounded CIDR', () {
    for (final value in [
      '192.168.1.0/24',
      '2001:db8::/32',
      '::1',
      '10.0.0.1',
    ]) {
      expect(validNetwork(value), true);
    }
    for (final value in [
      '10.0.0.0/33',
      '2001:db8::/129',
      '999.0.0.1',
      'example.com',
      '0.0.0.0/-1',
    ]) {
      expect(validNetwork(value), false);
    }
  });
}
