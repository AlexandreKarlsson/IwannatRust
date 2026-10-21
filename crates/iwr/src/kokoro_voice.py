"""Kokoro TTS for `iwr voice`: reads a job (JSON) on stdin, writes one audio file per part.
Progress goes to stderr as `@` lines the CLI reads; anything else there is a log line.

Job: {"model", "voices", "voice", "speed", "lang", "gap", "level", "cache",
      "parts": [{"out": "/abs/01-welcome.ogg", "cues": ["spoken text", ...]}]}
Output is Ogg Opus; `level` is libsndfile's compression level (0.95 ≈ 18 kbps, clear speech).
Progress: `@ready` once the model is loaded, `@cue P I N` when cue I (1-based) of N of part P
(0-based) is done, `@part {"out", "times": [0.0, 4.2, ...], "duration"}` when a part's file is written.

Each cue is synthesized alone (cached by text + voice + speed), so its start time is exact and
editing one cue only re-synthesizes that cue.

Japanese (`lang` "ja"): espeak cannot read kana or kanji, so the text goes through misaki's
Japanese G2P (pyopenjtalk) and Kokoro speaks the phonemes; Latin runs inside (code, names) are
phonemized as English by espeak.
"""

import hashlib
import json
import os
import re
import sys

import numpy as np
import soundfile as sf
from kokoro_onnx import Kokoro


def japanese(kokoro):
    from misaki import ja

    g2p = ja.JAG2P()

    def phonemes(text):
        out = []
        # Latin runs (`module_for`, "Rust") as English, the rest as Japanese
        for i, run in enumerate(re.split(r"([A-Za-z][A-Za-z0-9 '_-]*[A-Za-z0-9]|[A-Za-z])", text)):
            if not run.strip():
                continue
            if i % 2:
                out.append(kokoro.tokenizer.phonemize(run, "en-us"))
            else:
                r = g2p(run)
                out.append(r[0] if isinstance(r, tuple) else r)
        return " ".join(p.strip() for p in out if p and p.strip())

    return phonemes


def main():
    job = json.load(sys.stdin)
    kokoro = Kokoro(job["model"], job["voices"])
    print("@ready", file=sys.stderr, flush=True)
    g2p = japanese(kokoro) if job["lang"] == "ja" else None
    os.makedirs(job["cache"], exist_ok=True)
    sr = 24000
    gap = np.zeros(int(job["gap"] * sr), dtype=np.float32)
    for p, part in enumerate(job["parts"]):
        chunks, times, t = [], [], 0.0
        for i, text in enumerate(part["cues"]):
            key = hashlib.sha256(f'{job["voice"]}|{job["speed"]}|{job["lang"]}|{text}'.encode()).hexdigest()[:24]
            clip = os.path.join(job["cache"], key + ".wav")
            if os.path.exists(clip):
                audio, _ = sf.read(clip, dtype="float32")
            elif text.strip():
                if g2p:
                    audio, sr = kokoro.create(g2p(text), voice=job["voice"], speed=job["speed"], is_phonemes=True)
                else:
                    audio, sr = kokoro.create(text, voice=job["voice"], speed=job["speed"], lang=job["lang"])
                sf.write(clip, audio, sr)
            else:
                audio = np.zeros(int(0.5 * sr), dtype=np.float32)
            print(f'@cue {p} {i + 1} {len(part["cues"])}', file=sys.stderr, flush=True)
            times.append(round(t, 2))
            chunks += [audio, gap]
            t += (len(audio) + len(gap)) / sr
        audio = np.concatenate(chunks) if chunks else gap
        # written aside then moved: a cancelled job never leaves half a file
        sf.write(part["out"] + ".part", audio, sr, format="OGG", subtype="OPUS", compression_level=job["level"])
        os.replace(part["out"] + ".part", part["out"])
        print("@part " + json.dumps({"out": part["out"], "times": times, "duration": round(t, 2)}), file=sys.stderr, flush=True)


main()
