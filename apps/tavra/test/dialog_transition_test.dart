import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:tavra/api.dart';
import 'package:tavra/main.dart';
import 'package:tavra/settings.dart';

void main() {
  testWidgets('user save remains valid during reverse dialog transition', (
    tester,
  ) async {
    final users = <Map<String, dynamic>>[];
    final api = TavraApi(
      'http://localhost:8787',
      client: MockClient((r) async {
        if (r.url.path == '/v1/overview')
          return http.Response(
            '{"users":0,"subscriptions":0,"devices":0,"servers":0}',
            200,
          );
        if (r.method == 'POST') {
          users.add({...jsonDecode(r.body), 'id': 'u1', 'status': 'active'});
          return http.Response(jsonEncode(users.last), 200);
        }
        return http.Response(jsonEncode(users), 200);
      }),
    )..token = 'valid';
    await tester.pumpWidget(TavraApp(initialApi: api));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Пользователи').last);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Добавить'));
    await tester.pumpAndSettle();
    await tester.enterText(find.widgetWithText(TextFormField, 'Имя'), 'Test');
    await tester.tap(find.text('Сохранить'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    expect(find.text('Test '), findsOneWidget);
  });
  testWidgets('site rule survives reverse dialog transition', (tester) async {
    SharedPreferences.setMockInitialValues({});
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: SingleChildScrollView(child: SettingsView(local: true)),
        ),
      ),
    );
    await tester.pumpAndSettle();
    final add = find.byTooltip('Добавить: Исключённые сайты');
    await tester.ensureVisible(add);
    await tester.tap(add);
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextFormField).last, 'example.com');
    await tester.tap(find.widgetWithText(FilledButton, 'Добавить'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    expect(find.text('example.com'), findsOneWidget);
  });
}
