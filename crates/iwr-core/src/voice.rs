//! Generated codecast voice (`iwr voice`, the UI's Generate page): the Kokoro voices, the
//! languages they speak, and the JSON the server and the page exchange (status of every part,
//! a generation request, the progress of a running job).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The voice `iwr voice` uses unless told otherwise.
pub const DEFAULT_VOICE: &str = "af_heart";

/// A language Kokoro can record a codecast in.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct VoiceLang {
    /// what `iwr voice --lang` takes: Kokoro's (`en-us`, `en-gb`) for English, else a codecast language (`ja`)
    pub arg: &'static str,
    /// the codecast language its recordings are for (`en`, `ja`): `NN-part.ogg` or `NN-part.ja.ogg`
    pub code: &'static str,
    /// Kokoro's language
    pub kokoro: &'static str,
    pub name: &'static str,
    /// the voice picked when none is given
    pub default_voice: &'static str,
    /// a sentence to preview a voice with
    pub sample: &'static str,
}

pub const VOICE_LANGS: &[VoiceLang] = &[
    VoiceLang { arg: "en-us", code: "en", kokoro: "en-us", name: "English (US)", default_voice: "af_heart", sample: "Welcome to this codecast. First, the parser reads every file, then the resolver links the calls." },
    VoiceLang { arg: "en-gb", code: "en", kokoro: "en-gb", name: "English (UK)", default_voice: "bf_emma", sample: "Welcome to this codecast. First, the parser reads every file, then the resolver links the calls." },
    VoiceLang { arg: "ja", code: "ja", kokoro: "ja", name: "日本語", default_voice: "jf_alpha", sample: "このコードキャストへようこそ。まずパーサーがすべてのファイルを読み、次にリゾルバーが呼び出しをつなぎます。" },
    VoiceLang { arg: "fr", code: "fr", kokoro: "fr-fr", name: "Français", default_voice: "ff_siwis", sample: "Bienvenue dans ce codecast. D'abord, l'analyseur lit chaque fichier, puis le résolveur relie les appels." },
    VoiceLang { arg: "es", code: "es", kokoro: "es", name: "Español", default_voice: "ef_dora", sample: "Bienvenido a este codecast. Primero, el analizador lee cada archivo y luego el resolvedor enlaza las llamadas." },
    VoiceLang { arg: "it", code: "it", kokoro: "it", name: "Italiano", default_voice: "if_sara", sample: "Benvenuto in questo codecast. Prima il parser legge ogni file, poi il resolver collega le chiamate." },
    VoiceLang { arg: "pt", code: "pt", kokoro: "pt-br", name: "Português", default_voice: "pf_dora", sample: "Bem-vindo a este codecast. Primeiro, o analisador lê cada arquivo, depois o resolvedor liga as chamadas." },
];

/// A language by its `--lang` argument or its codecast code (`ja` → the Japanese entry, `en` → US English).
pub fn voice_lang(arg: &str) -> Option<&'static VoiceLang> {
    VOICE_LANGS.iter().find(|l| l.arg == arg).or_else(|| VOICE_LANGS.iter().find(|l| l.code == arg))
}

/// Kokoro v1.0 voices, best first within a language. The id says it all: language letter (`a`
/// American, `b` British, `j` Japanese, `f` French, `e` Spanish, `i` Italian, `p` Portuguese),
/// `f`/`m` for a female/male voice, then the name.
pub const VOICES: &[&str] = &[
    "af_heart", "af_bella", "af_nicole", "af_aoede", "af_kore", "af_sarah", "af_nova", "af_alloy", "af_sky", "af_jessica", "af_river",
    "am_fenrir", "am_michael", "am_puck", "am_echo", "am_eric", "am_liam", "am_onyx", "am_adam",
    "bf_emma", "bf_isabella", "bf_alice", "bf_lily", "bm_fable", "bm_george", "bm_lewis", "bm_daniel",
    "jf_alpha", "jf_gongitsune", "jf_tebukuro", "jf_nezumi", "jm_kumo",
    "ff_siwis",
    "ef_dora", "em_alex",
    "if_sara", "im_nicola",
    "pf_dora", "pm_alex",
];

