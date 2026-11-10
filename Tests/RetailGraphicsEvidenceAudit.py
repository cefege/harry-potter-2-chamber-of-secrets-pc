#!/usr/bin/env python3
"""Audit a completed advanced graphics run without launching the application."""
from __future__ import annotations
import argparse
import json
import re
from pathlib import Path

COUNTERS=re.compile(r'frames presented=(\d+) occluded=(\d+) outdated=(\d+) other_failure=(\d+)')

def audit(report, logs):
    findings=[]; phases={}
    if report.get('status')=='fail' or report.get('failures'): findings.append('source report has failures; not waived')
    if not report.get('inventory_unchanged'): findings.append('inventory unchanged was not established')
    for phase in ('set','restart'):
        log=logs.get(phase)
        if log is None:
            findings.append(f'{phase}: log missing');phases[phase]={'log_missing':True};continue
        cuts=[line for line in log.splitlines() if 'CutError:' in line and not line.startswith('Destroying ')]
        counters=[tuple(map(int,m)) for m in COUNTERS.findall(log)]
        if cuts: findings.append(f'{phase}: unwaived stock CutError')
        if not counters: findings.append(f'{phase}: presentation counters missing')
        elif not any(row[0]>0 for row in counters): findings.append(f'{phase}: no frames presented; desktop presentation not verified')
        phases[phase]={'cut_errors':cuts,'presentation_counters':[dict(zip(('presented','occluded','outdated','other_failure'),row)) for row in counters]}
    return {'status':'fail' if findings else 'visual-and-live-review-required','findings':findings,'phases':phases,
            'limitations':['settings readback does not prove visible effects','screenshots require visual review',
                           'replayed input does not prove physical mouse alignment','audibility not tested']}

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('run_dir',type=Path);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
    report=json.loads((a.run_dir/'report.json').read_text())
    logs={name:(a.run_dir/name/'run.log').read_text(encoding='utf-8',errors='replace') if (a.run_dir/name/'run.log').is_file() else None for name in ('set','restart')}
    result=audit(report,logs)
    with a.output.open('x',encoding='utf-8') as out:json.dump(result,out,indent=2);out.write('\n')
    print(f"{result['status']}: {a.output}")
    return 1 if result['findings'] else 0
if __name__=='__main__': raise SystemExit(main())
