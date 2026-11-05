# How-to videos

Short narrated videos of SpecDeck, made from the UI's demo mode with
[HyperFrames](https://hyperframes.heygen.com) and a local [Kokoro](https://huggingface.co/hexgrad/Kokoro-82M) voice.
Only the files that make a video are kept here; screenshots, voice clips, `index.html` and the
render are made again each time.

| Video | Shows | Length |
| - | - | - |
| `getting-started` | Hosting a session, sharing the code, a teammate joining, People and End session | about 1:00 |
| `following` | Overview or Developer, the steps and the host's decisions, View, search, Files and the status bar | about 1:30 |
| `diagnostics` | The Connection status panel, Export Diagnostics and Verbose logging | about 1:30 |

```sh
just video                    # all three
just video getting-started    # one
```

This needs Google Chrome, Python 3 and ffmpeg, and the app's own `pnpm install`. Each video ends
up in `<video>/renders/specdeck-<video>.mp4`, and a smaller copy with a poster in
`docs/site/public/video/`, which the documentation pages play and the main README links to;
commit those after a change. The first run downloads the Kokoro model (about 340 MB) to
`~/.cache/kokoro`.

Each video's folder has:

| File | Does |
| - | - |
| `SCRIPT.json` | The narration, one line per scene. Edit the words here. |
| `capture.mjs` | Screenshots the demo app and records where the parts to highlight are. |
| `build.mjs` | Writes `index.html`, the HyperFrames composition: each scene as long as its line, with the zooms, highlights and captions. |

and `lib/` what they share:

| File | Does |
| - | - |
| `capture.mjs` | Opens the demo in Chrome, starting the UI dev server if it is not running. Sets the clock just after the demo's events, so "for how long" reads seconds, and hides the demo's update notice. |
| `speak.py` | Speaks each line of a video's script with Kokoro and measures it: `speak.py <video> [voice]`, for example `am_michael`. |
| `film.mjs` | Scene timing, the app window with its camera and highlight ring, the title and closing cards, and the captions. |

After editing only the words, `just video <name>` redoes everything; to look before rendering, run
`pnpm exec hyperframes preview` in the video's folder.

The demo has no native menu, so the diagnostics capture opens the panel through `window.demoMenu`,
which the demo bridge (`app/ui/src/api/demo.ts`) sets in place of the Help menu.
