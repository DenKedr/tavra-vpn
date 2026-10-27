import 'dart:io';
import 'package:flutter/material.dart';
import 'api.dart';
import 'dialog_fields.dart';

class DomainsView extends StatefulWidget {
  final TavraApi api;
  const DomainsView({super.key, required this.api});
  @override
  State<DomainsView> createState() => _DomainsViewState();
}

class _DomainsViewState extends State<DomainsView> {
  List<Map<String, dynamic>> domains = [], servers = [];
  bool busy = true;
  String? error;
  @override
  void initState() {
    super.initState();
    run(refresh);
  }

  Future<void> run(Future<void> Function() action) async {
    setState(() {
      busy = true;
      error = null;
    });
    try {
      await action();
    } catch (e) {
      if (mounted)
        setState(
          () => error = e is ApiFailure
              ? e.message
              : 'Не удалось обработать ответ API.',
        );
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  Future<void> refresh() async {
    final results = await Future.wait([
      widget.api.request('GET', '/domains'),
      widget.api.request('GET', '/servers'),
    ]);
    if (mounted)
      setState(() {
        domains = [for (final r in results[0]) Map<String, dynamic>.from(r)];
        servers = [for (final r in results[1]) Map<String, dynamic>.from(r)];
      });
  }

  Future<void> edit([Map<String, dynamic>? row]) async {
    var selected = row?['server_id'] as String? ?? '';
    final form = GlobalKey<FormState>();
    if (selected.isNotEmpty && !servers.any((s) => s['id'] == selected)) {
      await run(refresh);
      return;
    }
    final result = await showDialog<Map<String, dynamic>>(
      context: context,
      builder: (_) => DialogFields(
        initialValues: {'name': row?['name'] as String? ?? ''},
        builder: (c, controllers) => StatefulBuilder(
          builder: (c, setDialogState) => AlertDialog(
            title: Text(row == null ? 'Добавить домен' : 'Изменить домен'),
            content: SizedBox(
              width: 500,
              child: SingleChildScrollView(
                child: Form(
                  key: form,
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      const Text(
                        'Сохраняется только запись и назначение серверу. DNS, сертификат TLS и готовность протоколов не проверяются.',
                      ),
                      const SizedBox(height: 18),
                      TextFormField(
                        controller: controllers['name'],
                        decoration: const InputDecoration(labelText: 'Домен'),
                        validator: (v) {
                          final value = (v ?? '').trim();
                          final labels = value
                              .replaceFirst(RegExp(r'\.$'), '')
                              .split('.');
                          if (value.isEmpty ||
                              value.length > 1024 ||
                              InternetAddress.tryParse(value) != null ||
                              labels.length < 2 ||
                              labels.any(
                                (l) =>
                                    l.isEmpty ||
                                    l.startsWith('-') ||
                                    l.endsWith('-'),
                              ) ||
                              RegExp(r'[\s/@:*?#\[\]\\]').hasMatch(value))
                            return 'Укажите домен без URL, IP, wildcard или пароля.';
                          return null;
                        },
                      ),
                      const SizedBox(height: 18),
                      DropdownButtonFormField<String>(
                        initialValue: selected,
                        isExpanded: true,
                        decoration: const InputDecoration(labelText: 'Сервер'),
                        items: [
                          const DropdownMenuItem(
                            value: '',
                            child: Text('Не назначен'),
                          ),
                          for (final s in servers)
                            DropdownMenuItem(
                              value: s['id'] as String,
                              child: Text('${s['name']}'),
                            ),
                        ],
                        onChanged: (v) =>
                            setDialogState(() => selected = v ?? ''),
                      ),
                    ],
                  ),
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
                  if (form.currentState!.validate())
                    Navigator.pop(c, {
                      'name': controllers['name']!.text.trim(),
                      'server_id': selected.isEmpty ? null : selected,
                    });
                },
                child: const Text('Сохранить'),
              ),
            ],
          ),
        ),
      ),
    );
    if (result == null || !mounted) return;
    await run(() async {
      await widget.api.request(
        row == null ? 'POST' : 'PATCH',
        '/domains${row == null ? '' : '/${row['id']}'}',
        result,
      );
      await refresh();
    });
  }

  Future<void> delete(Map<String, dynamic> row) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (c) => AlertDialog(
        title: const Text('Удалить домен?'),
        content: Text(
          'Запись ${row['name']} будет удалена. DNS-зона и сертификаты на сервере не изменятся.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(c, false),
            child: const Text('Отмена'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(c, true),
            child: const Text('Подтвердить'),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    await run(() async {
      await widget.api.request('DELETE', '/domains/${row['id']}');
      await refresh();
    });
  }

  String serverLabel(dynamic id) {
    if (id == null) return 'Не назначен';
    for (final server in servers) {
      if (server['id'] == id) return '${server['name']}';
    }
    return '$id';
  }

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Wrap(
        spacing: 24,
        runSpacing: 12,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          Text('Домены', style: Theme.of(context).textTheme.headlineLarge),
          IconButton(
            tooltip: 'Обновить домены',
            onPressed: busy ? null : () => run(refresh),
            icon: const Icon(Icons.refresh),
          ),
          FilledButton.icon(
            onPressed: busy ? null : edit,
            icon: const Icon(Icons.add),
            label: const Text('Добавить домен'),
          ),
        ],
      ),
      const SizedBox(height: 16),
      const Text(
        'Домен в списке не означает рабочий DNS, TLS или готовность VPN. Проверка ещё не подключена.',
      ),
      const SizedBox(height: 24),
      if (busy) const LinearProgressIndicator(),
      if (error != null)
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 16),
          child: Semantics(
            liveRegion: true,
            child: Text(
              error!,
              style: const TextStyle(color: Color(0xffffb4ab)),
            ),
          ),
        ),
      if (domains.isEmpty && !busy)
        const Padding(
          padding: EdgeInsets.symmetric(vertical: 24),
          child: Text(
            'Пока нет доменов. Добавьте запись и при необходимости назначьте сервер.',
          ),
        ),
      for (final row in domains)
        Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  SelectableText(
                    '${row['name']}',
                    style: const TextStyle(
                      fontSize: 20,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  const SizedBox(height: 8),
                  const Text('Не проверен · DNS / TLS неизвестны'),
                  Text('Сервер: ${serverLabel(row['server_id'])}'),
                  const SizedBox(height: 8),
                  Wrap(
                    spacing: 12,
                    children: [
                      TextButton(
                        onPressed: busy ? null : () => edit(row),
                        child: const Text('Изменить'),
                      ),
                      TextButton(
                        onPressed: busy ? null : () => delete(row),
                        child: const Text('Удалить домен'),
                      ),
                    ],
                  ),
                ],
              ),
            ),
            const Divider(height: 1),
          ],
        ),
    ],
  );
}
