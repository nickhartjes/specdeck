# How-to videos

Short narrated videos of SpecDeck features, made from the UI's demo mode with
[HyperFrames](https://hyperframes.heygen.com) and a local [Kokoro](https://huggingface.co/hexgrad/Kokoro-82M) voice.
Only the files that make a video are kept here; screenshots, voice clips, `index.html` and the
render are made again each time.

## diagnostics

The Connection status panel, Export Diagnostics and Verbose logging, about 1:30.

```sh
just video
```

This needs Google Chrome, Python 3 and ffmpeg, and the app's own `pnpm install`. It writes
`diagnostics/renders/specdeck-diagnostics.mp4`. The first run downloads the Kokoro model (about
340 MB) to `~/.cache/kokoro`.

The steps, each in `diagnostics/`:

| File | Does |
| - | - |
| `SCRIPT.json` | The narration, one line per scene. Edit the words here. |
| `capture.mjs` | Screenshots the demo app (`/?demo=host`, `/?demo=viewer`, Settings) and records where the parts to highlight are. Starts the UI dev server if it is not running. |
| `speak.py` | Speaks each line with Kokoro and measures it. Pass a voice to change it: `speak.py am_michael`. |
| `build.mjs` | Writes `index.html`, the HyperFrames composition: each scene as long as its line, with the zooms, highlights and captions. |

After editing only the words, `just video` redoes everything; to look before rendering, run
`pnpm exec hyperframes preview` in `diagnostics/`.

The demo has no native menu, so `capture.mjs` opens the panel through `window.demoMenu`, which
the demo bridge (`app/ui/src/api/demo.ts`) sets in place of the Help menu.
