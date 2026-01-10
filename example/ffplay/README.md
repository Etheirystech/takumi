# FFplay Example

Render video frames in real-time using Takumi and display them with `ffplay`.

## Prerequisites

- [Bun](https://bun.sh)
- [FFmpeg](https://ffmpeg.org) (specifically `ffplay`)

## Usage

```bash
bun install
bun run src/index.ts
```

Press `Ctrl+C` to exit.

## How It Works

1. Renders a timer using Takumi at 60fps
2. Outputs raw RGBA frames
3. Pipes frames to `ffplay` for real-time playback