/// The best-sounding voices, marked in the picker.
pub const FEATURED: &[&str] = &["af_heart", "af_bella", "am_fenrir", "am_michael", "bf_emma", "bm_fable", "jf_alpha", "ff_siwis", "ef_dora", "if_sara", "pf_dora"];

/// The voices of a language (by the first letter of their id).
pub fn voices_for(l: &VoiceLang) -> impl Iterator<Item = &'static str> + '_ {
    let letter = l.default_voice.chars().next();
    VOICES.iter().copied().filter(move |v| v.chars().next() == letter)
}

/// `af_heart` → (`Heart`, female?).
pub fn voice_label(id: &str) -> (String, bool) {
    let name = id.split_once('_').map(|(_, n)| n).unwrap_or(id);
    let mut c = name.chars();
    let name = c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default();
    (name, id.as_bytes().get(1) == Some(&b'f'))
}

/// One recording of a part, in one language.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Recording {
    /// `recorded` (plays), `stale` (the text changed since: TTS speaks the part), `own` (a
    /// recording `iwr voice` did not make: kept unless replacing is asked for)
    pub state: String,
    /// the Kokoro voice it was made with
    #[serde(default)]
    pub voice: Option<String>,
    #[serde(default)]
    pub speed: Option<f32>,
    /// seconds
    #[serde(default)]
    pub duration: Option<f64>,
    pub kb: u64,
}

/// A part file of a `codecast/` directory and its recordings.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PartStatus {
    /// file stem: `01-welcome`
    pub stem: String,
    /// the part's `##` heading
    pub name: String,
    pub cues: usize,
    /// why it cannot be recorded, if so
    #[serde(default)]
    pub skip: Option<String>,
    /// codecast language (`en`, `ja`) → its recording
    #[serde(default)]
    pub recordings: BTreeMap<String, Recording>,
}

/// What the Generate page shows: the parts, and whether Kokoro is set up yet.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct VoiceStatus {
    /// the codecast directory, or why there is nothing to record
    pub dir: Option<String>,
    #[serde(default)]
    pub problem: Option<String>,
    pub parts: Vec<PartStatus>,
    /// Kokoro (venv + model) installed: the first generation downloads it otherwise
    pub ready: bool,
}

/// A generation the page asks for (`POST /api/voice`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VoiceRequest {
    pub voice: String,
    pub speed: f32,
    /// a [`VoiceLang::arg`]
    pub lang: String,
    pub gap: f32,
    pub level: f32,
    /// replace recordings `iwr voice` did not make
    #[serde(default)]
    pub force: bool,
    /// part stems to record (each one, up to date or not); empty = every part that needs it
    #[serde(default)]
    pub parts: Vec<String>,
}

impl Default for VoiceRequest {
    fn default() -> Self {
        Self { voice: DEFAULT_VOICE.into(), speed: 1.0, lang: "en-us".into(), gap: 0.4, level: 0.95, force: false, parts: vec![] }
    }
}

/// A part being recorded.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct JobPart {
    pub stem: String,
    pub cues: usize,
    /// cues synthesized (or found in the clip cache) so far
    pub done: usize,
    /// set once its file is written
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub kb: u64,
}

/// A generation's progress (`GET /api/voice/job`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct VoiceJob {
    /// bumps with every new job
    pub id: u64,
    pub running: bool,
    /// what it is doing: setting up, translating, recording
    pub stage: String,
    pub parts: Vec<JobPart>,
    /// seconds since it started
    pub elapsed: f64,
    #[serde(default)]
    pub error: Option<String>,
    /// lines worth showing: skipped parts, setup steps, the sidecar's warnings
    #[serde(default)]
    pub log: Vec<String>,
    pub request: Option<VoiceRequest>,
}

impl VoiceJob {
    pub fn total(&self) -> usize {
        self.parts.iter().map(|p| p.cues).sum()
    }

    pub fn done(&self) -> usize {
        self.parts.iter().map(|p| p.done.min(p.cues)).sum()
    }

    /// The part being recorded: the first one without its file.
    pub fn current(&self) -> Option<usize> {
        self.parts.iter().position(|p| p.duration.is_none())
    }
}
