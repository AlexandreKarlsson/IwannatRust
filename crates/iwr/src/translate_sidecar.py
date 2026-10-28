"""Offline translation for codecast: reads a job (JSON) on stdin, prints the translations (JSON list).

Job: {"home": "/abs/cache/dir", "lang": "ja", "texts": ["English text", ...]}

Models are Argos Translate packages (OpenNMT models run by CTranslate2, SentencePiece tokens),
downloaded once into <home>/en_<lang>. Argos' own Python package is not used: it pulls in torch
for sentence splitting. Texts are split into sentences here; code in backticks is left to the
model, which copies it through.
"""

import json
import os
import re
import sys
import urllib.request
import zipfile

import ctranslate2
import sentencepiece as spm

INDEX = "https://raw.githubusercontent.com/argosopentech/argospm-index/main/index.json"
# no space between sentences in these
CJK = {"ja", "zh", "ko"}


def fetch(url):
    # the default urllib User-Agent is refused (403)
    return urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "codecast"}))


def model_dir(home, lang):
    d = os.path.join(home, f"en_{lang}")
    if os.path.isdir(os.path.join(d, "model")):
        return d
    with fetch(INDEX) as r:
        index = json.load(r)
    pkg = next((p for p in index if p["from_code"] == "en" and p["to_code"] == lang), None)
    if pkg is None:
        sys.exit(f"no offline model translates English to {lang}")
    url = pkg["links"][0]
    print(f"downloading the en→{lang} model ({url})…", file=sys.stderr, flush=True)
    tmp = d + ".zip.part"
    with fetch(url) as r, open(tmp, "wb") as f:
        while chunk := r.read(1 << 20):
            f.write(chunk)
    with zipfile.ZipFile(tmp) as z:
        # the archive holds one top folder (en_ja/…): unpack its model and tokenizer under d
        for name in z.namelist():
            rel = name.split("/", 1)[1] if "/" in name else ""
            if not (rel.startswith("model/") or rel == "sentencepiece.model") or name.endswith("/"):
                continue
            out = os.path.join(d, rel)
            os.makedirs(os.path.dirname(out), exist_ok=True)
            with z.open(name) as src, open(out, "wb") as dst:
                dst.write(src.read())
    os.remove(tmp)
    return d


def sentences(text):
    return [s for s in re.split(r"(?<=[.!?])\s+(?=[A-Z`\"(])", text.strip()) if s]


def main():
    job = json.load(sys.stdin)
    lang = job["lang"]
    d = model_dir(job["home"], lang)
    sp = spm.SentencePieceProcessor(model_file=os.path.join(d, "sentencepiece.model"))
    tr = ctranslate2.Translator(os.path.join(d, "model"), device="cpu", compute_type="int8")
    split = [sentences(t) for t in job["texts"]]
    flat = [s for ss in split for s in ss]
    print(f"translating {len(job['texts'])} text(s), {len(flat)} sentence(s) to {lang}…", file=sys.stderr, flush=True)
    done = tr.translate_batch([sp.encode(s, out_type=str) for s in flat], beam_size=2, max_batch_size=32) if flat else []
    # not sp.decode: the target vocabulary is larger than the tokenizer's, and sp.decode copies
    # the pieces it doesn't know with their ▁ word marks
    said = iter("".join(r.hypotheses[0]).replace("▁", " ").strip() for r in done)
    sep = "" if lang in CJK else " "
    json.dump([sep.join(next(said) for _ in ss) for ss in split], sys.stdout, ensure_ascii=False)


main()
