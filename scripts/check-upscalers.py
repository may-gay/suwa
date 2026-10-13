#!/usr/bin/env python3
import argparse
import json
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('models', type=Path)
parser.add_argument('input', type=Path)
parser.add_argument('output', type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
args.output.mkdir(parents=True, exist_ok=True)
results = []
for preset in json.loads((root / 'models/catalog.json').read_text()):
    manifest = args.models / (preset['id'] + '.json')
    if not manifest.is_file():
        raise SystemExit(f'Missing {manifest}; install models first')
    model = json.loads(manifest.read_text())
    variants = model.get('variants', []) or [None]
    for variant in variants:
        name = preset['id'] + (f'-{variant["height"]}p' if variant else '')
        output = args.output / (name + '.png')
        temporary = None
        try:
            path = manifest
            if variant:
                chosen = dict(model, file=variant['file'], sha256=variant['sha256'], variants=[])
                temporary = tempfile.NamedTemporaryFile(mode='w', suffix='.json', prefix='suwa-check-', dir=args.models, delete=False)
                json.dump(chosen, temporary)
                temporary.close()
                path = Path(temporary.name)
            result = subprocess.run([str(root / 'scripts/run.sh'), '--upscale-check', str(path.resolve()), str(args.input.resolve()), str(output.resolve())], cwd=root, capture_output=True, text=True)
            results.append({'model': name, 'returncode': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr})
            print(name, 'PASS' if result.returncode == 0 else 'FAIL', flush=True)
            print(result.stdout, end='', flush=True)
            if result.returncode:
                print(result.stderr, flush=True)
        finally:
            if temporary:
                Path(temporary.name).unlink(missing_ok=True)
        (args.output / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
if any(result['returncode'] for result in results):
    raise SystemExit(1)
