//! Procedural audio: built-in synthesised sound effects, custom tones and a tiny
//! note-sequencer for music. No audio files needed.

use std::sync::{mpsc, Arc, Mutex};

pub const RATE: u32 = 44100;

/// A request from the script, queued in the world and played by the audio thread.
#[derive(Clone, Debug)]
pub enum SoundCmd {
    Effect(String),
    Tone { freq: f32, secs: f32, wave: String, volume: f32 },
    Music { bpm: f32, notes: String, wave: String, volume: f32 },
    StopMusic,
}

fn note_freq(name: &str) -> Option<f32> {
    // e.g. "C4", "F#3", "Bb5", "-" or "." = rest
    let name = name.trim();
    if name.is_empty() || name == "-" || name == "." {
        return Some(0.0);
    }
    let bytes: Vec<char> = name.chars().collect();
    let base = match bytes[0].to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let mut i = 1;
    let mut semis = base;
    if i < bytes.len() && (bytes[i] == '#' || bytes[i] == 'b') {
        semis += if bytes[i] == '#' { 1 } else { -1 };
        i += 1;
    }
    let octave: i32 = bytes[i..].iter().collect::<String>().parse().unwrap_or(4);
    let midi = (octave + 1) * 12 + semis;
    Some(440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0))
}

fn osc(wave: &str, phase: f32) -> f32 {
    let p = phase.fract();
    match wave {
        "square" => if p < 0.5 { 1.0 } else { -1.0 },
        "saw" => 2.0 * p - 1.0,
        "triangle" => 4.0 * (p - 0.5).abs() - 1.0,
        "noise" => rand::random::<f32>() * 2.0 - 1.0,
        _ => (p * std::f32::consts::TAU).sin(),
    }
}

/// Renders a tone with a pitch slide and an attack/decay envelope.
fn render(freq0: f32, freq1: f32, secs: f32, wave: &str, volume: f32, attack: f32, decay_pow: f32) -> Vec<f32> {
    let n = (secs * RATE as f32) as usize;
    let mut out = Vec::with_capacity(n);
    let mut phase = 0.0f32;
    for i in 0..n {
        let t = i as f32 / n.max(1) as f32;
        let f = freq0 + (freq1 - freq0) * t;
        phase += f / RATE as f32;
        let env = if t < attack { t / attack } else { (1.0 - (t - attack) / (1.0 - attack)).max(0.0).powf(decay_pow) };
        out.push(osc(wave, phase) * env * volume);
    }
    out
}

pub fn effect(name: &str) -> Vec<f32> {
    match name {
        "shoot" | "laser" => render(900.0, 300.0, 0.15, "square", 0.25, 0.01, 1.5),
        "hit" => render(200.0, 80.0, 0.12, "noise", 0.35, 0.005, 2.0),
        "explosion" | "explode" => render(120.0, 30.0, 0.5, "noise", 0.5, 0.01, 1.2),
        "jump" => render(300.0, 700.0, 0.18, "square", 0.25, 0.01, 1.0),
        "coin" | "pickup" => {
            let mut a = render(1200.0, 1200.0, 0.07, "square", 0.25, 0.005, 1.0);
            a.extend(render(1800.0, 1800.0, 0.18, "square", 0.25, 0.005, 1.5));
            a
        }
        "powerup" => render(400.0, 1600.0, 0.35, "triangle", 0.3, 0.02, 1.0),
        "hurt" | "damage" => render(250.0, 120.0, 0.25, "saw", 0.35, 0.01, 1.0),
        "click" | "select" => render(1000.0, 1000.0, 0.04, "square", 0.2, 0.005, 1.0),
        "win" => {
            let mut a = Vec::new();
            for f in [523.0, 659.0, 784.0, 1046.0] {
                a.extend(render(f, f, 0.15, "square", 0.25, 0.01, 0.8));
            }
            a
        }
        "lose" | "gameover" => {
            let mut a = Vec::new();
            for f in [392.0, 349.0, 311.0, 261.0] {
                a.extend(render(f, f, 0.22, "saw", 0.25, 0.01, 0.8));
            }
            a
        }
        "bounce" => render(500.0, 250.0, 0.08, "sine", 0.3, 0.005, 1.0),
        "step" => render(150.0, 100.0, 0.05, "noise", 0.15, 0.005, 2.0),
        "splash" => {
            let mut a = render(900.0, 200.0, 0.12, "noise", 0.4, 0.005, 1.5);
            a.extend(render(300.0, 120.0, 0.3, "noise", 0.25, 0.01, 2.0));
            a
        }
        _ => render(440.0, 440.0, 0.1, "sine", 0.2, 0.01, 1.0),
    }
}

