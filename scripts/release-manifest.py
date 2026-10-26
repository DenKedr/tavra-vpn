#!/usr/bin/env python3
"""Generate an unsigned dependency inventory and exact artifact hashes (no credentials)."""
import argparse
import hashlib
import json
import pathlib
import re
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--directory', default='dist')
args = parser.parse_args()
out = pathlib.Path(args.directory)
out.mkdir(parents=True, exist_ok=True)
metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--format-version', '1']))
components = []
for package in metadata['packages']:
    components.append({
        'type': 'library', 'name': package['name'], 'version': package['version'],
        'purl': f"pkg:cargo/{package['name']}@{package['version']}",
        **({'licenses': [{'expression': package['license']}]} if package.get('license') else {}),
    })
lock = pathlib.Path('apps/tavra/pubspec.lock').read_text()
for name, block in re.findall(r'^  ([a-zA-Z0-9_]+):\n(.*?)(?=^  [a-zA-Z0-9_]+:|^sdks:|\Z)', lock, re.M | re.S):
    version = re.search(r'^    version: "([^"]+)"', block, re.M)
    if version:
        components.append({'type': 'library', 'name': name, 'version': version[1],
                           'purl': f'pkg:pub/{name}@{version[1]}'})
sbom = {'bomFormat': 'CycloneDX', 'specVersion': '1.5', 'version': 1,
        'metadata': {'component': {'type': 'application', 'name': 'TAVRA Preview', 'version': '0.1.0'},
                     'properties': [{'name': 'tavra:scope', 'value': 'Cargo metadata and Dart lock inventory; excludes OS, Flutter engine and installer toolchain dependencies; not a complete runtime SBOM'}]},
        'components': components}
(out / 'dependency-inventory.cdx.json').write_text(json.dumps(sbom, indent=2) + '\n')
entries = []
for file in sorted(out.rglob('*')):
    if file.is_file() and file.name != 'SHA256SUMS':
        entries.append(f'{hashlib.sha256(file.read_bytes()).hexdigest()}  {file.relative_to(out).as_posix()}')
(out / 'SHA256SUMS').write_text('\n'.join(entries) + '\n')
