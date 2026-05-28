const els = {
  start: document.querySelector("#start"),
  stop: document.querySelector("#stop"),
  clear: document.querySelector("#clear"),
  socketState: document.querySelector("#socketState"),
  audioState: document.querySelector("#audioState"),
  rate: document.querySelector("#rate"),
  frames: document.querySelector("#frames"),
  modelName: document.querySelector("#modelName"),
  signalPanel: document.querySelector(".signal-panel"),
  signalState: document.querySelector("#signalState"),
  signalValue: document.querySelector("#signalValue"),
  signalWaveGlow: document.querySelector("#signalWaveGlow"),
  signalWaveTrail: document.querySelector("#signalWaveTrail"),
  signalWavePrimary: document.querySelector("#signalWavePrimary"),
  level: document.querySelector("#level"),
  partial: document.querySelector("#partial"),
  final: document.querySelector("#final"),
  events: document.querySelector("#events"),
  apiKey: document.querySelector("#apiKey"),
};

let ws = null;
let mediaStream = null;
let audioContext = null;
let source = null;
let worklet = null;
let mutedOutput = null;
let frameCount = 0;
let isRecording = false;
let transcriptText = "";
let committedTranscriptText = "";
let signalTargetLevel = 0;
let signalDisplayLevel = 0;
let signalLastState = "quiet";
let signalPhase = 0;
let signalAnimationFrame = null;
let lastSignalFrameAt = 0;
const MAX_TRANSCRIPT_LINE = 12;
const MAX_EVENT_LINES = 5;
const MAX_WS_BUFFERED_BYTES = 512 * 1024;
const AUDIO_BATCH_MS = 20;
const SIGNAL_EASING = 0.18;
const SIGNAL_IDLE_THRESHOLD = 0.08;
const API_KEY_STORAGE_KEY = "seedrelay.apiKey";
const DEFAULT_RUNTIME_CONFIG = {
  model: "seed-asr",
  authRequired: false,
};
let runtimeConfig = { ...DEFAULT_RUNTIME_CONFIG };
let configReady = Promise.resolve();
let pendingAudioSamples = [];
let pendingAudioSampleCount = 0;
let lastBackpressureLogAt = 0;

const workletSource = `
  class CaptureProcessor extends AudioWorkletProcessor {
    process(inputs) {
      const channel = inputs[0] && inputs[0][0];
      if (channel && channel.length) {
        this.port.postMessage(channel.slice(0));
      }
      return true;
    }
  }
  registerProcessor("capture-processor", CaptureProcessor);
`;

function normalizeRuntimeConfig(config) {
  const model =
    typeof config?.model === "string" && config.model.trim()
      ? config.model.trim()
      : DEFAULT_RUNTIME_CONFIG.model;
  return { model, authRequired: config?.authRequired === true };
}

async function loadRuntimeConfig() {
  try {
    const response = await fetch("/config.json", { cache: "no-store" });
    if (!response.ok) throw new Error(`config ${response.status}`);
    runtimeConfig = normalizeRuntimeConfig(await response.json());
  } catch (error) {
    runtimeConfig = { ...DEFAULT_RUNTIME_CONFIG };
    log(error.message || String(error), "config");
  }
  els.modelName.textContent = runtimeConfig.model;
  els.apiKey.value = localStorage.getItem(API_KEY_STORAGE_KEY) || "";
}

function currentApiKey() {
  return els.apiKey.value.trim();
}

function persistApiKey() {
  const apiKey = currentApiKey();
  if (apiKey) {
    localStorage.setItem(API_KEY_STORAGE_KEY, apiKey);
  } else {
    localStorage.removeItem(API_KEY_STORAGE_KEY);
  }
}

function realtimeUrl() {
  const url = new URL("/v1/realtime", window.location.href);
  url.protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  url.searchParams.set("model", runtimeConfig.model);
  return url.toString();
}

function realtimeProtocols() {
  const protocols = ["realtime"];
  const apiKey = currentApiKey();
  if (apiKey) {
    protocols.push("openai-insecure-api-key." + apiKey);
  }
  return protocols;
}

function displayRealtimeUrl() {
  return realtimeUrl();
}

function log(line, kind = "") {
  const time = new Date().toLocaleTimeString();
  const prefix = kind ? `[${kind}]` : "";
  const nextLine = `${time} ${prefix} ${line}`;
  const lines = els.events.textContent.split("\n").filter(Boolean);
  els.events.textContent = [nextLine, ...lines]
    .slice(0, MAX_EVENT_LINES)
    .join("\n");
}