/// Renders a whole loop of music: notes separated by spaces, each note lasts one beat;
/// append `/2` for half beat, `*2` for two beats (e.g. "C4 E4 G4*2 - C5/2 B4/2").
pub fn music(bpm: f32, notes: &str, wave: &str, volume: f32) -> Vec<f32> {
    let beat = 60.0 / bpm.max(20.0);
    let mut out = Vec::new();
    for tok in notes.split_whitespace() {
        let (note, len) = if let Some((n, d)) = tok.split_once('/') {
            (n, 1.0 / d.parse::<f32>().unwrap_or(1.0))
        } else if let Some((n, m)) = tok.split_once('*') {
            (n, m.parse::<f32>().unwrap_or(1.0))
        } else {
            (tok, 1.0)
        };
        let f = note_freq(note).unwrap_or(0.0);
        let secs = beat * len;
        if f <= 0.0 {
            out.extend(std::iter::repeat(0.0).take((secs * RATE as f32) as usize));
        } else {
            out.extend(render(f, f, secs, wave, volume, 0.01, 0.6));
        }
    }
    out
}

/// Audio output running on its own thread. Cheap to construct; silently disabled when no
/// audio device exists (headless sim, CI).
pub struct Audio {
    tx: mpsc::Sender<SoundCmd>,
}

impl Audio {
    pub fn start() -> Audio {
        let (tx, rx) = mpsc::channel::<SoundCmd>();
        std::thread::spawn(move || {
            let Ok((_stream, handle)) = rodio::OutputStream::try_default() else {
                log::warn!("no audio output device; sound disabled");
                for _ in rx {}
                return;
            };
            let music_sink: Arc<Mutex<Option<rodio::Sink>>> = Arc::new(Mutex::new(None));
            for cmd in rx {
                match cmd {
                    SoundCmd::Effect(name) => {
                        let buf = rodio::buffer::SamplesBuffer::new(1, RATE, effect(&name));
                        let _ = handle.play_raw(rodio::Source::convert_samples(buf));
                    }
                    SoundCmd::Tone { freq, secs, wave, volume } => {
                        let buf = rodio::buffer::SamplesBuffer::new(1, RATE, render(freq, freq, secs.clamp(0.01, 5.0), &wave, volume.clamp(0.0, 1.0), 0.01, 1.0));
                        let _ = handle.play_raw(rodio::Source::convert_samples(buf));
                    }
                    SoundCmd::Music { bpm, notes, wave, volume } => {
                        let samples = music(bpm, &notes, &wave, volume.clamp(0.0, 1.0) * 0.5);
                        let mut guard = music_sink.lock().unwrap();
                        if let Some(old) = guard.take() {
                            old.stop();
                        }
                        if let Ok(sink) = rodio::Sink::try_new(&handle) {
                            let buf = rodio::buffer::SamplesBuffer::new(1, RATE, samples);
                            sink.append(rodio::Source::repeat_infinite(buf));
                            *guard = Some(sink);
                        }
                    }
                    SoundCmd::StopMusic => {
                        if let Some(old) = music_sink.lock().unwrap().take() {
                            old.stop();
                        }
                    }
                }
            }
        });
        Audio { tx }
    }

    pub fn play(&self, cmd: SoundCmd) {
        let _ = self.tx.send(cmd);
    }
}
