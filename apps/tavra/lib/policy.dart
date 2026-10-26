import 'dart:io';
import 'dart:convert';

String normalizeSite(String value) {
  final raw = value.trim();
  final uri = Uri.tryParse(raw.contains('://') ? raw : 'https://$raw');
  if (uri == null ||
      !['http', 'https'].contains(uri.scheme) ||
      uri.userInfo.isNotEmpty ||
      uri.host.isEmpty ||
      uri.host.length > 253 ||
      uri.host.contains('*') ||
      InternetAddress.tryParse(uri.host) != null)
    throw const FormatException(
      'Укажите имя сайта или HTTP(S) URL без пароля.',
    );
  final host = uri.host.toLowerCase().replaceFirst(RegExp(r'\.$'), '');
  if (!host.contains('.') ||
      host
          .split('.')
          .any(
            (s) =>
                s.isEmpty ||
                s.length > 63 ||
                !RegExp(r'^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$').hasMatch(s),
          ))
    throw const FormatException(
      'Неверный домен. Для IDNA используйте punycode.',
    );
  return host;
}

bool validNetwork(String value) {
  final parts = value.trim().split('/');
  if (parts.length > 2) return false;
  final ip = InternetAddress.tryParse(parts.first);
  if (ip == null) return false;
  if (parts.length == 1) return true;
  final prefix = int.tryParse(parts.last);
  return prefix != null &&
      prefix >= 0 &&
      prefix <= (ip.type == InternetAddressType.IPv4 ? 32 : 128);
}

bool validEncryptedUrl(String value, {bool filter = false}) {
  if (utf8.encode(value).length > 2048 || RegExp(r'\s').hasMatch(value))
    return false;
  try {
    final uri = Uri.parse(value);
    final authority = RegExp(
      r'^[a-zA-Z][a-zA-Z0-9+.-]*://([^/?#]*)',
    ).firstMatch(value)?.group(1);
    if (authority == null ||
        authority.contains('@') ||
        RegExp(r':0+$').hasMatch(authority))
      return false;
    return (filter ? ['https'] : ['https', 'tls', 'quic']).contains(
          uri.scheme,
        ) &&
        uri.hasAuthority &&
        uri.host.isNotEmpty &&
        !uri.authority.contains('@') &&
        uri.userInfo.isEmpty &&
        !uri.hasQuery &&
        !uri.hasFragment &&
        (!uri.hasPort || (uri.port > 0 && uri.port <= 65535));
  } catch (_) {
    return false;
  }
}

Map<String, dynamic> validatedPolicy(Map<String, dynamic> config) {
  final sites = List<String>.from(config['excluded_sites'] ?? []);
  final networks = List<String>.from(config['excluded_networks'] ?? []);
  if (sites.length + networks.length > 256)
    throw const FormatException(
      'Не более 256 исключений сайтов и сетей суммарно.',
    );
  final p = Map<String, dynamic>.from(config['protection'] ?? {});
  final lists = List<String>.from(p['filter_lists'] ?? []);
  if (lists.length > 32)
    throw const FormatException('Не более 32 списков фильтров.');
  if (lists.any((u) => !validEncryptedUrl(u, filter: true)))
    throw const FormatException(
      'Список фильтров: нужен HTTPS URL без пароля, query, fragment или порта 0; максимум 2048 байт.',
    );
  final dns = p['dns_server'] as String? ?? '';
  if (dns.isNotEmpty && !validEncryptedUrl(dns))
    throw const FormatException(
      'DNS: нужен HTTPS, TLS или QUIC URL без пароля, query, fragment или порта 0; максимум 2048 байт.',
    );
  if (networks.any((n) => !validNetwork(n)))
    throw const FormatException('Неверный IP или CIDR префикс.');
  for (final field in ['allow_domains', 'block_domains']) {
    final domains = List<String>.from(p[field] ?? []);
    if (domains.length > 256)
      throw const FormatException('Не более 256 доменов в списке.');
    p[field] = domains.map(normalizeSite).toList();
  }
  return {
    'bypass_lan': config['bypass_lan'] ?? true,
    'excluded_sites': sites.map(normalizeSite).toList(),
    'excluded_networks': networks,
    'protection': {...p, 'dns_server': dns, 'filter_lists': lists},
  };
}
