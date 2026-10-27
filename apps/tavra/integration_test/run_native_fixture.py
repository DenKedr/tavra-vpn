"""Disposable native acceptance fixture. Never prints runtime credentials."""
import json, os, pathlib, subprocess, time, urllib.parse, urllib.request
ROOT = pathlib.Path(__file__).resolve().parents[3]
RUNTIME = ROOT.parent.parent / 'work/runtime'
KEY = 'TAVRA_NATIVE_TEST_ONLY_ADMIN_KEY_0000000000'
BOOTSTRAP = 'TAVRA_NATIVE_TEST_ONLY_BOOTSTRAP_0000000000'
def main():
    config = json.loads((RUNTIME / 'dev-env.json').read_text())
    database = config['TAVRA_TEST_DATABASE_URL']
    assert urllib.parse.urlparse(database).path == '/tavra_test', 'Refusing non-test database'
    parsed = urllib.parse.urlparse(database)
    env = dict(os.environ, PGDATABASE='tavra_test', PGHOST=parsed.hostname or '127.0.0.1', PGPORT=str(parsed.port or 5432), PGUSER=urllib.parse.unquote(parsed.username or ''), PGPASSWORD=urllib.parse.unquote(parsed.password or ''))
    subprocess.run(['psql', '-X', '-q', '-v', 'ON_ERROR_STOP=1', '-c',
      'TRUNCATE admins,sessions,users,subscriptions,challenges,devices,servers,domains,inventory_audit,app_settings CASCADE; UPDATE security_clock SET high_water=0,generation=generation+1'], env=env, check=True)
    env.update(TAVRA_DATABASE_URL=database, TAVRA_BOOTSTRAP_SECRET=BOOTSTRAP, TAVRA_BIND='127.0.0.1:8790')
    log = open(RUNTIME / 'native-test-control.log', 'w')
    process = subprocess.Popen([str(ROOT / 'target/debug/tavra-control')], env=env, stdout=log, stderr=log)
    try:
        for _ in range(50):
            try:
                request = urllib.request.Request('http://127.0.0.1:8790/v1/admin/bootstrap', data=json.dumps({'bootstrap_secret':BOOTSTRAP,'admin_secret':KEY}).encode(), headers={'Content-Type':'application/json'})
                with urllib.request.urlopen(request) as response:
                    assert response.status == 200
                break
            except ConnectionError:
                time.sleep(.2)
            except urllib.error.URLError as error:
                if not isinstance(error, urllib.error.HTTPError) or error.code == 503: time.sleep(.2)
                else: raise
        else: raise RuntimeError('Test API unavailable')
        result = subprocess.run([str(ROOT.parent.parent / 'work/toolchains/flutter/bin/flutter'), 'test', 'integration_test/native_smoke_test.dart', '-d', 'macos'], cwd=ROOT / 'apps/tavra')
        raise SystemExit(result.returncode)
    finally:
        process.terminate()
        process.wait(timeout=10)
        log.close()
if __name__ == '__main__': main()
