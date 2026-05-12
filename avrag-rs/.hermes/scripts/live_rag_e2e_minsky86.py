#!/usr/bin/env python3
import json
import os
import sys
import time
import urllib.error
import urllib.request
from collections import Counter
from pathlib import Path

ROOT = Path('/home/chuan/context-osv6/avrag-rs')
PDF = Path('/mnt/e/Download/minsky86.pdf')
BASE = os.environ.get('AVRAG_BASE_URL', 'http://127.0.0.1:8080').rstrip('/')
RUN_TS = int(time.time())
RUN_NAME = f'e2e-minsky86-{RUN_TS}'
OUT_DIR = ROOT / '.hermes' / 'runs'
OUT_DIR.mkdir(parents=True, exist_ok=True)
OUT_JSON = OUT_DIR / f'{RUN_NAME}.json'

result = {
    'run_name': RUN_NAME,
    'base_url': BASE,
    'pdf': str(PDF),
    'pdf_size': None,
    'timeline': [],
    'health': None,
    'ready': None,
    'milvus_collections': None,
    'email': None,
    'notebook_id': None,
    'document_id': None,
    'final_status': None,
    'status_history': [],
    'chat': None,
    'error': None,
}

def log(msg):
    stamp = time.strftime('%H:%M:%S')
    line = f'[{stamp}] {msg}'
    result['timeline'].append(line)
    print(line, flush=True)

def req(method, url, data=None, headers=None, timeout=30):
    headers = dict(headers or {})
    body = None
    if data is not None:
        if isinstance(data, (bytes, bytearray)):
            body = bytes(data)
        else:
            body = json.dumps(data).encode('utf-8')
            headers.setdefault('Content-Type', 'application/json')
    request = urllib.request.Request(url, data=body, headers=headers, method=method)
    try:
        with urllib.request.urlopen(request, timeout=timeout) as resp:
            payload = resp.read()
            text = payload.decode('utf-8', errors='replace')
            return resp.status, text, dict(resp.headers)
    except urllib.error.HTTPError as e:
        text = e.read().decode('utf-8', errors='replace')
        return e.code, text, dict(e.headers)

def json_req(method, path, data=None, token=None, timeout=30):
    headers = {}
    if token:
        headers['Authorization'] = f'Bearer {token}'
    status, text, hdrs = req(method, BASE + path, data=data, headers=headers, timeout=timeout)
    parsed = None
    try:
        parsed = json.loads(text) if text else None
    except Exception:
        parsed = {'_raw': text[:1000]}
    return status, parsed, text, hdrs

def auth_headers(token):
    return {'Authorization': f'Bearer {token}'}

def parse_sse(text):
    events = []
    event = 'message'
    data_parts = []
    for raw in text.splitlines():
        if raw.startswith('event:'):
            event = raw.split(':', 1)[1].strip()
        elif raw.startswith('data:'):
            data_parts.append(raw.split(':', 1)[1].lstrip())
        elif raw == '':
            if data_parts:
                events.append({'event': event, 'data': '\n'.join(data_parts)})
            event = 'message'
            data_parts = []
    if data_parts:
        events.append({'event': event, 'data': '\n'.join(data_parts)})
    return events

def abbreviate(obj, n=500):
    text = obj if isinstance(obj, str) else json.dumps(obj, ensure_ascii=False)
    return text if len(text) <= n else text[:n] + '...'