function setSocket(state, ok = false) {
  els.socketState.textContent = state;
  els.socketState.className = ok ? "ok" : "";
}

function setAudio(state, bad = false) {
  els.audioState.textContent = state;
  els.audioState.className = bad ? "bad" : "";
}

function formatCloseReason(event) {
  const parts = [`code ${event.code}`];
  if (event.reason) parts.push(event.reason);
  if (!event.wasClean) parts.push("unclean");
  return `closed (${parts.join(", ")})`;
}

function formatSocketError(event) {
  if (event && typeof event.message === "string" && event.message) {
    return event.message;
  }
  return "websocket error";
}

function signalStateForPeak(peak) {
  if (peak >= 0.7) return "clipping";
  if (peak >= 0.01) return "voice";
  return "quiet";
}

function signalLevelFromPeak(peak) {
  return Math.min(100, Math.sqrt(Math.min(1, Math.max(0, peak))) * 125);
}

function buildWavePoints(level, phase, offset = 0) {
  const width = 240;
  const center = 48;
  const amplitude = level === 0 ? 0 : 0.8 + level * 0.1;
  const points = [];
  for (let i = 0; i <= 56; i += 1) {
    const t = i / 56;
    const x = t * width;
    const carrier = Math.sin(t * Math.PI * 6 + phase + offset);
    const detail = Math.sin(t * Math.PI * 17 + phase * 0.74 + offset) * 0.24;
    const shimmer = Math.sin(t * Math.PI * 31 - phase * 0.36) * 0.08;
    const y = Math.max(
      4,
      Math.min(92, center + (carrier + detail + shimmer) * amplitude),
    );
    points.push(`${x.toFixed(1)},${y.toFixed(1)}`);
  }
  return points.join(" ");
}

function renderSignalWave(level) {
  const primary = buildWavePoints(level, signalPhase);
  const trail = buildWavePoints(level * 0.58, signalPhase - 0.9, 0.7);
  els.signalWaveGlow.setAttribute("points", primary);
  els.signalWavePrimary.setAttribute("points", primary);
  els.signalWaveTrail.setAttribute("points", trail);
}

function drawSignalFrame(now) {
  signalAnimationFrame = null;
  const frameMs = Math.min(34, Math.max(0, now - lastSignalFrameAt || 16));
  lastSignalFrameAt = now;
  signalDisplayLevel +=
    (signalTargetLevel - signalDisplayLevel) * SIGNAL_EASING;
  if (signalTargetLevel === 0 && signalDisplayLevel < SIGNAL_IDLE_THRESHOLD) {
    signalDisplayLevel = 0;
  }
  els.level.style.width = `${signalDisplayLevel}%`;
  els.signalPanel.style.setProperty("--signal-level", `${signalDisplayLevel}%`);
  els.signalValue.textContent = `${Math.round(signalDisplayLevel)}%`;
  signalPhase =
    (signalPhase + frameMs * (0.0008 + signalDisplayLevel / 52000)) %
    (Math.PI * 2);
  renderSignalWave(signalDisplayLevel);
  if (
    isRecording ||
    signalTargetLevel > SIGNAL_IDLE_THRESHOLD ||
    signalDisplayLevel > SIGNAL_IDLE_THRESHOLD
  ) {
    signalAnimationFrame = requestAnimationFrame(drawSignalFrame);
  }
}

function requestSignalFrame() {
  if (signalAnimationFrame === null) {
    signalAnimationFrame = requestAnimationFrame(drawSignalFrame);
  }
}

function setSignal(peak) {
  const level = signalLevelFromPeak(peak);
  signalTargetLevel =
    peak === 0 ? 0 : Math.max(signalTargetLevel * 0.86, level);
  const nextState = signalStateForPeak(peak);
  if (signalLastState !== nextState) {
    signalLastState = nextState;
    els.signalState.textContent = nextState;
  }
  requestSignalFrame();
}

function pcm16Base64(samples) {
  const bytes = new Uint8Array(samples.length * 2);
  const view = new DataView(bytes.buffer);
  let peak = 0;
  for (let i = 0; i < samples.length; i += 1) {
    const sample = Math.max(-1, Math.min(1, samples[i]));
    peak = Math.max(peak, Math.abs(sample));
    const int16 = sample < 0 ? sample * 0x8000 : sample * 0x7fff;
    view.setInt16(i * 2, int16, true);
  }
  setSignal(peak);

  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}

function sendJson(payload) {
  if (!ws || ws.readyState !== WebSocket.OPEN) return;
  ws.send(JSON.stringify(payload));
}

