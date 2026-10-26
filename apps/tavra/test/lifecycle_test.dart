import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:tavra/lifecycle.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  test('desktop preference reads and saves confirmed native result', () async {
    final calls = <MethodCall>[];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(const MethodChannel('dev.tavra/lifecycle'), (
          call,
        ) async {
          calls.add(call);
          return call.method == 'getBackgroundMode' ? true : call.arguments;
        });
    final service = LifecyclePreference();
    expect(await service.read(), true);
    expect(await service.write(false), false);
    expect(calls.last.method, 'setBackgroundMode');
    expect(calls.last.arguments, false);
  });
}
