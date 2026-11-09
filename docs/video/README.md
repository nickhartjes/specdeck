# How-to videos

Short narrated videos of SpecDeck, made from the UI's demo mode with
[HyperFrames](https://hyperframes.heygen.com) and a local [Kokoro](https://huggingface.co/hexgrad/Kokoro-82M) voice.
Only the files that make a video are kept here; screenshots, voice clips, `index.html` and the
render are made again each time.

| Video | Shows | Length |
| - | - | - |
| `getting-started` | Hosting a session, sharing the code, a teammate joining, People and End session | about 1:00 |
| `following` | Overview or Developer, the steps and the host's decisions, the layout menu, search, Files and the status bar | about 1:30 |
| `diagnostics` | The Connection status panel, Export Diagnostics and Verbose logging | about 1:30 |
| `taking-part` | Suggesting a prompt and what became of it, the host's Prompts tab, sending a link, the braindump and Give | about 1:30 |
| `web-viewer` | Opening the web link, the code and your name, the session in a browser, the menu and the keyboard shortcuts | about 1:05 |
| `read-and-estimate` | Ask to read with a time limit, viewers' answers, Estimate, hidden votes, Reveal and Keep | about 1:05 |
| `plans` | The plan card, a document tab with the plan's other documents, the task count, New and Changed marks, and Source | about 1:00 |
| `claude-browser` | Claude's browser steps in the timeline, a screenshot full size, and the Settings for the browser and screenshots | about 1:00 |
| `self-hosting` | The Docker Compose guide's DNS, firewall and `.env`, a real `docker compose` run, and the app's Sharing service | about 1:10 |

```sh
just video                    # all of them
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
| `capture.mjs` | Opens the demo in Chrome, starting the UI dev server if it is not running: the app's (:5173), or with `server: 'web'` the web viewer's (:5174), whose `web-demo.html` shows the web page with the demo session. Sets the clock just after the demo's events, so "for how long" reads seconds, and hides the demo's update notice. |
| `speak.py` | Speaks each line of a video's script with Kokoro and measures it: `speak.py <video> [voice]`, for example `am_michael`. |
| `words.mjs` | Finds when each spoken word starts, with `hyperframes transcribe` (a local Whisper), so highlights and captions follow the voice. |
| `film.mjs` | Scene timing, the app window with its camera and highlight ring, the title and closing cards, and the captions. |

After editing only the words, `just video <name>` redoes everything; to look before rendering, run
`pnpm exec hyperframes preview` in the video's folder.

Some videos need demo content the default demo does not have, which `?scene=` adds:
`plans` (an OpenSpec change the agent plans and then changes) and `browser` (Claude's browser
steps and a screenshot, `demo-browser.png`). Without `scene` the demo is unchanged.

`self-hosting` shows a real `docker compose` run, saved in `self-hosting/run/`, so rendering it
needs no Docker. To record it again, for example after a release, run
`DOCKER_CONTEXT=<context> docs/video/self-hosting/record.sh` against a Docker whose ports 80, 443
and 3478 are free (a separate Colima profile works: `colima start --profile compose`). It leaves
out the TURN secret and every address but 127.0.0.1. Its capture shows the Docker Compose page
from the built site, so run `just docs-build` first.

The demo has no native menu, so the diagnostics capture opens the panel through `window.demoMenu`,
which the demo bridge (`app/ui/src/api/demo.ts`) sets in place of the Help menu.
