import 'policy.dart';
import 'dialog_fields.dart';
export 'policy.dart';
import 'package:flutter/material.dart';
import 'api.dart';
import 'local_settings.dart';
import 'lifecycle.dart';

class SettingsView extends StatefulWidget {
  final TavraApi? api;
  final bool local;
  const SettingsView({super.key, this.api, this.local = false});
  @override
  State<SettingsView> createState() => _SettingsViewState();
}

class _SettingsViewState extends State<SettingsView> {
  Map<String, dynamic> config = {
    'bypass_lan': true,
    'excluded_sites': <String>[],
    'excluded_networks': <String>[],
    'protection': <String, dynamic>{
      'ad_blocking': false,
      'trackers': false,
      'malware': false,
      'annoyances': false,
      'dns_server': '',
      'filter_lists': <String>[],
      'allow_domains': <String>[],
      'block_domains': <String>[],
    },
  };
  bool busy = true, loaded = false;
  String? error, message;
  @override
  void initState() {
    super.initState();
    fetch();
  }

  Future<void> fetch() async {
    setState(() {
      busy = true;
      error = null;
    });
    try {
      final value = widget.local
          ? (await LocalSettings().load() ?? config)
          : await widget.api!.request('GET', '/settings');
      if (mounted)
        setState(() {
          config = Map<String, dynamic>.from(value);
          loaded = true;
        });
    } catch (e) {
      if (mounted) setState(() => error = e.toString());
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  Future<void> save() async {
    setState(() {
      busy = true;
      error = null;
      message = null;
    });
    try {
      final policy = {
        for (final field in [
          'bypass_lan',
          'excluded_sites',
          'excluded_networks',
          'protection',
        ])
          field: config[field],
      };
      if (widget.local) {
        await LocalSettings().save(policy);
      } else {
        await widget.api!.request('PUT', '/settings', policy);
      }
      if (mounted)
        setState(
          () => message =
              'Настройки сохранены. Применение к трафику ожидает VPN runtime.',
        );
    } catch (e) {
      if (mounted) setState(() => error = e.toString());
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  Map<String, dynamic> get protection =>
      Map<String, dynamic>.from(config['protection'] ?? {});
  void setProtection(String field, dynamic value) {
    final p = protection;
    p[field] = value;
    setState(() => config['protection'] = p);
  }

  Future<void> addRule(
    String field, {
    bool network = false,
    bool filter = false,
  }) async {
    final form = GlobalKey<FormState>();
    String? normalized;
    final result = await showDialog<String>(
      context: context,
      builder: (_) => DialogFields(
        initialValues: const {'value': ''},
        builder: (c, controllers) => AlertDialog(
          title: Text(
            network
                ? 'Исключение IP / подсети'
                : filter
                ? 'URL списка фильтров'
                : 'Правило сайта',
          ),
          content: SizedBox(
            width: 440,
            child: Form(
              key: form,
              child: TextFormField(
                controller: controllers['value'],
                autofocus: true,
                decoration: InputDecoration(
                  labelText: network
                      ? 'IPv4, IPv6 или CIDR'
                      : filter
                      ? 'HTTPS URL'
                      : 'Домен или HTTP(S) URL',
                ),
                validator: (v) {
                  try {
                    if (network) {
                      if (!validNetwork(v ?? ''))
                        return 'Некорректный IP или CIDR префикс';
                      normalized = v!.trim();
                    } else if (filter) {
                      if (!validEncryptedUrl(v ?? '', filter: true))
                        return 'Нужен HTTPS URL без пароля, query, fragment, порта 0; до 2048 байт';
                      normalized = v;
                    } else {
                      normalized = normalizeSite(v ?? '');
                    }
                    return null;
                  } on FormatException catch (e) {
                    return e.message;
                  }
                },
              ),
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(c),
              child: const Text('Отмена'),
            ),
            FilledButton(
              onPressed: () {
                if (form.currentState!.validate()) Navigator.pop(c, normalized);
              },
              child: const Text('Добавить'),
            ),
          ],
        ),
      ),
    );
    if (result == null || !mounted) return;
    final routing = ['excluded_sites', 'excluded_networks'].contains(field);
    final list = List<String>.from(
      routing ? config[field] ?? [] : protection[field] ?? [],
    );
    if (list.contains(result)) return;
    final routingCount =
        (config['excluded_sites'] as List? ?? []).length +
        (config['excluded_networks'] as List? ?? []).length;
    if ((routing && routingCount >= 256) ||
        (!routing && list.length >= (filter ? 32 : 256))) {
      setState(
        () => error = routing
            ? 'Не более 256 исключений сайтов и сетей суммарно.'
            : filter
            ? 'Не более 32 списков фильтров.'
            : 'Не более 256 правил в списке.',
      );
      return;
    }
    list.add(result);
    if (routing) {
      setState(() => config[field] = list);
    } else {
      setProtection(field, list);
    }
  }

  Widget rules(
    String title,
    String field, {
    bool network = false,
    bool filter = false,
  }) {
    final routing = ['excluded_sites', 'excluded_networks'].contains(field);
    final list = List<String>.from(
      routing ? config[field] ?? [] : protection[field] ?? [],
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(child: Text(title, style: const TextStyle(fontSize: 18))),
            IconButton(
              tooltip: 'Добавить: $title',
              onPressed: busy || !loaded
                  ? null
                  : () => addRule(field, network: network, filter: filter),
              icon: const Icon(Icons.add),
            ),
          ],
        ),
        if (list.isEmpty)
          const Padding(
            padding: EdgeInsets.only(bottom: 12),
            child: Text('Список пуст'),
          ),
        for (final value in list)
          ListTile(
            contentPadding: EdgeInsets.zero,
            title: SelectableText(value),
            trailing: IconButton(
              tooltip: 'Удалить $value',
              icon: const Icon(Icons.close),
              onPressed: busy
                  ? null
                  : () {
                      list.remove(value);
                      if (routing) {
                        setState(() => config[field] = list);
                      } else {
                        setProtection(field, list);
                      }
                    },
            ),
          ),
        const SizedBox(height: 12),
      ],
    );
  }

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Text('Настройки', style: Theme.of(context).textTheme.headlineLarge),
      const SizedBox(height: 10),
      const Text(
        'Политики маршрутизации и защиты. VPN runtime пока не применяет конфигурацию.',
      ),
      const SizedBox(height: 24),
      const BackgroundSetting(),
      const ListTile(
        contentPadding: EdgeInsets.zero,
        leading: Icon(Icons.info_outline),
        title: Text('Настроено ≠ применяется к трафику'),
        subtitle: Text(
          'Фильтрация ещё не подключена к туннелю. VPN runtime не применяет маршруты, DNS или правила защиты.',
        ),
      ),
      if (busy) const LinearProgressIndicator(),
      if (error != null)
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 16),
          child: Text(
            error!,
            style: const TextStyle(color: Colors.orangeAccent),
          ),
        ),
      if (message != null)
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 16),
          child: Semantics(liveRegion: true, child: Text(message!)),
        ),
      if (!loaded)
        TextButton.icon(
          onPressed: busy ? null : fetch,
          icon: const Icon(Icons.refresh),
          label: const Text('Загрузить настройки'),
        ),
      const SizedBox(height: 24),
      Text('Маршрутизация', style: Theme.of(context).textTheme.headlineMedium),
      SwitchListTile(
        contentPadding: EdgeInsets.zero,
        title: const Text('Не туннелировать LAN'),
        subtitle: const Text(
          'Локальные сети обходят VPN. CGNAT и произвольные публичные сети не считаются LAN.',
        ),
        value: config['bypass_lan'] == true,
        onChanged: busy || !loaded
            ? null
            : (v) => setState(() => config['bypass_lan'] = v),
      ),
      const Text(
        'Трафик исключённых сайтов, IP и подсетей намеренно пойдёт напрямую, вне VPN.',
      ),
      const SizedBox(height: 16),
      rules('Исключённые сайты', 'excluded_sites'),
      rules('IP и подсети', 'excluded_networks', network: true),
      const Divider(height: 40),
      Text('TAVRA Shield', style: Theme.of(context).textTheme.headlineMedium),
      const SizedBox(height: 10),
      const Text(
        'Политики защиты. DNS-блокировка не удаляет элементы страниц, встроенную рекламу и всю видеорекламу.',
      ),
      for (final entry in {
        'ad_blocking': 'Блокировка рекламы',
        'trackers': 'Защита от трекеров',
        'malware': 'Вредоносные и фишинговые домены',
        'annoyances': 'Раздражающие элементы и cookie-баннеры',
      }.entries)
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          value: protection[entry.key] == true,
          title: Text(entry.value),
          onChanged: busy || !loaded
              ? null
              : (v) => setProtection(entry.key, v),
        ),
      const SizedBox(height: 16),
      TextFormField(
        key: ValueKey('dns-${protection['dns_server']}'),
        initialValue: protection['dns_server'] ?? '',
        decoration: const InputDecoration(
          labelText: 'DNS upstream (HTTPS DoH / TLS DoT)',
        ),
        enabled: !busy && loaded,
        onChanged: (v) {
          final p = protection;
          p['dns_server'] = v;
          config['protection'] = p;
        },
      ),
      const SizedBox(height: 16),
      rules('Списки DNS фильтров', 'filter_lists', filter: true),
      rules('Разрешённые домены', 'allow_domains'),
      rules('Заблокированные домены', 'block_domains'),
      const ListTile(
        contentPadding: EdgeInsets.zero,
        leading: Icon(Icons.bar_chart_outlined),
        title: Text('Статистика недоступна'),
        subtitle: Text(
          'Заблокированные запросы не измеряются. Журнал DNS и сбор истории выключены. Обновление фильтров и применение правил ожидают runtime.',
        ),
      ),
      const SizedBox(height: 24),
      FilledButton.icon(
        onPressed: busy || !loaded ? null : save,
        icon: const Icon(Icons.save_outlined),
        label: const Text('Сохранить настройки'),
      ),
      const SizedBox(height: 24),
    ],
  );
}
