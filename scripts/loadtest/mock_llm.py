#!/usr/bin/env python3
"""Mock OpenAI-compatible LLM stub for load testing the avrag-rs API layer.

Zero deps (stdlib only). Streams canned SSE chunks with configurable per-chunk
delay so SSE connection-holding behavior can be load-tested without burning
real LLM quota or hitting provider rate limits.

Env:
  STUB_PORT           listen port (default 8399)
  STUB_DELAY_MS       per-chunk delay in ms (default 200) — simulates thinking
  STUB_CHUNKS         number of content chunks (default 20)
  STUB_ANSWER         answer text prefix (default canned)

Endpoints:
  POST /v1/chat/completions   stream=true → SSE; else single JSON
  POST /v1/responses          single JSON with web_search_call + message
  POST /v1/embeddings         fixed 1024-dim vectors
  GET  /v1/models             static list
  GET  /health                200
"""
import json
import os
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PORT = int(os.environ.get("STUB_PORT", "8399"))
DELAY_MS = int(os.environ.get("STUB_DELAY_MS", "200"))
CHUNKS = int(os.environ.get("STUB_CHUNKS", "20"))
ANSWER = os.environ.get(
    "STUB_ANSWER",
    "Stub answer: retrieved facts A/B/C point to the same conclusion.",
)

CANNED_SOURCES = [
    {"type": "url", "url": "https://example.com/source-1"},
    {"type": "url", "url": "https://example.com/source-2"},
]


def completion_payload(model: str, stream: bool):
    return {
        "id": "chatcmpl-stub",
        "object": "chat.completion",
        "created": int(time.time()),
        "model": model,
        "choices": [
            {
                "index": 0,
                "message": {"role": "assistant", "content": ANSWER},
                "finish_reason": "stop",
            }
        ],
        "usage": {
            "prompt_tokens": 128,
            "completion_tokens": CHUNKS,
            "total_tokens": 128 + CHUNKS,
        },
    }


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_args):  # quiet
        pass

    def _json(self, code: int, obj):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _read_body(self) -> dict:
        length = int(self.headers.get("Content-Length") or 0)
        if length <= 0:
            return {}
        try:
            return json.loads(self.rfile.read(length) or b"{}")
        except Exception:
            return {}

    def do_GET(self):
        if self.path == "/health":
            return self._json(200, {"ok": True})
        if self.path.endswith("/models"):
            return self._json(
                200,
                {
                    "object": "list",
                    "data": [
                        {"id": "stub-model", "object": "model"},
                        {"id": "deepseek-ai/DeepSeek-V4-Flash", "object": "model"},
                        {"id": "qwen3.7-flash", "object": "model"},
                    ],
                },
            )
        return self._json(404, {"error": "not found"})

    def do_POST(self):
        body = self._read_body()
        if self.path.endswith("/embeddings"):
            n = len(body.get("input") or [""]) if isinstance(body.get("input"), list) else 1
            vec = [0.01] * 1024
            return self._json(
                200,
                {
                    "object": "list",
                    "data": [
                        {"object": "embedding", "index": i, "embedding": vec}
                        for i in range(n)
                    ],
                    "usage": {"prompt_tokens": 8 * n, "total_tokens": 8 * n},
                },
            )
        if self.path.endswith("/responses"):
            return self._json(
                200,
                {
                    "id": "resp-stub",
                    "object": "response",
                    "model": body.get("model", "stub-model"),
                    "output": [
                        {
                            "type": "web_search_call",
                            "action": {
                                "type": "search",
                                "queries": ["stub query"],
                                "sources": CANNED_SOURCES,
                            },
                        },
                        {
                            "type": "message",
                            "content": [{"type": "output_text", "text": ANSWER}],
                        },
                    ],
                    "usage": {"input_tokens": 64, "output_tokens": 32, "total_tokens": 96},
                },
            )
        if self.path.endswith("/chat/completions"):
            model = body.get("model", "stub-model")
            if body.get("stream"):
                return self._stream_chat(model)
            return self._json(200, completion_payload(model, False))
        return self._json(404, {"error": "not found"})

    def _stream_chat(self, model: str):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "keep-alive")
        self.end_headers()
        words = (ANSWER + " ").split(" ")
        per = max(1, CHUNKS)
        try:
            for i in range(per):
                chunk = {
                    "id": "chatcmpl-stub",
                    "object": "chat.completion.chunk",
                    "created": int(time.time()),
                    "model": model,
                    "choices": [
                        {
                            "index": 0,
                            "delta": {"content": words[i % len(words)] + " "},
                            "finish_reason": None,
                        }
                    ],
                }
                self.wfile.write(f"data: {json.dumps(chunk)}\n\n".encode())
                self.wfile.flush()
                time.sleep(DELAY_MS / 1000.0)
            done = {
                "id": "chatcmpl-stub",
                "object": "chat.completion.chunk",
                "created": int(time.time()),
                "model": model,
                "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
            }
            self.wfile.write(f"data: {json.dumps(done)}\n\n".encode())
            self.wfile.write(b"data: [DONE]\n\n")
            self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError):
            pass  # client disconnected mid-stream — expected under load


if __name__ == "__main__":
    server = ThreadingHTTPServer(("0.0.0.0", PORT), Handler)
    print(f"mock-llm stub on :{PORT} delay={DELAY_MS}ms chunks={CHUNKS}")
    server.serve_forever()
