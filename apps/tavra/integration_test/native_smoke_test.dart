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
    final check = TavraApi('http://127.0.0.1:8790');
    await tester.runAsync(() async {
      await check.login(testKey);
      final settings = await check.request('GET', '/settings');
      expect(settings['bypass_lan'], false);
      expect(settings['excluded_sites'], contains('example.org'));
      expect(settings['excluded_networks'], contains('10.23.0.0/16'));
      expect(await check.request('GET', '/users'), isEmpty);
      check.close();
    });
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
