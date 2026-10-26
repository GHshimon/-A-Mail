#!/usr/bin/env bash
set -euo pipefail

DEMO_DIR="$(cd "$(dirname "$0")" && pwd)"
DEMO_NODE="${DEMO_NODE:-$(command -v node)}"
DEMO_BUNDLED_MODULES="/Users/shimon/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules"
DEMO_FFMPEG="${DEMO_FFMPEG:-/opt/homebrew/bin/ffmpeg}"

if [[ -d "$DEMO_BUNDLED_MODULES" ]]; then
  export NODE_PATH="${NODE_PATH:-$DEMO_BUNDLED_MODULES}"
fi

"$DEMO_NODE" "$DEMO_DIR/render_phase_b_mobile_demo.mjs"

"$DEMO_FFMPEG" -hide_banner -loglevel warning -y \
  -loop 1 -framerate 30 -t 7.5 -i "$DEMO_DIR/frames/01-title.png" \
  -loop 1 -framerate 30 -t 7.5 -i "$DEMO_DIR/frames/02-context-boundary.png" \
  -loop 1 -framerate 30 -t 7.5 -i "$DEMO_DIR/frames/03-connect.png" \
  -loop 1 -framerate 30 -t 7.5 -i "$DEMO_DIR/frames/04-browser-auth.png" \
  -loop 1 -framerate 30 -t 7.5 -i "$DEMO_DIR/frames/05-opt-in.png" \
  -loop 1 -framerate 30 -t 7.5 -i "$DEMO_DIR/frames/06-generate-candidate.png" \
  -loop 1 -framerate 30 -t 7.5 -i "$DEMO_DIR/frames/07-human-review.png" \
  -loop 1 -framerate 30 -t 7.5 -i "$DEMO_DIR/frames/08-status.png" \
  -filter_complex "\
    [0:v]fps=30,trim=duration=7.5,setpts=PTS-STARTPTS,fade=t=in:st=0:d=0.28,fade=t=out:st=7.22:d=0.28[v0];\
    [1:v]fps=30,trim=duration=7.5,setpts=PTS-STARTPTS,fade=t=in:st=0:d=0.28,fade=t=out:st=7.22:d=0.28[v1];\
    [2:v]fps=30,trim=duration=7.5,setpts=PTS-STARTPTS,fade=t=in:st=0:d=0.28,fade=t=out:st=7.22:d=0.28[v2];\
    [3:v]fps=30,trim=duration=7.5,setpts=PTS-STARTPTS,fade=t=in:st=0:d=0.28,fade=t=out:st=7.22:d=0.28[v3];\
    [4:v]fps=30,trim=duration=7.5,setpts=PTS-STARTPTS,fade=t=in:st=0:d=0.28,fade=t=out:st=7.22:d=0.28[v4];\
    [5:v]fps=30,trim=duration=7.5,setpts=PTS-STARTPTS,fade=t=in:st=0:d=0.28,fade=t=out:st=7.22:d=0.28[v5];\
    [6:v]fps=30,trim=duration=7.5,setpts=PTS-STARTPTS,fade=t=in:st=0:d=0.28,fade=t=out:st=7.22:d=0.28[v6];\
    [7:v]fps=30,trim=duration=7.5,setpts=PTS-STARTPTS,fade=t=in:st=0:d=0.28,fade=t=out:st=7.22:d=0.28[v7];\
    [v0][v1][v2][v3][v4][v5][v6][v7]concat=n=8:v=1:a=0,format=yuv420p[v]" \
  -map "[v]" -an -c:v libx264 -profile:v high -level 4.1 -preset medium -crf 20 \
  -movflags +faststart "$DEMO_DIR/phase-b-mobile-demo.mp4"

echo "Built $DEMO_DIR/phase-b-mobile-demo.mp4"
