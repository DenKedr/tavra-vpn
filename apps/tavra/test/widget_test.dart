import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:tavra/main.dart';
import 'package:tavra/api.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

void main() {
  testWidgets('expired dashboard session returns to login', (tester) async {
    final api = TavraApi(
      'http://localhost:8787',
      client: MockClient(
        (r) async => http.Response('{"error":{"code":"unauthorized"}}', 401),
      ),
    )..token = 'expired';
    await tester.pumpWidget(TavraApp(initialApi: api));
    await tester.pumpAndSettle();
    expect(find.text('Войти'), findsOneWidget);
  });
  testWidgets('narrow authenticated screen can log out', (tester) async {
    tester.view.physicalSize = const Size(320, 700);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final api = TavraApi(
      'http://localhost:8787',
      client: MockClient(
        (r) async => http.Response(
          '{"users":0,"subscriptions":0,"devices":0,"servers":0}',
          200,
        ),
      ),
    )..token = 'valid';
    await tester.pumpWidget(TavraApp(initialApi: api));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Выйти'));
    await tester.pumpAndSettle();
    expect(find.text('Войти'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
  testWidgets('admin field accepts server issued base64url key', (
    tester,
  ) async {
    await tester.pumpWidget(const TavraApp());
    final field = tester.widget<TextFormField>(
      find.byType(TextFormField).at(1),
    );
    expect(
      field.validator!('vN0P4J2dYq7Sx9CkL3Hb6ZtR8Wm5AfEeU1I_o-GhBQs'),
      isNull,
    );
    expect(field.validator!('short'), isNotNull);
    expect(field.validator!('x' * 40 + '\n'), isNotNull);
  });
  testWidgets('narrow phone layout has no overflow', (tester) async {
    tester.view.physicalSize = const Size(320, 700);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(const TavraApp());
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
  });
  testWidgets('empty admin login validates without a request', (tester) async {
    await tester.pumpWidget(const TavraApp());
    await tester.tap(find.text('Войти'));
    await tester.pumpAndSettle();
    expect(find.textContaining('32'), findsWidgets);
  });
  testWidgets('client mode exposes import and cannot claim connected', (
    tester,
  ) async {
    await tester.pumpWidget(const TavraApp());
    await tester.tap(find.text('Клиент'));
    await tester.pumpAndSettle();
    expect(find.text('Импорт приглашения'), findsOneWidget);
    expect(
      tester
          .widget<FilledButton>(
            find.widgetWithText(FilledButton, 'Подключить VPN'),
          )
          .onPressed,
      isNull,
    );
  });
}
