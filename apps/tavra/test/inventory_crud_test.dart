import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:tavra/api.dart';
import 'package:tavra/main.dart';
import 'package:tavra/domains.dart';

void main() {
  testWidgets(
    'domain invalid input prevents request and duplicate response stays actionable',
    (tester) async {
      var creates = 0;
      final api = TavraApi(
        'http://localhost:8787',
        client: MockClient((r) async {
          if (r.method == 'POST') {
            creates++;
            return http.Response('{"error":{"code":"conflict"}}', 409);
          }
          return http.Response('[]', 200);
        }),
      )..token = 'valid';
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SingleChildScrollView(child: DomainsView(api: api)),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('Добавить домен'));
      await tester.pumpAndSettle();
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Домен'),
        'https://*.example.com',
      );
      await tester.tap(find.text('Сохранить'));
      await tester.pumpAndSettle();
      expect(creates, 0);
      expect(find.textContaining('Укажите домен'), findsOneWidget);
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Домен'),
        'duplicate.example.com',
      );
      await tester.tap(find.text('Сохранить'));
      await tester.pumpAndSettle();
      expect(creates, 1);
      expect(find.textContaining('конфликтует'), findsOneWidget);
      expect(find.text('Добавить домен'), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'server edit and delete use inventory API and confirm no uninstall',
    (tester) async {
      Map<String, dynamic>? server = {
        'id': 's1',
        'name': 'Primary',
        'host': 'server.test',
        'username': 'root',
        'ssh_port': 22,
        'status': 'not_provisioned',
      };
      final actions = <http.Request>[];
      final api = TavraApi(
        'http://localhost:8787',
        client: MockClient((r) async {
          if (r.url.path == '/v1/overview')
            return http.Response(
              '{"users":0,"subscriptions":0,"devices":0,"servers":1}',
              200,
            );
          if (r.method == 'PATCH') {
            actions.add(r);
            server = {...server!, ...jsonDecode(r.body)};
            return http.Response(jsonEncode(server), 200);
          }
          if (r.method == 'DELETE') {
            actions.add(r);
            server = null;
            return http.Response(
              '{"deleted":true,"remote_uninstall_performed":false,"unassigned_domain_ids":[]}',
              200,
            );
          }
          return http.Response(jsonEncode(server == null ? [] : [server]), 200);
        }),
      )..token = 'valid';
      await tester.pumpWidget(TavraApp(initialApi: api));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Серверы').last);
      await tester.pumpAndSettle();
      await tester.tap(find.text('Изменить'));
      await tester.pumpAndSettle();
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Название'),
        'Renamed',
      );
      await tester.tap(find.text('Сохранить'));
      await tester.pumpAndSettle();
      expect(actions.single.method, 'PATCH');
      expect(actions.single.url.path, '/v1/servers/s1');
      expect(find.text('Renamed'), findsOneWidget);
      await tester.tap(find.text('Удалить из списка'));
      await tester.pumpAndSettle();
      expect(find.textContaining('ПО на сервере'), findsWidgets);
      expect(actions.length, 1);
      await tester.tap(find.text('Подтвердить'));
      await tester.pumpAndSettle();
      expect(actions.last.method, 'DELETE');
      expect(find.text('Renamed'), findsNothing);
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets('domains create assign edit unassign and delete through API', (
    tester,
  ) async {
    final domains = <Map<String, dynamic>>[];
    final actions = <http.Request>[];
    final api = TavraApi(
      'http://localhost:8787',
      client: MockClient((r) async {
        if (r.url.path == '/v1/overview')
          return http.Response(
            '{"users":0,"subscriptions":0,"devices":0,"servers":1}',
            200,
          );
        if (r.url.path == '/v1/servers')
          return http.Response(
            '[{"id":"s1","name":"Primary","host":"server.test"}]',
            200,
          );
        if (r.method == 'POST') {
          actions.add(r);
          domains.add({
            'id': 'd1',
            ...jsonDecode(r.body),
            'verification_status': 'unverified',
          });
          return http.Response(jsonEncode(domains.last), 200);
        }
        if (r.method == 'PATCH') {
          actions.add(r);
          domains[0] = {...domains[0], ...jsonDecode(r.body)};
          return http.Response(jsonEncode(domains[0]), 200);
        }
        if (r.method == 'DELETE') {
          actions.add(r);
          domains.clear();
          return http.Response('{"deleted":true}', 200);
        }
        return http.Response(jsonEncode(domains), 200);
      }),
    )..token = 'valid';
    await tester.pumpWidget(TavraApp(initialApi: api));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Домены'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Добавить домен'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.widgetWithText(TextFormField, 'Домен'),
      'vpn.example.com',
    );
    await tester.tap(find.byType(DropdownButtonFormField<String>));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Primary').last);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Сохранить'));
    await tester.pumpAndSettle();
    expect(jsonDecode(actions.single.body), {
      'name': 'vpn.example.com',
      'server_id': 's1',
    });
    expect(find.textContaining('Не проверен'), findsWidgets);
    await tester.tap(find.text('Изменить'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.widgetWithText(TextFormField, 'Домен'),
      'new.example.com',
    );
    await tester.tap(find.byType(DropdownButtonFormField<String>));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Не назначен').last);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Сохранить'));
    await tester.pumpAndSettle();
    expect(actions.last.method, 'PATCH');
    expect(jsonDecode(actions.last.body), {
      'name': 'new.example.com',
      'server_id': null,
    });
    await tester.tap(find.text('Удалить домен'));
    await tester.pumpAndSettle();
    expect(actions.length, 2);
    await tester.tap(find.text('Подтвердить'));
    await tester.pumpAndSettle();
    expect(actions.last.method, 'DELETE');
    expect(find.text('new.example.com'), findsNothing);
    expect(tester.takeException(), isNull);
  });
}
