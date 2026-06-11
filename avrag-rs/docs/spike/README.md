# Ingestion spikes

| Script | Purpose |
|--------|---------|
| `scripts/spike/paddle_ocr_spike.py` | Paddle AI Studio OCR job on PDF page slice |

```bash
cd avrag-rs
set -a && source .env && set +a
python3 -m venv /tmp/paddle-spike-venv && /tmp/paddle-spike-venv/bin/pip install requests
/tmp/paddle-spike-venv/bin/python scripts/spike/paddle_ocr_spike.py
```

Outputs: `docs/spike/paddle-black-swan-p1-20/report.json`

**WSL**：脚本已禁用 requests 环境代理；若 worker 集成后仍 SSL 失败，在 `.env` 增加  
`NO_PROXY=...,paddleocr.aistudio-app.com,bj.bcebos.com`
