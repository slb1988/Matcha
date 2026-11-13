#!/usr/bin/env python3
"""run_corpus.py - 表格报告真实数据回归语料对拍

用法（在本 corpus 目录下）：
  python run_corpus.py                全部用例与各自 baseline.txt 对拍
  python run_corpus.py --update       用当前构建重生成全部 baseline（先人工确认新行为正确再用）
  python run_corpus.py --case <目录名>  只跑一个用例

matcha 路径：环境变量 MATCHA 优先；否则取仓库 target/debug/matcha.exe，再退 target/release/matcha.exe。
退出码：全部 PASS 为 0，否则为 FAIL 用例数（可进 CI）。FAIL 时保留该用例目录下的 actual.txt 供 diff。
"""
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))


def find_matcha():
    if os.environ.get('MATCHA'):
        return os.environ['MATCHA']
    repo = os.path.abspath(os.path.join(ROOT, '..', '..', '..', '..'))
    for rel in (os.path.join('target', 'debug', 'matcha.exe'),
                os.path.join('target', 'release', 'matcha.exe')):
        p = os.path.join(repo, rel)
        if os.path.exists(p):
            return p
    sys.exit('找不到 matcha.exe：请先 cargo build -p bcompare，或设 MATCHA 环境变量')


def norm(text):
    # 报告头部的 left:/right: 是绝对路径，随机器变化，归一化后再对拍
    out = []
    for ln in text.splitlines():
        if ln.startswith('left:'):
            ln = 'left:  left.xlsx'
        elif ln.startswith('right:'):
            ln = 'right: right.xlsx'
        out.append(ln)
    return '\n'.join(out) + '\n'


def run_case(matcha, casedir, update):
    left = os.path.join(casedir, 'left.xlsx')
    right = os.path.join(casedir, 'right.xlsx')
    baseline = os.path.join(casedir, 'baseline.txt')
    actual_p = os.path.join(casedir, 'actual.txt')
    if not (os.path.exists(left) and os.path.exists(right)):
        return None
    subprocess.run([matcha, '-report=summary', '-reportoptions=title-row:1',
                    '-reportfile=' + actual_p, left, right], check=True)
    with open(actual_p, encoding='utf-8') as f:
        actual = norm(f.read())
    if update or not os.path.exists(baseline):
        with open(baseline, 'w', encoding='utf-8', newline='\n') as f:
            f.write(actual)
        os.remove(actual_p)
        return 'UPDATED'
    with open(baseline, encoding='utf-8') as f:
        expect = f.read()
    if expect == actual:
        os.remove(actual_p)
        return 'PASS'
    return 'FAIL'  # actual.txt 保留供 diff


def main():
    update = '--update' in sys.argv
    only = None
    if '--case' in sys.argv:
        only = sys.argv[sys.argv.index('--case') + 1]
    matcha = find_matcha()
    print('matcha:', matcha)
    fails = 0
    for name in sorted(os.listdir(ROOT)):
        d = os.path.join(ROOT, name)
        if not os.path.isdir(d) or (only and name != only):
            continue
        r = run_case(matcha, d, update)
        if r is None:
            continue
        print(f'{r:8} {name}')
        if r == 'FAIL':
            fails += 1
    print('---')
    print('baselines updated' if update else f'FAIL: {fails}')
    sys.exit(1 if fails else 0)


if __name__ == '__main__':
    main()
