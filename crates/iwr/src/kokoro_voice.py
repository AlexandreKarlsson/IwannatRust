"""Kokoro TTS for `iwr voice`: reads a job (JSON) on stdin, writes one audio file per part,
prints the start time of every cue (JSON) on stdout. Progress goes to stderr.

Job: {"model", "voices", "voice", "speed", "lang", "gap", "level", "cache",
      "parts": [{"out": "/abs/01-welcome.ogg", "cues": ["spoken text", ...]}]}
Output is Ogg Opus; `level` is libsndfile's compression level (0.95 ≈ 18 kbps, clear speech).
Result: [{"out", "times": [0.0, 4.2, ...], "duration"}]

Each cue is synthesized alone (cached by text + voice + speed), so its start time is exact and
editing one cue only re-synthesizes that cue.
"""

import hashlib
import json
import os
import sys

import numpy as np
import soundfile as sf
from kokoro_onnx import Kokoro


def main():
    job = json.load(sys.stdin)
    kokoro = Kokoro(job["model"], job["voices"])
    os.makedirs(job["cache"], exist_ok=True)
    sr = 24000
    gap = np.zeros(int(job["gap"] * sr), dtype=np.float32)
    result = []
    for part in job["parts"]:
        chunks, times, t = [], [], 0.0
        for i, text in enumerate(part["cues"]):
            key = hashlib.sha256(f'{job["voice"]}|{job["speed"]}|{job["lang"]}|{text}'.encode()).hexdigest()[:24]
            clip = os.path.join(job["cache"], key + ".wav")
            if os.path.exists(clip):
                audio, _ = sf.read(clip, dtype="float32")
            elif text.strip():
                print(f'  {os.path.basename(part["out"])} cue {i + 1}/{len(part["cues"])}', file=sys.stderr, flush=True)
                audio, sr = kokoro.create(text, voice=job["voice"], speed=job["speed"], lang=job["lang"])
                sf.write(clip, audio, sr)
            else:
                audio = np.zeros(int(0.5 * sr), dtype=np.float32)
            times.append(round(t, 2))
            chunks += [audio, gap]
            t += (len(audio) + len(gap)) / sr
        audio = np.concatenate(chunks) if chunks else gap
        sf.write(part["out"], audio, sr, format="OGG", subtype="OPUS", compression_level=job["level"])
        result.append({"out": part["out"], "times": times, "duration": round(t, 2)})
    json.dump(result, sys.stdout)


main()
