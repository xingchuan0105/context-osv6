#!/usr/bin/env bash
# Regenerate desktop app icons + NSIS chrome from icons/icon.svg (Full ContextOsMark).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ICONS="$ROOT/desktop/src-tauri/icons"
NSIS="$ICONS/nsis"
command -v rsvg-convert >/dev/null || { echo "need rsvg-convert (librsvg2-bin)"; exit 1; }
command -v python3 >/dev/null || exit 1
mkdir -p "$NSIS" /tmp/cos-icons
for s in 16 32 48 64 128 256 512; do
  rsvg-convert -w "$s" -h "$s" "$ICONS/icon.svg" -o "/tmp/cos-icons/$s.png"
done
cp /tmp/cos-icons/32.png "$ICONS/32x32.png"
cp /tmp/cos-icons/128.png "$ICONS/128x128.png"
cp /tmp/cos-icons/256.png "$ICONS/128x128@2x.png"
python3 - "$ICONS" <<'PY'
import struct, sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
ROOT = Path(sys.argv[1]); NSIS = ROOT / "nsis"; NSIS.mkdir(exist_ok=True)
def write_ico(path, sizes):
    images=[]
    for s in sizes:
        data=Path(f"/tmp/cos-icons/{s}.png").read_bytes(); images.append((s,data))
    buf=bytearray(); buf+=struct.pack("<HHH",0,1,len(images)); off=6+16*len(images)
    for s,data in images:
        w=0 if s>=256 else s; h=w
        buf+=struct.pack("<BBBBHHII", w,h,0,0,1,32,len(data),off); off+=len(data)
    for _,data in images: buf+=data
    Path(path).write_bytes(buf); print("wrote", path)
write_ico(ROOT/"icon.ico",[16,32,48,64,128,256])
write_ico(NSIS/"installer.ico",[16,32,48,64,128,256])
SLATE,INDIGO,WHITE,SOFT,MUTED=(27,31,45),(79,124,243),(255,255,255),(248,250,252),(100,116,139)
mark64=Image.open("/tmp/cos-icons/64.png").convert("RGBA")
mark128=Image.open("/tmp/cos-icons/128.png").convert("RGBA")
header=Image.new("RGB",(150,57),SOFT); d=ImageDraw.Draw(header)
d.rectangle([0,0,3,57], fill=INDIGO)
m=mark64.resize((34,34), Image.Resampling.LANCZOS); header.paste(m,(12,11),m)
try:
    font=ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",13)
    font_sm=ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",10)
except Exception:
    font=font_sm=ImageFont.load_default()
d.text((54,13),"Context-OS",fill=SLATE,font=font); d.text((54,31),"Client",fill=MUTED,font=font_sm)
header.save(NSIS/"header.bmp","BMP")
side=Image.new("RGB",(164,314),SLATE); d=ImageDraw.Draw(side)
for i,c in enumerate([(35,42,72),(45,55,120),(55,70,170),(70,100,220),(79,124,243)]):
    y=200+i*24; d.rectangle([0,y,164,min(314,y+24)], fill=c)
m=mark128.resize((88,88), Image.Resampling.LANCZOS); side.paste(m,((164-88)//2,40),m)
try:
    font=ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",15)
    font_sm=ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",11)
except Exception:
    font=font_sm=ImageFont.load_default()
for text,y,col,f in [("Context-OS",145,WHITE,font),("Client",168,(180,190,210),font_sm)]:
    bb=d.textbbox((0,0),text,font=f); d.text(((164-(bb[2]-bb[0]))//2,y),text,fill=col,font=f)
d.rectangle([66,196,98,199], fill=INDIGO)
tag="Private knowledge"; bb=d.textbbox((0,0),tag,font=font_sm)
d.text(((164-(bb[2]-bb[0]))//2,250),tag,fill=(148,163,184),font=font_sm)
side.save(NSIS/"sidebar.bmp","BMP")
print("nsis chrome ok")
PY
echo "regen-desktop-icons: done"
