#!/usr/bin/env python3
"""Classify only submission IDs explicitly listed in local preparation reports."""
import argparse
import json
import re
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--storage-root', type=Path, required=True)
parser.add_argument('--reports', type=Path, required=True)
parser.add_argument('--apply', action='store_true')
args = parser.parse_args()
count = 0
for report in sorted(args.reports.glob('*/verification.json')):
    data = json.loads(report.read_text())
    problem_id = data['problem_id']
    if not re.fullmatch(r'[A-Za-z0-9_-]{1,100}', problem_id):
        raise ValueError('Invalid problem ID')
    for item in data.get('checks', []) + data.get('submissions', []):
        submission_id = item['submission_id']
        if not re.fullmatch(r'[A-Za-z0-9_-]{1,100}', submission_id):
            raise ValueError('Invalid submission ID')
        root = args.storage_root / 'problems' / problem_id
        path = root / 'submissions' / f'{submission_id}.json'
        original = path.read_bytes()
        record = json.loads(original)
        if record['id'] != submission_id or record['problem_id'] != problem_id:
            raise ValueError('Submission identity mismatch')
        if record['verdict'] == 'JUDGING':
            raise ValueError('Cannot classify a pending run')
        if record.get('purpose') == 'preparation':
            continue
        record['purpose'] = 'preparation'
        # Reports identify preparation runs, but do not establish original expectations.
        record['preparation'] = {'name': item['name'], 'expected': None, 'answer_id': None, 'revision': None}
        if args.apply:
            backup = report.parent / 'classification-backup' / path.name
            backup.parent.mkdir(parents=True, exist_ok=True)
            if not backup.exists():
                backup.write_bytes(original)
            tmp = path.with_suffix('.classification.tmp')
            tmp.write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n')
            tmp.replace(path)
            cache = root / 'submission-summaries' / path.name
            if cache.exists():
                cache.unlink()
        print(f'{problem_id} / {item["name"]} / {submission_id}')
        count += 1
print(f'{count} records {"classified" if args.apply else "identified (dry run)"}')
