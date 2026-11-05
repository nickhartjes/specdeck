"""Speaks each line of a video's SCRIPT.json with Kokoro and records its length.

Writes <video>/assets/voice/<id>.wav and durations.json. Downloads the Kokoro model to
~/.cache/kokoro the first time. Usage: speak.py <video folder> [voice] [id ...]; the voice
defaults to af_heart.
"""
import json
import pathlib
import shutil
import sys
import urllib.request

import espeakng_loader
import soundfile
from kokoro_onnx import EspeakConfig, Kokoro

MODEL = "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/"
CACHE = pathlib.Path.home() / ".cache"

project = pathlib.Path(sys.argv[1])
out = project / "assets" / "voice"
out.mkdir(parents=True, exist_ok=True)
voice = sys.argv[2] if len(sys.argv) > 2 else "af_heart"
only = set(sys.argv[3:])


def model_file(name: str) -> str:
    path = CACHE / "kokoro" / name
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        print(f"downloading {name}")
        urllib.request.urlretrieve(MODEL + name, path)
    return str(path)


def espeak_data() -> str:
    # espeak-ng ignores a data path of 160 characters or more and phonemizer resolves symlinks,
    # so a deep checkout gets a copy of the bundled data at a short path.
    data = espeakng_loader.get_data_path()
    if len(data) < 160:
        return data
    copy = CACHE / "espeak-ng-data"
    if not copy.exists():
        shutil.copytree(data, copy)
    return str(copy)


kokoro = Kokoro(
    model_file("kokoro-v1.0.onnx"),
    model_file("voices-v1.0.bin"),
    espeak_config=EspeakConfig(lib_path=espeakng_loader.get_library_path(), data_path=espeak_data()),
)
durations = {}
for line in json.loads((project / "SCRIPT.json").read_text()):
    path = out / f"{line['id']}.wav"
    if not only or line["id"] in only:
        samples, rate = kokoro.create(line["text"], voice=voice, speed=1.0, lang="en-us")
        soundfile.write(path, samples, rate)
    durations[line["id"]] = round(soundfile.info(path).duration, 3)
    print(line["id"], durations[line["id"]])
(out / "durations.json").write_text(json.dumps(durations, indent=2))
