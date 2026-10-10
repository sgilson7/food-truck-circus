// Sound, synthesized from data/sounds.json, and the crowd's voice. The core
// says what happened and which word was called; this file only performs it.
// The white noise comes from the core's generator, since the page may not
// draw random numbers. OH NO and DARN are Sam's own recordings (Slushline's);
// every other word is spoken by the browser's built-in speech voice.
import { noise } from "./pkg/ftc_shim.js";

let ctx = null;
let master = null;
let noiseBuffer = null;
let recipes = null;
let sayings = null;
let voiceLang = "";
let useClips = true;
let muted = false;
const clips = new Map();

// The crowd's words come from the copy of the language in use: Sam's two
// recordings in English, the browser's own voice in that language otherwise.
export function setup(sounds, copy) {
  recipes = sounds;
  sayings = copy.say;
  voiceLang = copy.voice_lang;
  useClips = copy.voice_clips;
}

async function loadClips(build) {
  for (const [word, path] of Object.entries(recipes.clips)) {
    try {
      const bytes = await fetch(`${path}?v=${build}`).then((r) => r.arrayBuffer());
      clips.set(word, await ctx.decodeAudioData(bytes));
    } catch {
      // without the clip the word is spoken instead
    }
  }
}

// Browsers start audio only after a person presses something.
export function wake(build) {
  if (!recipes) return;
  if (!ctx) {
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return;
    ctx = new AC({ latencyHint: "interactive" });
    master = ctx.createGain();
    master.gain.value = recipes.master;
    master.connect(ctx.destination);
    const data = noise(ctx.sampleRate * recipes.noise_seconds, recipes.seed);
    noiseBuffer = ctx.createBuffer(1, data.length, ctx.sampleRate);
    noiseBuffer.copyToChannel(data, 0);
    loadClips(build);
  }
  if (ctx.state === "suspended") ctx.resume().catch(() => {});
}

export function toggleMute() {
  muted = !muted;
  if (master) master.gain.value = muted ? 0 : recipes.master;
  if (muted && window.speechSynthesis) window.speechSynthesis.cancel();
  return muted;
}

function layer(l, level) {
  const t0 = ctx.currentTime + l.at;
  const g = ctx.createGain();
  g.gain.setValueAtTime(recipes.floor, t0);
  g.gain.linearRampToValueAtTime(l.gain * level, t0 + l.attack);
  g.gain.exponentialRampToValueAtTime(recipes.floor, t0 + l.dur);
  g.connect(master);
  let src;
  if (l.type === "osc") {
    src = ctx.createOscillator();
    src.type = l.wave;
    src.frequency.setValueAtTime(l.from, t0);
    src.frequency.exponentialRampToValueAtTime(l.to, t0 + l.dur);
    src.connect(g);
  } else {
    src = ctx.createBufferSource();
    src.buffer = noiseBuffer;
    src.loop = true;
    const f = ctx.createBiquadFilter();
    f.type = l.filter;
    f.frequency.value = l.freq;
    f.Q.value = l.q;
    src.connect(f);
    f.connect(g);
  }
  src.start(t0);
  src.stop(t0 + l.dur + recipes.tail);
}

export function play(kind, level) {
  if (!ctx || muted) return;
  const recipe = recipes.recipes[kind];
  if (!recipe) return;
  const scale = level / recipes.percent;
  for (const l of recipe) layer(l, scale);
}

// The crowd calls a word: a cheer or a groan under it, then the word itself.
export function call(word) {
  if (!ctx || muted) return;
  const crowd = recipes.words[word];
  if (crowd) play(crowd, recipes.percent);
  const clip = useClips ? clips.get(word) : null;
  if (clip) {
    const src = ctx.createBufferSource();
    const g = ctx.createGain();
    g.gain.value = recipes.clip_gain;
    src.buffer = clip;
    src.connect(g);
    g.connect(master);
    src.start();
    return;
  }
  const line = sayings[word];
  if (!line || !window.speechSynthesis || !window.SpeechSynthesisUtterance) return;
  const u = new SpeechSynthesisUtterance(line);
  u.rate = recipes.voice.rate;
  u.pitch = recipes.voice.pitch;
  u.volume = recipes.voice.volume;
  u.lang = voiceLang;
  window.speechSynthesis.cancel();
  window.speechSynthesis.speak(u);
}

// Leaving the page with sound still scheduled made Firefox report an error
// in Slushline, so the context closes first.
window.addEventListener("pagehide", () => {
  if (ctx) {
    const c = ctx;
    ctx = null;
    c.close().catch(() => {});
  }
});
