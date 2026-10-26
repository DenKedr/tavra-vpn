import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'api.dart';
import 'settings.dart';
import 'dialog_fields.dart';

const navy = Color(0xff071a3b),
    ivory = Color(0xfff4eddc),
    aqua = Color(0xff00d8cc);
void main() => runApp(const TavraApp());

class TavraApp extends StatelessWidget {
  final TavraApi? initialApi;
  const TavraApp({super.key, this.initialApi});
  @override
  Widget build(BuildContext context) => MaterialApp(
    title: 'TAVRA VPN',
    debugShowCheckedModeBanner: false,
    theme: ThemeData(
      useMaterial3: true,
      brightness: Brightness.dark,
      scaffoldBackgroundColor: navy,
      colorScheme: const ColorScheme.dark(
        primary: aqua,
        onPrimary: navy,
        surface: navy,
        onSurface: ivory,
      ),
      textTheme: const TextTheme(
        headlineLarge: TextStyle(
          fontSize: 36,
          fontWeight: FontWeight.w600,
          color: ivory,
        ),
        headlineMedium: TextStyle(
          fontSize: 26,
          fontWeight: FontWeight.w600,
          color: ivory,
        ),
      ),
      inputDecorationTheme: const InputDecorationTheme(
        border: OutlineInputBorder(),
        labelStyle: TextStyle(color: ivory),
        floatingLabelStyle: TextStyle(color: ivory),
        hintStyle: TextStyle(color: Color(0xffc5c4c0)),
        helperStyle: TextStyle(color: Color(0xffc5c4c0)),
        iconColor: ivory,
        prefixIconColor: ivory,
        suffixIconColor: ivory,
        errorStyle: TextStyle(color: Color(0xffffb4ab)),
        enabledBorder: OutlineInputBorder(
          borderSide: BorderSide(color: Color(0xff8d98ab)),
        ),
        focusedBorder: OutlineInputBorder(
          borderSide: BorderSide(color: aqua, width: 2),
        ),
        errorBorder: OutlineInputBorder(
          borderSide: BorderSide(color: Color(0xffffb4ab), width: 1.5),
        ),
        focusedErrorBorder: OutlineInputBorder(
          borderSide: BorderSide(color: Color(0xffffb4ab), width: 2),
        ),
        contentPadding: EdgeInsets.all(16),
      ),
      dividerColor: ivory.withValues(alpha: .16),
    ),
    home: Workspace(initialApi: initialApi),
  );
}

class BridgeMark extends CustomPainter {
  const BridgeMark();
  @override
  void paint(Canvas canvas, Size size) {
    final p = Paint()
      ..color = aqua
      ..strokeWidth = 4
      ..style = PaintingStyle.stroke
      ..strokeCap = StrokeCap.round;
    canvas.drawLine(
      Offset(4, size.height * .3),
      Offset(size.width - 4, size.height * .3),
      p,
    );
    canvas.drawLine(
      Offset(size.width * .5, size.height * .3),
      Offset(size.width * .5, size.height - 5),
      p,
    );
    canvas.drawArc(
      Rect.fromLTWH(4, size.height * .1, size.width - 8, size.height * .6),
      0,
      3.14159,
      false,
      p,
    );
    for (final x in [4.0, size.width - 4]) {
      canvas.drawCircle(Offset(x, size.height * .3), 3, Paint()..color = ivory);
    }
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) => false;
}

class Workspace extends StatefulWidget {
  final TavraApi? initialApi;
  const Workspace({super.key, this.initialApi});
  @override
  State<Workspace> createState() => _WorkspaceState();
}

class _WorkspaceState extends State<Workspace> {
  final url = TextEditingController(text: 'http://127.0.0.1:8787'),
      key = TextEditingController(),
      bootstrap = TextEditingController(),
      invitation = TextEditingController();
  final form = GlobalKey<FormState>();
  TavraApi? api;
  bool clientMode = false, firstOwner = false, busy = false;
  int page = 0;
  String? error;
  dynamic data;
  Map<String, dynamic>? registration;
  final labels = [
    'Обзор',
    'Серверы',
    'Пользователи',
    'Подписки',
    'Устройства',
    'Настройки',
  ];
  final icons = [
    Icons.space_dashboard_outlined,
    Icons.dns_outlined,
    Icons.people_outline,
    Icons.key_outlined,
    Icons.devices,
    Icons.tune,
  ];
  void sessionExpired() {
    if (mounted)
      setState(() {
        data = null;
        page = 0;
        clientMode = false;
        error = 'Сессия истекла. Войдите снова.';
      });
  }

