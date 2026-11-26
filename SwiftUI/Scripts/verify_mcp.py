#!/usr/bin/env python3
"""Bounded, dependency-free stdio MCP smoke test against the actual executable."""
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import tempfile

binary = Path(sys.argv[1] if len(sys.argv) > 1 else Path(__file__).resolve().parents[1] / '.build/debug/diffloupe-mcp').resolve()
with tempfile.TemporaryDirectory(prefix='diffloupe-mcp-') as temporary:
    root = Path(temporary)
    left, right = root / 'left', root / 'right'
    left.mkdir()
    right.mkdir()
    original = ''.join(f'unchanged line {i:04d}\n' for i in range(1000))
    changed = original.replace('unchanged line 0500', 'changed line 0500')
    (left / 'sample.txt').write_text(original)
    (right / 'sample.txt').write_text(changed)
    (left / 'same.txt').write_text('same')
    (right / 'same.txt').write_text('same')
    os.mkfifo(left / 'pipe')
    (left / 'external-link').symlink_to('/etc/hosts')
    with tempfile.TemporaryFile() as errors:
        process = subprocess.Popen([str(binary), '--root', str(root)], stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=errors, bufsize=0)
        pending = bytearray()
        def send(message):
            process.stdin.write(json.dumps(message).encode() + b'\n')
        def rpc(identifier, method, params=None):
            send(dict(jsonrpc='2.0', id=identifier, method=method, params=params or {}))
            while b'\n' not in pending:
                if not select.select([process.stdout], [], [], 10)[0]:
                    raise TimeoutError(f'{method} timed out')
                chunk = os.read(process.stdout.fileno(), 4096)
                if not chunk:
                    raise RuntimeError(f'{method}: server exited')
                pending.extend(chunk)
            line, _, rest = pending.partition(b'\n')
            pending[:] = rest
            reply = json.loads(line)
            assert reply['id'] == identifier, reply
            assert 'error' not in reply, reply
            return reply['result']
        def call(identifier, name, arguments):
            result = rpc(identifier, 'tools/call', dict(name=name, arguments=arguments))
            assert not result['isError'], result
            return json.loads(result['content'][0]['text']), result
        try:
            result = rpc(1, 'initialize', dict(protocolVersion='2025-11-25', capabilities={},
                                              clientInfo=dict(name='smoke', version='1')))
            assert result['protocolVersion'] == '2025-11-25'
            send(dict(jsonrpc='2.0', method='notifications/initialized'))
            tools = rpc(2, 'tools/list')['tools']
            assert {t['name'] for t in tools} == {'compare_folders', 'diff_files'}
            print('PASS initialize / tools/list (2 tools)')
            result, _ = call(3, 'compare_folders', dict(left=str(left), right=str(right)))
            assert result['summary']['identical'] == 1 and result['summary']['different'] == 1, result
            assert [e['path'] for e in result['entries']] == ['sample.txt'], result
            print('PASS compare_folders (unchanged contents omitted; symlink/FIFO skipped)')
            result, wire = call(4, 'diff_files', dict(left=str(left / 'sample.txt'), right=str(right / 'sample.txt')))
            assert result['changedRows'] == 1 and len(result['rows']) == 5, result
            original_bytes = len(original.encode()) + len(changed.encode())
            response_bytes = len(json.dumps(wire, separators=(',', ':')).encode())
            print(f'PASS diff_files: full inputs {original_bytes} bytes -> tool result {response_bytes} bytes '
                  f'({100 * (1 - response_bytes / original_bytes):.1f}% fewer bytes; not a token measurement)')
            rejected = rpc(5, 'tools/call', dict(name='diff_files', arguments=dict(left='/etc/hosts', right='/etc/hosts')))
            assert rejected['isError']
            print('PASS outside-root path rejected')
            process.stdin.close()
            assert process.wait(timeout=10) == 0
            errors.seek(0)
            assert errors.read() == b''
            print('ALL PASSED (stdout JSON-RPC only, stderr empty, clean EOF)')
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
