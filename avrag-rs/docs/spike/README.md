# Ingestion spikes

| Script | Purpose |
|--------|---------|
| `scripts/spike/paddle_ocr_spike.py` | Paddle AI Studio OCR job on PDF page slice |
| `scripts/spike/probe_page_stats.py` | R1–R3: per-page `readable_ratio`, `figure_area_ratio`, route prediction |

```bash
cd avrag-rs
set -a && source .env && set +a
python3 -m venv /tmp/paddle-spike-venv && /tmp/paddle-spike-venv/bin/pip install requests
/tmp/paddle-spike-venv/bin/python scripts/spike/paddle_ocr_spike.py
```

Outputs: `docs/spike/paddle-black-swan-p1-20/report.json`

### Probe page stats (threshold calibration)

```bash
pip install pymupdf
python scripts/spike/probe_page_stats.py \
  --pdf "/mnt/e/OneDrive/桌面/知境笔记/Taleb_Antifragile__2012.pdf" \
  --sample 30 --out docs/spike/probe-antifragile-30.json
python scripts/spike/probe_page_stats.py \
  --pdf "/mnt/e/OneDrive/桌面/知境笔记/the-black-swan_-....pdf" \
  --sample 30 --out docs/spike/probe-black-swan-30.json
```

Compare `naive_figure_pages_image_gt_0` vs `smart_figure_pages_route_B` for B-trigger savings.

**WSL**：脚本已禁用 requests 环境代理；若 worker 集成后仍 SSL 失败，在 `.env` 增加  
`NO_PROXY=...,paddleocr.aistudio-app.com,bj.bcebos.com`
