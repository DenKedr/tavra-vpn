import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:tavra/main.dart';
import 'package:tavra/api.dart';

// Public credentials exclusively for the disposable tavra_test fixture.
const testKey = 'TAVRA_NATIVE_TEST_ONLY_ADMIN_KEY_0000000000';
void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets('native admin CRUD and persisted routing policies', (
    tester,
  ) async {
    Future<void> settle() async {
      await tester.pumpAndSettle();
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 350)),
      );
      await tester.pumpAndSettle();
    }

    Future<void> tap(Finder f) async {
      await tester.ensureVisible(f);
      await tester.tap(f);
      await settle();
    }

    Future<void> navigate(String label) => tap(
      find.descendant(
        of: find.byWidgetPredicate(
          (w) => w is NavigationRail || w is NavigationBar,
        ),
        matching: find.text(label),
      ),
    );
    final check = TavraApi('http://127.0.0.1:8790');
    addTearDown(check.close);
    Future<dynamic> read(String path) async {
      dynamic value;
      await tester.runAsync(() async {
        value = await check.request('GET', path);
      });
      return value;
    }

    Finder field(String label) => find.widgetWithText(TextFormField, label);
    await tester.binding.setSurfaceSize(const Size(320, 700));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(const TavraApp());
    await settle();
    expect(
      Theme.of(tester.element(find.byType(Scaffold))).brightness,
      Brightness.dark,
    );
    expect(tester.takeException(), isNull);
    await tester.binding.setSurfaceSize(const Size(1200, 900));
    await settle();
    await tester.enterText(field('Адрес API'), 'http://127.0.0.1:8790');
    await tester.enterText(field('Ключ администратора'), testKey);
    await tap(find.text('Войти'));
    expect(find.text('Сеть под вашим контролем'), findsOneWidget);
    expect(find.text('0'), findsNWidgets(4));
    await tap(
      find.descendant(
        of: find.byWidgetPredicate(
          (w) => w is NavigationRail || w is NavigationBar,
        ),
        matching: find.text('Пользователи'),
      ),
    );
    await tap(find.text('Добавить'));
    await tester.enterText(field('Имя'), 'Native');
    await tester.enterText(field('Фамилия'), 'Smoke');
    await tester.enterText(field('Email'), 'native-smoke@example.invalid');
    await tap(find.text('Сохранить'));
    expect(find.text('Native Smoke'), findsOneWidget);
    await tap(find.text('Заблокировать'));
    expect(find.text('Изменить доступ?'), findsOneWidget);
    await tap(find.text('Подтвердить'));
    expect(find.text('Разблокировать'), findsOneWidget);
    await tap(find.text('Удалить'));
    await tap(find.text('Отмена'));
    expect(find.text('Native Smoke'), findsOneWidget);
    await tap(find.text('Удалить'));
    await tap(find.text('Подтвердить'));
    expect(find.text('Native Smoke'), findsNothing);
    await tap(
      find.descendant(
        of: find.byWidgetPredicate(
          (w) => w is NavigationRail || w is NavigationBar,
        ),
        matching: find.text('Настройки'),
      ),
    );
    await tap(find.text('Не туннелировать LAN'));
    await tap(find.byTooltip('Добавить: Исключённые сайты'));
    await tester.enterText(
      field('Домен или HTTP(S) URL'),
      'https://example.org/path',
    );
    await tap(find.text('Добавить'));
    await tap(find.byTooltip('Добавить: IP и подсети'));
    await tester.enterText(field('IPv4, IPv6 или CIDR'), '10.23.0.0/16');
    await tap(find.text('Добавить'));
    await tap(find.text('Сохранить настройки'));
    expect(find.textContaining('Настройки сохранены.'), findsOneWidget);
    await tester.runAsync(() async {
      await check.login(testKey);
      final settings = await check.request('GET', '/settings');
      expect(settings['bypass_lan'], false);
      expect(settings['excluded_sites'], contains('example.org'));
      expect(settings['excluded_networks'], contains('10.23.0.0/16'));
      expect(await check.request('GET', '/users'), isEmpty);
    });
    expect(tester.takeException(), isNull);

    // Inventory changes never provision reserved-address hosts or alter DNS.
    await navigate('Серверы');
    await tap(find.text('Добавить'));
    await tester.enterText(field('Название'), 'Native inventory');
    await tester.enterText(field('IP или имя хоста'), '198.51.100.10');
    await tap(find.text('Сохранить'));
    expect(find.text('Native inventory'), findsOneWidget);
    await tap(find.text('Изменить'));
    await tester.enterText(field('Название'), 'Native edited');
    await tester.enterText(field('SSH порт'), '2222');
    await tap(find.text('Сохранить'));
    final servers = await read('/servers') as List;
    expect(servers, hasLength(1));
    final serverId = servers.single['id'];
    expect(servers.single['name'], 'Native edited');
    expect(servers.single['host'], '198.51.100.10');
    expect(servers.single['ssh_port'], 2222);
    expect(servers.single['status'], 'not_provisioned');

    await navigate('Домены');
    await tap(find.text('Добавить домен'));
    await tester.enterText(field('Домен'), 'NATIVE-SMOKE.EXAMPLE.');
    await tap(find.byType(DropdownButtonFormField<String>));
    await tap(find.text('Native edited').last);
    await tap(find.text('Сохранить'));
    expect(find.text('native-smoke.example'), findsOneWidget);
    var domains = await read('/domains') as List;
    expect(domains, hasLength(1));
    expect(domains.single['server_id'], serverId);
    expect(domains.single['verification_status'], 'unverified');

    await tap(find.text('Изменить'));
    await tester.enterText(field('Домен'), 'native-edited.example');
    await tap(find.byType(DropdownButtonFormField<String>));
    await tap(find.text('Не назначен').last);
    await tap(find.text('Сохранить'));
    domains = await read('/domains') as List;
    expect(domains.single['name'], 'native-edited.example');
    expect(domains.single['server_id'], isNull);
    expect(domains.single['verification_status'], 'unverified');
    expect(find.text('Сервер: Не назначен'), findsOneWidget);

    await tap(find.text('Изменить'));
    await tap(find.byType(DropdownButtonFormField<String>));
    await tap(find.text('Native edited').last);
    await tap(find.text('Сохранить'));
    domains = await read('/domains') as List;
    expect(domains.single['server_id'], serverId);

    await navigate('Серверы');
    await tap(find.text('Удалить из списка'));
    expect(find.text('Удалить сервер из списка?'), findsOneWidget);
    await tap(find.text('Отмена'));
    expect(await read('/servers'), hasLength(1));
    await tap(find.text('Удалить из списка'));
    await tap(find.text('Подтвердить'));
    expect(
      find.textContaining('ПО на сервере не удалялось. Отвязано доменов: 1.'),
      findsOneWidget,
    );
    expect(await read('/servers'), isEmpty);
    domains = await read('/domains') as List;
    expect(domains.single['name'], 'native-edited.example');
    expect(domains.single['server_id'], isNull);
    expect(domains.single['verification_status'], 'unverified');

    // Verify the actual deletion response contract independently of UI copy.
    await tester.runAsync(() async {
      final probe = await check.request('POST', '/servers', {
        'name': 'Deletion response probe',
        'host': '198.51.100.10',
        'ssh_port': 22,
        'username': 'root',
      });
      final deleted = await check.request('DELETE', '/servers/${probe['id']}');
      expect(deleted['deleted'], true);
      expect(deleted['remote_uninstall_performed'], false);
      expect(deleted['unassigned_domain_ids'], isEmpty);
    });
    await navigate('Домены');
    expect(find.text('Сервер: Не назначен'), findsOneWidget);
    await tap(find.text('Удалить домен'));
    await tap(find.text('Отмена'));
    expect(find.text('native-edited.example'), findsOneWidget);
    expect(await read('/domains'), hasLength(1));
    await tap(find.text('Удалить домен'));
    await tap(find.text('Подтвердить'));
    expect(find.text('native-edited.example'), findsNothing);
    expect(await read('/domains'), isEmpty);
    expect(tester.takeException(), isNull);
    await tap(find.text('Клиент'));
    await tap(find.text('Маршрутизация и защита'));
    await tap(find.byTooltip('Добавить: IP и подсети'));
    await tester.enterText(field('IPv4, IPv6 или CIDR'), '10.0.0.0/99');
    await tap(find.text('Добавить'));
    expect(find.text('Некорректный IP или CIDR префикс'), findsOneWidget);
    await tap(find.text('Отмена'));
    expect(tester.takeException(), isNull);
  });
}
