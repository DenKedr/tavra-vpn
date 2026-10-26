#!/usr/bin/env python3
"""Record public, exact input file hashes; never serialize environment/config values."""
import argparse
import hashlib
import json
import pathlib
import platform
import subprocess

paths = ['Cargo.lock', 'Cargo.toml', 'apps/tavra/pubspec.lock', 'apps/tavra/pubspec.yaml']
paths += [str(p) for parent in ['.github/workflows', 'packaging'] for p in pathlib.Path(parent).rglob('*') if p.is_file()]
paths += [str(p) for p in pathlib.Path('scripts').glob('release-*') if p.is_file()]
data = {'revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
        'system': platform.system(), 'architecture': platform.machine(),
        'rust_version': '1.99.1', 'flutter_version': '3.47.8', 'dart_version': '3.13.7',
        'distribution': 'development preview; no vendor signing or notarization',
        'inputs_sha256': {p: hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest() for p in sorted(paths)}}
parser = argparse.ArgumentParser()
parser.add_argument('--directory', default='dist')
args = parser.parse_args()
out = pathlib.Path(args.directory); out.mkdir(parents=True, exist_ok=True)
(out / 'BUILD-INPUTS.json').write_text(json.dumps(data, indent=2) + '\n')