function resetAudioBatch() {
  pendingAudioSamples = [];
  pendingAudioSampleCount = 0;
}

function enqueueAudioSamples(samples) {
  pendingAudioSamples.push(samples);
  pendingAudioSampleCount += samples.length;

  const batchSamples = Math.max(
    1,
    Math.round((audioContext.sampleRate * AUDIO_BATCH_MS) / 1000),
  );
  if (pendingAudioSampleCount >= batchSamples) {
    flushAudioBatch();
  }
}

function flushAudioBatch() {
  if (!pendingAudioSampleCount) return;
  if (!ws || ws.readyState !== WebSocket.OPEN) {
    resetAudioBatch();
    return;
  }

  if (ws.bufferedAmount > MAX_WS_BUFFERED_BYTES) {
    resetAudioBatch();
    const now = Date.now();
    if (now - lastBackpressureLogAt > 1000) {
      lastBackpressureLogAt = now;
      log("audio dropped while websocket is backed up", "warn");
    }
    return;
  }

  const batch = new Float32Array(pendingAudioSampleCount);
  let offset = 0;
  for (const samples of pendingAudioSamples) {
    batch.set(samples, offset);
    offset += samples.length;
  }
  resetAudioBatch();
  frameCount += 1;
  els.frames.textContent = String(frameCount);
  sendJson({
    type: "input_audio_buffer.append",
    audio: pcm16Base64(batch),
  });
}

function setText(el, text) {
  el.textContent = text;
  el.classList.toggle("empty", !text);
}

function normalizeTranscript(text) {
  return (text || "").replace(/\s+/g, " ").trim();
}

function createTranscriptLine(text, index, active) {
  const row = document.createElement("div");
  row.className = `transcript-line${active ? " active" : ""}`;

  const number = document.createElement("span");
  number.className = "line-index";
  number.textContent = String(index + 1).padStart(2, "0");

  const content = document.createElement("span");
  content.className = "line-text";
  content.textContent = text;

  row.append(number, content);
  return row;
}

function splitTranscript(text) {
  const normalized = normalizeTranscript(text);
  if (!normalized) return [];

  const phrases = transcriptPhrases(normalized);
  const lines = [];
  for (const phrase of phrases) {
    let rest = phrase.trim();
    while (rest.length > MAX_TRANSCRIPT_LINE) {
      const slice = rest.slice(0, MAX_TRANSCRIPT_LINE + 1);
      const spaceBreak = slice.lastIndexOf(" ");
      const cut = spaceBreak > 6 ? spaceBreak : MAX_TRANSCRIPT_LINE;
      lines.push(rest.slice(0, cut).trim());
      rest = rest.slice(cut).trim();
    }
    if (rest) lines.push(rest);
  }
  return lines;
}

function transcriptPhrases(text) {
  const normalized = normalizeTranscript(text);
  if (!normalized) return [];
  return (normalized.match(/[^。！？!?；;\n]+[。！？!?；;]?/g) || [
    normalized,
  ]).map((phrase) => phrase.trim()).filter(Boolean);
}

function joinTranscriptParts(parts) {
  return parts.reduce((joined, part) => {
    if (!joined) return part;
    if (/[\w.!?;]$/.test(joined) && /^[\w]/.test(part)) {
      return `${joined} ${part}`;
    }
    return `${joined}${part}`;
  }, "");
}

function partitionTranscript(text) {
  const phrases = transcriptPhrases(text);
  if (phrases.length <= 1) {
    return {
      committed: "",
      active: normalizeTranscript(text),
    };
  }
  return {
    committed: joinTranscriptParts(phrases.slice(0, -1)),
    active: phrases[phrases.length - 1],
  };
}

function renderTranscript(text) {
  const lines = splitTranscript(text);
  els.partial.replaceChildren();
  els.partial.classList.toggle("empty", lines.length === 0);
  for (const [index, line] of lines.entries()) {
    els.partial.append(
      createTranscriptLine(line, index, index === lines.length - 1),
    );
  }
  els.partial.scrollTop = els.partial.scrollHeight;
}

function renderLiveTranscript(text) {
  transcriptText = normalizeTranscript(text);
  const { committed, active } = partitionTranscript(text);
  committedTranscriptText = committed;
  renderTranscript(transcriptText);
  renderFinalTranscript(committedTranscriptText);
}

function renderFinalTranscript(text) {
  setText(els.final, text);
  els.final.scrollTop = els.final.scrollHeight;
}