def main():
    if not PDF.exists():
        raise RuntimeError(f'PDF missing: {PDF}')
    result['pdf_size'] = PDF.stat().st_size
    log(f'Starting live RAG E2E: {PDF} ({result["pdf_size"]} bytes)')

    status, body, text, _ = json_req('GET', '/health')
    result['health'] = {'status': status, 'body': body}
    log(f'GET /health -> {status} {abbreviate(body)}')
    if status != 200:
        raise RuntimeError('/health failed')

    status, body, text, _ = json_req('GET', '/ready')
    result['ready'] = {'status': status, 'body': body}
    log(f'GET /ready -> {status} {abbreviate(body)}')
    if status != 200:
        raise RuntimeError('/ready failed')

    milvus_url = None
    env_path = ROOT / '.env'
    if env_path.exists():
        for line in env_path.read_text(errors='ignore').splitlines():
            if line.startswith('MILVUS_URL='):
                milvus_url = line.split('=', 1)[1].strip().strip('"').strip("'")
                break
    if milvus_url:
        try:
            m_status, m_text, _ = req('POST', milvus_url.rstrip('/') + '/v2/vectordb/collections/list', data={}, headers={'Content-Type': 'application/json'}, timeout=10)
            try:
                m_body = json.loads(m_text)
            except Exception:
                m_body = {'_raw': m_text[:1000]}
            result['milvus_collections'] = {'status': m_status, 'body': m_body}
            log(f'Milvus collections/list -> {m_status} {abbreviate(m_body)}')
        except Exception as exc:
            result['milvus_collections'] = {'error': str(exc)}
            log(f'Milvus collections/list failed: {exc}')

    email = f'{RUN_NAME}@e2e.test'
    password = os.environ.get('E2E_PASSWORD') or f'e2e-{RUN_TS}-password'
    status, body, _, _ = json_req('POST', '/api/auth/register', {
        'email': email,
        'password': password,
        'full_name': 'E2E Minsky Test User',
    })
    result['email'] = email
    log(f'POST /api/auth/register -> {status}')
    if status not in (200, 201):
        raise RuntimeError(f'register failed: {abbreviate(body)}')
    token = (((body or {}).get('data') or {}).get('token') or '').strip()
    if not token:
        raise RuntimeError('register returned no token')

    status, body, _, _ = json_req('GET', '/api/auth/me', token=token)
    log(f'GET /api/auth/me -> {status}')
    if status != 200:
        raise RuntimeError(f'/me failed: {abbreviate(body)}')

    status, body, _, _ = json_req('POST', '/api/v1/notebooks', {
        'name': RUN_NAME,
        'title': RUN_NAME,
        'description': 'Live E2E minsky86.pdf after SUMMARY_LLM restore',
    }, token=token)
    log(f'POST /api/v1/notebooks -> {status} {abbreviate(body)}')
    if status not in (200, 201):
        raise RuntimeError(f'create notebook failed: {abbreviate(body)}')
    notebook_id = ((body or {}).get('notebook') or {}).get('id')
    result['notebook_id'] = notebook_id
    if not notebook_id:
        raise RuntimeError('create notebook returned no id')

    status, body, _, _ = json_req('POST', f'/api/v1/notebooks/{notebook_id}/documents', {
        'filename': PDF.name,
        'file_size': result['pdf_size'],
        'mime_type': 'application/pdf',
    }, token=token)
    log(f'POST /api/v1/notebooks/{notebook_id}/documents -> {status} {abbreviate(body)}')
    if status not in (200, 201):
        raise RuntimeError(f'create document failed: {abbreviate(body)}')
    document_id = (body or {}).get('document_id')
    result['document_id'] = document_id
    if not document_id:
        raise RuntimeError('create document returned no document_id')

    pdf_bytes = PDF.read_bytes()
    status, text, _ = req('PUT', f'{BASE}/dev-upload/{document_id}', data=pdf_bytes, headers={
        'Authorization': f'Bearer {token}',
        'Content-Type': 'application/octet-stream',
    }, timeout=120)
    log(f'PUT /dev-upload/{document_id} -> {status} {abbreviate(text)}')
    if status not in (200, 201, 202):
        raise RuntimeError(f'upload failed: {text[:1000]}')

    final_status = 'unknown'
    start = time.monotonic()
    last_logged_status = None
    # 18 minutes: previous successful run was ~5.5 minutes, this leaves room for external APIs.
    for i in range(0, 216):
        status, body, _, _ = json_req('GET', f'/api/v1/documents/{document_id}/status', token=token, timeout=20)
        doc_status = (body or {}).get('status') or 'unknown'
        elapsed = int(time.monotonic() - start)
        entry = {'elapsed_s': elapsed, 'http_status': status, 'status': doc_status, 'body': body}
        result['status_history'].append(entry)
        should_log = doc_status != last_logged_status or i % 12 == 0 or doc_status in ('completed', 'failed')
        if should_log:
            log(f'status poll {i:03d} elapsed={elapsed}s -> HTTP {status}, status={doc_status}, body={abbreviate(body)}')
            last_logged_status = doc_status
        if doc_status in ('completed', 'failed'):
            final_status = doc_status
            break
        time.sleep(5)
    result['final_status'] = final_status
    log(f'Final document status: {final_status}')

    if final_status != 'completed':
        log('Skipping RAG chat because document did not complete ingestion.')
        return 2

    chat_body = {
        'query': '请基于上传的 Minsky 论文，用中文概括它的核心观点，并指出至少一个引用来源。',
        'notebook_id': notebook_id,
        'agent_type': 'rag',
        'doc_scope': [document_id],
        'stream': True,
    }
    headers = {
        'Authorization': f'Bearer {token}',
        'Content-Type': 'application/json',
        'Accept': 'text/event-stream',
        'x-request-id': RUN_NAME,
    }
    status, sse_text, _ = req('POST', f'{BASE}/api/v1/chat', data=chat_body, headers=headers, timeout=300)
    log(f'POST /api/v1/chat SSE -> {status}, bytes={len(sse_text)}')
    if status != 200:
        result['chat'] = {'status': status, 'raw': sse_text[:2000]}
        raise RuntimeError(f'chat failed: {status} {sse_text[:1000]}')
    events = parse_sse(sse_text)
    counts = Counter(e['event'] for e in events)
    answer = ''
    citations_payload = None
    activity_samples = []
    for e in events:
        if e['event'] == 'token':
            try:
                answer += json.loads(e['data']).get('content') or ''
            except Exception:
                pass
        elif e['event'] == 'citations':
            try:
                citations_payload = json.loads(e['data'])
            except Exception:
                citations_payload = {'_raw': e['data'][:1000]}
        elif e['event'] in ('activity', 'trace') and len(activity_samples) < 8:
            activity_samples.append(e)
    citation_count = 0
    if isinstance(citations_payload, dict) and isinstance(citations_payload.get('citations'), list):
        citation_count = len(citations_payload['citations'])
    result['chat'] = {
        'status': status,
        'event_counts': dict(counts),
        'answer_length': len(answer),
        'answer_preview': answer[:1200],
        'citation_count': citation_count,
        'citations_payload_preview': citations_payload,
        'activity_samples': activity_samples,
    }
    log(f'SSE event counts: {dict(counts)}')
    log(f'Answer length: {len(answer)}; citations: {citation_count}')
    if answer:
        log(f'Answer preview: {answer[:300].replace(chr(10), " ")}')
    if citation_count == 0:
        log('WARNING: chat completed but returned zero citations')
    return 0

try:
    code = main()
except Exception as exc:
    result['error'] = repr(exc)
    log(f'ERROR: {exc!r}')
    code = 1
finally:
    OUT_JSON.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
    log(f'Wrote JSON report: {OUT_JSON}')

sys.exit(code)
