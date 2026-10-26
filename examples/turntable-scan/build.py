#!/usr/bin/env python3
"""The turntable scanner, closed loop through a real video file: the
frames rendered by `cargo test -p ok-photo --test scan` (a lobed gourd
of known shape on the 2x2 field, going round under a camera and a line
laser) are encoded with ffmpeg, the video is scanned through the MCP
server, and the mesh is imported and pictured. Needs
`cargo build --release -p ok-mcp` and ffmpeg."""
import glob
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
OUT = os.path.join(HERE, "out")
FRAMES = os.path.join(HERE, "frames")
sys.path.insert(0, os.path.dirname(HERE))
from okmcp import Mcp  # noqa: E402

frames = sorted(glob.glob(os.path.join(FRAMES, "*.png")))
if not frames:
    sys.exit("no frames: render them first with `cargo test -p ok-photo --test scan`")
video = os.path.join(HERE, "turntable.mp4")
# A 3 second video of the turn, compressed as a phone would.
subprocess.run(
    ["ffmpeg", "-y", "-loglevel", "error", "-framerate", "12", "-i", os.path.join(FRAMES, "%04d.png"),
     "-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18", video],
    check=True,
)
print(f"encoded {len(frames)} frames into {video}, {os.path.getsize(video) // 1024} KB")
os.makedirs(OUT, exist_ok=True)
doc = os.path.join(OUT, "scan.okpart")
if os.path.exists(doc):
    os.remove(doc)
mcp = Mcp(doc)
mcp.call("create_document", {"name": "turntable_scan"})
print(mcp.call("scan_turntable", {"video": video, "out": OUT, "fps": 12}))
print(mcp.call("import", {"path": os.path.join(OUT, "scan.stl"), "name": "gourd scan"}))
# The frame picture the scanner writes is a full-size PNG; the committed copy is a smaller JPEG.
subprocess.run(
    ["ffmpeg", "-y", "-loglevel", "error", "-i", os.path.join(OUT, "scan-frame.png"), "-vf", "scale=900:-1",
     "-q:v", "4", os.path.join(OUT, "scan-frame.jpg")],
    check=True,
)
for view, name in (("iso", "scan-mesh"), ("front", "scan-mesh-front")):
    mcp.call("screenshot", {"tab": 1, "view": view, "width": 1000, "height": 800, "path": os.path.join(OUT, f"{name}.png")})
mcp.close()