function commitFinalTranscript(text) {
  transcriptText = normalizeTranscript(text);
  committedTranscriptText = transcriptText;
  renderTranscript(transcriptText);
  renderFinalTranscript(committedTranscriptText);
}

function resetTranscript() {
  transcriptText = "";
  committedTranscriptText = "";
  renderTranscript("");
  renderFinalTranscript("");
}

async function start() {
  await configReady;
  els.start.disabled = true;
  els.stop.disabled = false;
  frameCount = 0;
  els.frames.textContent = "0";
  resetTranscript();
  setAudio("requesting");

  try {
    if (runtimeConfig.authRequired && !currentApiKey()) {
      els.start.disabled = false;
      els.stop.disabled = true;
      setAudio("needs key", true);
      log("api key required", "auth");
      return;
    }
    persistApiKey();

    mediaStream = await navigator.mediaDevices.getUserMedia({
      audio: {
        echoCancellation: true,
        noiseSuppression: true,
        autoGainControl: true,
      },
    });
    audioContext = new AudioContext();
    els.rate.textContent = `${audioContext.sampleRate} Hz`;
    const moduleUrl = URL.createObjectURL(
      new Blob([workletSource], { type: "application/javascript" }),
    );
    await audioContext.audioWorklet.addModule(moduleUrl);
    URL.revokeObjectURL(moduleUrl);

    ws = new WebSocket(realtimeUrl(), realtimeProtocols());
    setSocket("connecting");

    ws.addEventListener("open", () => {
      setSocket("open", true);
      setAudio("streaming");
      isRecording = true;
      sendJson({
        type: "session.update",
        session: {
          type: "transcription",
          audio: {
            input: {
              format: {
                type: "audio/pcm",
                rate: audioContext.sampleRate,
              },
            },
          },
        },
      });

      source = audioContext.createMediaStreamSource(mediaStream);
      worklet = new AudioWorkletNode(audioContext, "capture-processor");
      mutedOutput = audioContext.createGain();
      mutedOutput.gain.value = 0;
      worklet.port.onmessage = (event) => {
        if (!isRecording || !ws || ws.readyState !== WebSocket.OPEN) return;
        enqueueAudioSamples(event.data);
      };
      source.connect(worklet);
      worklet.connect(mutedOutput);
      mutedOutput.connect(audioContext.destination);
    });

    ws.addEventListener("message", (message) => {
      let event;
      try {
        event = JSON.parse(message.data);
      } catch (error) {
        log("invalid server event", "error");
        return;
      }
      if (event.type === "conversation.item.input_audio_transcription.delta") {
        const nextTranscript =
          event.transcript || transcriptText + (event.delta || "");
        renderLiveTranscript(nextTranscript);
      } else if (
        event.type === "conversation.item.input_audio_transcription.completed"
      ) {
        commitFinalTranscript(event.transcript || transcriptText);
        if (!isRecording) setSocket("completed", true);
      } else if (event.type === "error") {
        log(event.error?.message || "error", "error");
      } else {
        log(event.type || message.data);
      }
    });

    ws.addEventListener("close", (event) => {
      const reason = formatCloseReason(event);
      setSocket(reason);
      log(reason, "socket");
      cleanupAudio();
    });

    ws.addEventListener("error", (event) => {
      setSocket("error");
      log(formatSocketError(event), "error");
    });
  } catch (error) {
    els.start.disabled = false;
    els.stop.disabled = true;
    setAudio("failed", true);
    log(error.message || String(error), "error");
    cleanupAudio();
  }
}

function cleanupAudio() {
  isRecording = false;
  resetAudioBatch();
  if (worklet) worklet.disconnect();
  if (source) source.disconnect();
  if (mutedOutput) mutedOutput.disconnect();
  if (mediaStream) {
    for (const track of mediaStream.getTracks()) track.stop();
  }
  if (audioContext && audioContext.state !== "closed") {
    audioContext.close();
  }
  worklet = null;
  source = null;
  mutedOutput = null;
  mediaStream = null;
  audioContext = null;
  els.start.disabled = false;
  els.stop.disabled = true;
  setAudio("idle");
  setSignal(0);
}

function stop() {
  isRecording = false;
  flushAudioBatch();
  cleanupAudio();
  setAudio("committed");
  sendJson({ type: "input_audio_buffer.commit" });
}

els.start.addEventListener("click", start);
els.stop.addEventListener("click", stop);
els.clear.addEventListener("click", () => {
  resetTranscript();
  els.events.textContent = "";
});

setSocket("idle");
setAudio("idle");
setSignal(0);
configReady = loadRuntimeConfig().finally(() => {
  log(displayRealtimeUrl());
});