  void logout() {
    api?.token = null;
    setState(() {
      data = null;
      page = 0;
      clientMode = false;
      error = null;
    });
  }

  @override
  void initState() {
    super.initState();
    api = widget.initialApi;
    if (api != null) api!.onUnauthorized = sessionExpired;
    if (api?.token != null) Future.microtask(() => perform(load));
  }

  @override
  void dispose() {
    for (final c in [url, key, bootstrap, invitation]) {
      c.dispose();
    }
    api?.close();
    super.dispose();
  }

  Future<void> perform(Future<void> Function() action) async {
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

  Future<void> load() async {
    if (api?.token == null || page == 5) return;
    final paths = [
      '/overview',
      '/servers',
      '/users',
      '/subscriptions',
      '/devices',
    ];
    final result = await api!.request('GET', paths[page]);
    if (mounted) setState(() => data = result);
  }

  Future<void> login() async {
    if (!form.currentState!.validate()) return;
    await perform(() async {
      api?.close();
      api = TavraApi(url.text)..onUnauthorized = sessionExpired;
      await api!.login(key.text, bootstrap: firstOwner ? bootstrap.text : null);
      key.clear();
      bootstrap.clear();
      await load();
    });
  }

  Future<void> choose(int value) async {
    setState(() {
      page = value;
      data = null;
      error = null;
    });
    await perform(load);
  }

  void notice(String text) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(text)));
  }

  Future<bool> confirm(String title, String message) async =>
      await showDialog<bool>(
        context: context,
        builder: (c) => AlertDialog(
          title: Text(title),
          content: Text(message),
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
      ) ??
      false;
  Future<void> mutate(
    String method,
    String path, [
    Map<String, dynamic>? body,
  ]) async {
    await perform(() async {
      final result = await api!.request(method, path, body);
      if (result is Map && result['invitation'] != null && mounted)
        await showInvitation(result['invitation']);
      await load();
    });
  }

  Future<void> showInvitation(String value) async {
    await showDialog<void>(
      context: context,
      builder: (c) => AlertDialog(
        title: const Text('Одноразовый показ приглашения'),
        content: SizedBox(
          width: 480,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Сохраните код сейчас: повторно получить его нельзя. Ротация отзовёт прежний код и устройства.',
              ),
              const SizedBox(height: 16),
              SelectableText(value),
              const SizedBox(height: 16),
              const Text(
                'Приглашение регистрирует устройство. VPN lease и работающий туннель пока не выдаются.',
              ),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () async {
              await Clipboard.setData(ClipboardData(text: value));
              notice('Код скопирован. Буфер обмена содержит секрет.');
            },
            child: const Text('Копировать'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(c),
            child: const Text('Сохранено'),
          ),
        ],
      ),
    );
  }

  Future<void> edit(String kind, [Map<String, dynamic>? row]) async {
    final fields = kind == 'server'
        ? {
            'name': 'Название',
            'host': 'IP или имя хоста',
            'ssh_port': 'SSH порт',
            'username': 'SSH пользователь',
          }
        : kind == 'user'
        ? {
            'first_name': 'Имя',
            'last_name': 'Фамилия',
            'phone': 'Телефон',
            'email': 'Email',
          }
        : {
            'user_id': 'ID пользователя',
            'expires_at': 'Истекает: Unix секунды (0 — бессрочно)',
            'device_limit': 'Лимит устройств (0 — без лимита)',
          };
    final initialValues = {
      for (final entry in fields.entries)
        entry.key:
            row?[entry.key]?.toString() ??
            (entry.key == 'ssh_port'
                ? '22'
                : entry.key == 'username'
                ? 'root'
                : ['expires_at', 'device_limit'].contains(entry.key)
                ? '0'
                : ''),
    };
    final dialogForm = GlobalKey<FormState>();
    final result = await showDialog<Map<String, dynamic>>(
      context: context,
      builder: (_) => DialogFields(
        initialValues: initialValues,
        builder: (c, controllers) => AlertDialog(
          title: Text(
            row == null
                ? 'Добавить ${kind == 'server'
                      ? 'сервер'
                      : kind == 'user'
                      ? 'пользователя'
                      : 'подписку'}'
                : 'Изменить пользователя',
          ),
          content: SizedBox(
            width: 500,
            child: SingleChildScrollView(
              child: Form(
                key: dialogForm,
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (kind == 'server')
                      const Padding(
                        padding: EdgeInsets.only(bottom: 16),
                        child: Text(
                          'Сохраняется только запись. SSH-подключение и установка не запускаются.',
                        ),
                      ),
                    ...fields.entries.map(
                      (e) => Padding(
                        padding: const EdgeInsets.only(bottom: 14),
                        child: TextFormField(
                          style: const TextStyle(color: ivory),
                          controller: controllers[e.key],
                          decoration: InputDecoration(labelText: e.value),
                          keyboardType:
                              [
                                'ssh_port',
                                'expires_at',
                                'device_limit',
                              ].contains(e.key)
                              ? TextInputType.number
                              : TextInputType.text,
                          validator: (v) {
                            v = v ?? '';
                            if ([
                              'ssh_port',
                              'expires_at',
                              'device_limit',
                            ].contains(e.key)) {
                              final n = int.tryParse(v);
                              if (n == null ||
                                  n < 0 ||
                                  (e.key == 'ssh_port' &&
                                      (n < 1 || n > 65535)) ||
                                  (e.key == 'device_limit' && n > 1000000))
                                return 'Укажите допустимое целое число';
                            } else if (kind == 'server') {
                              if (v.trim().isEmpty) return 'Заполните поле';
                              if (e.key == 'name' && v.runes.length > 120)
                                return 'Максимум 120 символов';
                              if (e.key == 'host' &&
                                  (!RegExp(
                                        r'^[a-zA-Z0-9.\-:\[\]]+$',
                                      ).hasMatch(v) ||
                                      v.length > 253))
                                return 'Укажите IP или имя хоста';
                              if (e.key == 'username' &&
                                  (!RegExp(r'^[a-zA-Z0-9_-]+$').hasMatch(v) ||
                                      v.length > 64))
                                return 'Неверное имя SSH пользователя';
                            } else if (e.key == 'user_id' &&
                                !RegExp(
                                  r'^[a-fA-F0-9]{8}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{12}$',
                                ).hasMatch(v))
                              return 'Укажите UUID пользователя';
                            else if (kind == 'user' &&
                                (v.runes.length >
                                        (e.key == 'email'
                                            ? 254
                                            : e.key == 'phone'
                                            ? 64
                                            : 120) ||
                                    RegExp(r'[\x00-\x1f\x7f]').hasMatch(v)))
                              return 'Поле превышает лимит или содержит управляющие символы';
                            return null;
                          },
                        ),
                      ),
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
                if (dialogForm.currentState!.validate())
                  Navigator.pop(c, {
                    for (final e in controllers.entries)
                      e.key:
                          [
                            'ssh_port',
                            'expires_at',
                            'device_limit',
                          ].contains(e.key)
                          ? int.parse(e.value.text)
                          : e.value.text.trim(),
                  });
              },
              child: const Text('Сохранить'),
            ),
          ],
        ),
      ),
    );
    if (result != null && mounted)
      await mutate(
        row == null ? 'POST' : 'PATCH',
        kind == 'server'
            ? '/servers'
            : kind == 'user'
            ? '/users${row == null ? '' : '/${row['id']}'}'
            : '/subscriptions',
        result,
      );
  }

  @override
  Widget build(BuildContext context) {
    final logged = api?.token != null;
    return LayoutBuilder(
      builder: (context, constraints) {
        final wide = constraints.maxWidth >= 850;
        return Scaffold(
          appBar: AppBar(
            title: logged && constraints.maxWidth < 550
                ? const SizedBox.shrink()
                : Row(
                    children: [
                      const SizedBox(
                        width: 34,
                        height: 34,
                        child: CustomPaint(painter: BridgeMark()),
                      ),
                      if (constraints.maxWidth >= 550)
                        const SizedBox(width: 14),
                      if (constraints.maxWidth >= 550)
                        const Text(
                          'TAVRA',
                          style: TextStyle(
                            fontWeight: FontWeight.w700,
                            letterSpacing: 3,
                          ),
                        ),
                    ],
                  ),
            actions: [
              if (logged)
                IconButton(
                  tooltip: 'Выйти',
                  onPressed: busy ? null : logout,
                  icon: const Icon(Icons.logout),
                ),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 12),
                child: SegmentedButton<bool>(
                  showSelectedIcon: false,
                  segments: const [
                    ButtonSegment(value: false, label: Text('Админ')),
                    ButtonSegment(value: true, label: Text('Клиент')),
                  ],
                  selected: {clientMode},
                  onSelectionChanged: busy
                      ? null
                      : (s) => setState(() {
                          clientMode = s.first;
                          error = null;
                        }),
                ),
              ),
            ],
          ),
          bottomNavigationBar: !wide && logged && !clientMode
              ? NavigationBar(
                  selectedIndex: page,
                  onDestinationSelected: busy ? null : choose,
                  destinations: List.generate(
                    labels.length,
                    (i) => NavigationDestination(
                      icon: Icon(icons[i]),
                      label: labels[i],
                    ),
                  ),
                )
              : null,
          body: Row(
            children: [
              if (wide && logged && !clientMode)
                NavigationRail(
                  extended: true,
                  selectedIndex: page,
                  onDestinationSelected: busy ? null : choose,
                  destinations: List.generate(
                    labels.length,
                    (i) => NavigationRailDestination(
                      icon: Icon(icons[i]),
                      label: Text(labels[i]),
                    ),
                  ),
                  trailing: Padding(
                    padding: const EdgeInsets.only(top: 32),
                    child: TextButton.icon(
                      onPressed: busy
                          ? null
                          : () {
                              api?.close();
                              setState(() {
                                api = null;
                                data = null;
                              });
                            },
                      icon: const Icon(Icons.logout),
                      label: const Text('Выйти'),
                    ),
                  ),
                ),
              Expanded(
                child: Column(
                  children: [
                    if (busy) const LinearProgressIndicator(minHeight: 2),
                    Expanded(
                      child: SingleChildScrollView(
                        padding: EdgeInsets.all(wide ? 36 : 20),
                        child: Center(
                          child: ConstrainedBox(
                            constraints: const BoxConstraints(maxWidth: 1100),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                if (error != null)
                                  Container(
                                    width: double.infinity,
                                    padding: const EdgeInsets.all(16),
                                    margin: const EdgeInsets.only(bottom: 24),
                                    decoration: BoxDecoration(
                                      color: Colors.red.withValues(alpha: .16),
                                      borderRadius: BorderRadius.circular(12),
                                    ),
                                    child: Semantics(
                                      liveRegion: true,
                                      child: Text(error!),
                                    ),
                                  ),
                                if (clientMode)
                                  clientView()
                                else if (!logged)
                                  loginView()
                                else
                                  adminView(),
                              ],
                            ),
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        );
      },
    );
  }

  Widget heading(String title, String subtitle, {Widget? action}) => Padding(
    padding: const EdgeInsets.only(bottom: 28),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Wrap(
          spacing: 24,
          runSpacing: 12,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            Text(title, style: Theme.of(context).textTheme.headlineLarge),
            if (action != null) action,
          ],
        ),
        const SizedBox(height: 10),
        Text(
          subtitle,
          style: TextStyle(color: ivory.withValues(alpha: .7), height: 1.5),
        ),
      ],
    ),
  );
  Widget loginView() => ConstrainedBox(
    constraints: const BoxConstraints(maxWidth: 520),
    child: Form(
      key: form,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          heading(
            'Ваш маршрут.\nВаше управление.',
            'TAVRA Admin · среда разработки\nПодключитесь к собственному серверу управления.',
          ),
          TextFormField(
            style: const TextStyle(color: ivory),
            controller: url,
            decoration: const InputDecoration(labelText: 'Адрес API'),
            validator: (v) {
              try {
                TavraApi.validateUrl(v ?? '');
                return null;
              } catch (e) {
                return e.toString();
              }
            },
          ),
          const SizedBox(height: 18),
          TextFormField(
            style: const TextStyle(color: ivory),
            controller: key,
            obscureText: true,
            decoration: InputDecoration(
              labelText: 'Ключ администратора',
              suffixIcon: IconButton(
                tooltip: 'Копировать ключ',
                onPressed: () async {
                  if (key.text.isNotEmpty) {
                    await Clipboard.setData(ClipboardData(text: key.text));
                    notice(
                      'Ключ скопирован. Сохраните его в менеджере паролей.',
                    );
                  }
                },
                icon: const Icon(Icons.copy),
              ),
            ),
            validator: (v) => adminKeyError(v ?? ''),
          ),
          const SizedBox(height: 8),
          TextButton.icon(
            onPressed: busy
                ? null
                : () {
                    key.text = generateSecret();
                    notice(
                      'Новый ключ создан. Сохраните его в менеджере паролей до активации.',
                    );
                  },
            icon: const Icon(Icons.auto_awesome_outlined),
            label: const Text('Создать новый ключ владельца'),
          ),
          CheckboxListTile(
            contentPadding: EdgeInsets.zero,
            value: firstOwner,
            onChanged: busy ? null : (v) => setState(() => firstOwner = v!),
            title: const Text('Первичная активация владельца'),
          ),
          if (firstOwner)
            Padding(
              padding: const EdgeInsets.only(bottom: 18),
              child: TextFormField(
                style: const TextStyle(color: ivory),
                controller: bootstrap,
                obscureText: true,
                decoration: const InputDecoration(
                  labelText: 'Bootstrap secret сервера',
                ),
                validator: (v) =>
                    v == null || v.isEmpty ? 'Укажите bootstrap secret' : null,
              ),
            ),
          FilledButton.icon(
            onPressed: busy ? null : login,
            icon: const Icon(Icons.login),
            label: const Text('Войти'),
          ),
          const SizedBox(height: 24),
          const Text(
            'Ключ и сессия хранятся только в памяти. После закрытия приложения войдите заново. Новый ключ нужен только для первичной активации; при обычном входе используйте сохранённый.',
          ),
        ],
      ),
    ),
  );
  Widget adminView() {
    if (page == 5) return SettingsView(api: api!);
    final title = labels[page];
    if (page == 0)
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          heading(
            'Сеть под вашим контролем',
            'Фактические записи сервера управления. Показатели трафика пока недоступны.',
            action: IconButton(
              tooltip: 'Обновить',
              onPressed: busy ? null : () => perform(load),
              icon: const Icon(Icons.refresh),
            ),
          ),
          if (data != null)
            Wrap(
              spacing: 36,
              runSpacing: 24,
              children: [
                for (final item in [
                  ('users', 'Пользователи'),
                  ('subscriptions', 'Подписки'),
                  ('devices', 'Устройства'),
                  ('servers', 'Серверы'),
                ])
                  SizedBox(
                    width: 160,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          '${data[item.$1]}',
                          style: const TextStyle(
                            fontSize: 48,
                            fontWeight: FontWeight.w300,
                          ),
                        ),
                        Text(item.$2),
                      ],
                    ),
                  ),
              ],
            ),
          const SizedBox(height: 44),
          const Divider(),
          const SizedBox(height: 24),
          const ListTile(
            contentPadding: EdgeInsets.zero,
            leading: Icon(Icons.lock_outline, color: aqua),
            title: Text('VPN ещё не развёрнут'),
            subtitle: Text(
              'Сначала добавьте серверы и пользователей. Inventory и регистрация устройств не создают VPN-туннель.',
            ),
          ),
          const SizedBox(height: 18),
          const FilledButton(onPressed: null, child: Text('Подключить VPN')),
        ],
      );
    final list = data is List ? data as List : [];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        heading(
          title,
          page == 1
              ? 'Учёт серверов. SSH и установка ещё не подключены.'
              : page == 2
              ? 'Контакты, доступ и подписки пользователей.'
              : page == 3
              ? 'Приглашения и ограничения регистрации устройств.'
              : 'Зарегистрированные ключи устройств. Туннельные сессии не измеряются.',
          action: Wrap(
            spacing: 12,
            children: [
              IconButton(
                tooltip: 'Обновить',
                onPressed: busy ? null : () => perform(load),
                icon: const Icon(Icons.refresh),
              ),
              if (page < 4)
                FilledButton.icon(
                  onPressed: busy
                      ? null
                      : () => edit(
                          page == 1
                              ? 'server'
                              : page == 2
                              ? 'user'
                              : 'subscription',
                        ),
                  icon: const Icon(Icons.add),
                  label: const Text('Добавить'),
                ),
            ],
          ),
        ),
        if (list.isEmpty && !busy)
          const Padding(
            padding: EdgeInsets.symmetric(vertical: 32),
            child: Text(
              'Пока нет записей. Добавьте первую запись или обновите список.',
            ),
          ),
        for (final raw in list) record(Map<String, dynamic>.from(raw)),
      ],
    );
  }

  Widget record(Map<String, dynamic> row) {
    final id = row['id'];
    final status = row['status'];
    String title = page == 1
        ? '${row['name']}'
        : page == 2
        ? '${row['first_name']} ${row['last_name']}'
        : '$id';
    if (title.trim().isEmpty) title = 'Без имени';
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SelectableText(
                title,
                style: const TextStyle(
                  fontSize: 20,
                  fontWeight: FontWeight.w500,
                ),
              ),
              const SizedBox(height: 8),
              SelectableText(
                page == 1
                    ? '${row['username']}@${row['host']}:${row['ssh_port']}\nНе установлен · доступность и нагрузка неизвестны'
                    : page == 2
                    ? '${row['email']}  ${row['phone']}\n${status == 'active' ? 'Доступ разрешён' : 'Заблокирован'}\nID: $id'
                    : page == 3
                    ? 'Пользователь: ${row['user_id']}\n${statusLabel(status)} · срок: ${row['expires_at'] == 0 ? 'бессрочно' : DateTime.fromMillisecondsSinceEpoch((row['expires_at'] as int) * 1000).toLocal().toString()}\nЛимит устройств: ${row['device_limit'] == 0 ? 'без лимита' : row['device_limit']}'
                    : 'Подписка: ${row['subscription_id']}\n${statusLabel(status)}\nПубличный ключ: ${row['public_key']}',
              ),
              const SizedBox(height: 10),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  if (page == 2) ...[
                    TextButton(
                      onPressed: busy ? null : () => edit('user', row),
                      child: const Text('Изменить'),
                    ),
                    TextButton(
                      onPressed: busy
                          ? null
                          : () async {
                              if (await confirm(
                                'Изменить доступ?',
                                'Блокировка отзовёт зарегистрированные устройства пользователя.',
                              ))
                                await mutate('PATCH', '/users/$id', {
                                  'status': status == 'active'
                                      ? 'blocked'
                                      : 'active',
                                });
                            },
                      child: Text(
                        status == 'active' ? 'Заблокировать' : 'Разблокировать',
                      ),
                    ),
                    TextButton(
                      onPressed: busy
                          ? null
                          : () async {
                              if (await confirm(
                                'Удалить пользователя?',
                                'Будут удалены его подписки и устройства. Это действие нельзя отменить.',
                              ))
                                await mutate('DELETE', '/users/$id');
                            },
                      child: const Text('Удалить'),
                    ),
                  ],
                  if (page == 3) ...[
                    TextButton(
                      onPressed: busy || status == 'revoked'
                          ? null
                          : () async {
                              if (await confirm(
                                'Заменить приглашение?',
                                'Прежнее приглашение и устройства будут отозваны.',
                              ))
                                await mutate(
                                  'POST',
                                  '/subscriptions/$id/rotate',
                                );
                            },
                      child: const Text('Заменить код'),
                    ),
                    TextButton(
                      onPressed: busy || status == 'revoked'
                          ? null
                          : () async {
                              if (await confirm(
                                'Отозвать подписку?',
                                'Приглашение и зарегистрированные устройства потеряют доступ.',
                              ))
                                await mutate(
                                  'POST',
                                  '/subscriptions/$id/revoke',
                                );
                            },
                      child: const Text('Отозвать'),
                    ),
                  ],
                  if (page == 4)
                    TextButton(
                      onPressed: busy || status == 'revoked'
                          ? null
                          : () async {
                              if (await confirm(
                                'Отозвать устройство?',
                                'Повторная регистрация с этим ключом будет запрещена.',
                              ))
                                await mutate('DELETE', '/devices/$id');
                            },
                      child: const Text('Отозвать устройство'),
                    ),
                ],
              ),
            ],
          ),
        ),
        const Divider(height: 1),
      ],
    );
  }

  String statusLabel(dynamic s) => switch (s) {
    'active' => 'Активна',
    'blocked' => 'Заблокирована',
    'revoked' => 'Отозвана',
    _ => '$s',
  };
  Widget clientView() => ConstrainedBox(
    constraints: const BoxConstraints(maxWidth: 650),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        heading('Защищённый маршрут', 'TAVRA Client · среда разработки'),
        const Icon(Icons.route_outlined, size: 76, color: aqua),
        const SizedBox(height: 24),
        Text(
          registration == null
              ? 'Нет VPN-подключения'
              : 'Устройство зарегистрировано без VPN',
          style: Theme.of(context).textTheme.headlineMedium,
        ),
        const SizedBox(height: 14),
        const Text(
          'Туннель и подписанный VPN lease ещё недоступны. Ваш трафик не изменяется.',
        ),
        const SizedBox(height: 20),
        const FilledButton(onPressed: null, child: Text('Подключить VPN')),
        const SizedBox(height: 32),
        const Divider(),
        const SizedBox(height: 24),
        Text(
          'Импорт приглашения',
          style: Theme.of(context).textTheme.headlineMedium,
        ),
        const SizedBox(height: 18),
        TextField(
          style: const TextStyle(color: ivory),
          controller: url,
          decoration: const InputDecoration(labelText: 'Адрес API'),
        ),
        const SizedBox(height: 16),
        TextField(
          style: const TextStyle(color: ivory),
          controller: invitation,
          obscureText: true,
          decoration: const InputDecoration(labelText: 'Код приглашения'),
        ),
        const SizedBox(height: 16),
        FilledButton(
          onPressed: busy
              ? null
              : () => perform(() async {
                  if (api == null ||
                      api!.base.toString() !=
                          TavraApi.validateUrl(url.text).toString()) {
                    api?.close();
                    api = TavraApi(url.text)..onUnauthorized = sessionExpired;
                  }
                  final result = await api!.enroll(invitation.text.trim());
                  invitation.clear();
                  if (mounted) setState(() => registration = result);
                }),
          child: const Text('Зарегистрировать устройство'),
        ),
        const SizedBox(height: 18),
        const Text(
          'Локальный Ed25519 ключ хранится только в памяти до закрытия приложения. Постоянное защищённое хранилище ещё не подключено. При перезапуске новый ключ займёт новый слот устройства.',
        ),
        if (registration != null) ...[
          const SizedBox(height: 18),
          SelectableText(
            'Статус: registered_without_vpn\nУстройство: ${registration!['device_id']}\nПодписка: ${registration!['subscription_id']}',
          ),
        ],
        const SizedBox(height: 28),
        TextButton.icon(
          onPressed: () => Navigator.of(context).push(
            MaterialPageRoute<void>(
              builder: (context) => Scaffold(
                appBar: AppBar(title: const Text('Настройки устройства')),
                body: const SingleChildScrollView(
                  padding: EdgeInsets.all(24),
                  child: SettingsView(local: true),
                ),
              ),
            ),
          ),
          icon: const Icon(Icons.tune),
          label: const Text('Маршрутизация и защита'),
        ),
        const ListTile(
          contentPadding: EdgeInsets.zero,
          leading: Icon(Icons.shield_outlined),
          title: Text('TrustTunnel (протокол AdGuard VPN)'),
          subtitle: Text(
            'Движок ещё не подключён. Выбор протокола недоступен.',
          ),
        ),
      ],
    ),
  );
}
