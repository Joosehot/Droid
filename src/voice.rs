//! Microphone loudness for game code: [`crate::api::Ctx::voice`].
//!
//! The default input device is captured on a background thread (cpal). Each audio block's RMS
//! level becomes decibels, the noise floor is measured in the first moment, and the level above
//! the floor is mapped to 0..1 so normal speech lands near 0.3 and a shout at 0.8 or more. It is
//! smoothed with a fast attack and a slower release and shared through an atomic. Every number
//! is in [`TUNING`]. Blocks of digital silence (a device warming up) don't count towards the
//! floor, and afterwards the floor follows the room: down quickly, up slowly, so a louder crowd
//! doesn't read as shouting but a long shout still does.
//!
//! Without a microphone (or on any audio error) the level stays 0, one warning is printed and the
//! game runs on. Capture starts on the first `voice()` call, so games that never ask never open
//! the microphone.
//!
//! Headless and testing: `DROID_VOICE` replaces the microphone with a schedule, either a
//! constant (`0.7`) or `seconds:level` steps (`0:0,3:0.9,6:0,8:1`: 0.9 from 3 s, 0 from 6 s...),
//! read against the game clock. Under `DROID_SIM` without `DROID_VOICE` the level is 0.
//! `DROID_VOICE_GAIN` (decibels, may be negative) shifts a live microphone's level for a quiet
//! or loud room; `DROID_VOICE_DEBUG=1` prints the level, the block's dBFS and the floor once a
//! second (to tune the gain on site).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

/// Every constant of the loudness meter.
#[derive(Clone, Copy, Debug)]
pub struct VoiceTuning {
    /// Seconds of input measured as the noise floor before any level is reported.
    pub calibrate_secs: f32,
    /// Blocks quieter than this (dBFS) are digital silence, not a room: skipped while calibrating.
    pub dead_db: f32,
    /// The floor is kept inside this range (dBFS), so a noisy start or a dead-silent device
    /// can't push the scale off.
    pub floor_min_db: f32,
    pub floor_max_db: f32,
    /// After calibration the floor follows quieter blocks with this time constant (seconds) and
    /// creeps up towards louder ones at most this fast (dB per second).
    pub floor_fall_secs: f32,
    pub floor_rise_db_per_sec: f32,
    /// Level 0 starts this far above the floor (dB): room noise and breathing read as silence.
    pub gate_db: f32,
    /// From the gate up, this many dB more is level 1.
    pub span_db: f32,
    /// Smoothing time constants (seconds): rising fast, falling slower.
    pub attack_secs: f32,
    pub release_secs: f32,
}

pub const TUNING: VoiceTuning = VoiceTuning {
    calibrate_secs: 1.0,
    dead_db: -100.0,
    floor_min_db: -80.0,
    floor_max_db: -30.0,
    floor_fall_secs: 0.5,
    floor_rise_db_per_sec: 0.5,
    gate_db: 14.0,
    span_db: 42.0,
    attack_secs: 0.03,
    release_secs: 0.35,
};

/// The quietest block level considered (dBFS); digital silence would be -inf.
const SILENCE_DB: f32 = -120.0;

/// Loudness 0..1 at game time `t` (seconds).
pub fn level(t: f32) -> f32 {
    if let Some(s) = schedule() {
        return s.at(t);
    }
    if std::env::var_os("DROID_SIM").is_some() {
        return 0.0;
    }
    match mic() {
        Some(l) => f32::from_bits(l.load(Ordering::Relaxed)),
        None => 0.0,
    }
}

// ---- schedule (DROID_VOICE) ------------------------------------------------------

/// A step function of time: `(from_seconds, level)`, sorted.
#[derive(Clone, Debug, PartialEq)]
pub struct Schedule(pub Vec<(f32, f32)>);

impl Schedule {
    /// `"0.7"` or `"0:0,3:0.9,6:0"`.
    pub fn parse(s: &str) -> Result<Schedule, String> {
        let s = s.trim();
        if s.is_empty() {
            return Ok(Schedule(vec![(0.0, 0.0)]));
        }
        if !s.contains(':') {
            let v: f32 = s.parse().map_err(|_| format!("DROID_VOICE: \"{s}\" is not a level"))?;
            return Ok(Schedule(vec![(0.0, v.clamp(0.0, 1.0))]));
        }
        let mut steps = Vec::new();
        for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (t, v) = part.split_once(':').ok_or_else(|| format!("DROID_VOICE: \"{part}\" is not seconds:level"))?;
            let t: f32 = t.trim().parse().map_err(|_| format!("DROID_VOICE: \"{t}\" is not seconds"))?;
            let v: f32 = v.trim().parse().map_err(|_| format!("DROID_VOICE: \"{v}\" is not a level"))?;
            steps.push((t, v.clamp(0.0, 1.0)));
        }
        steps.sort_by(|a, b| a.0.total_cmp(&b.0));
        Ok(Schedule(steps))
    }

    pub fn at(&self, t: f32) -> f32 {
        self.0.iter().take_while(|(from, _)| *from <= t).last().map(|(_, v)| *v).unwrap_or(0.0)
    }
}

fn schedule() -> Option<&'static Schedule> {
    static S: OnceLock<Option<Schedule>> = OnceLock::new();
    S.get_or_init(|| {
        let v = std::env::var("DROID_VOICE").ok()?;
        match Schedule::parse(&v) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("droid: {e}; voice is 0");
                Some(Schedule(vec![(0.0, 0.0)]))
            }
        }
    })
    .as_ref()
}

// ---- level meter -----------------------------------------------------------------

/// Turns audio blocks into the smoothed 0..1 level. Pure, so it is tested without a device.
pub struct Meter {
    pub tuning: VoiceTuning,
    /// Added to every block's dB (`DROID_VOICE_GAIN`).
    pub gain_db: f32,
    calib_secs: f32,
    calib_sum: f64,
    calib_n: u32,
    floor_db: Option<f32>,
    level: f32,
    last_db: f32,
}

impl Meter {
    pub fn new(tuning: VoiceTuning, gain_db: f32) -> Meter {
        Meter { tuning, gain_db, calib_secs: 0.0, calib_sum: 0.0, calib_n: 0, floor_db: None, level: 0.0, last_db: SILENCE_DB }
    }

    /// The noise floor once measured (dBFS).
    pub fn floor_db(&self) -> Option<f32> {
        self.floor_db
    }

    /// The last block's level (dBFS, gain included).
    pub fn last_db(&self) -> f32 {
        self.last_db
    }

    /// Feed one block of mono samples (-1..1) lasting `secs`; returns the smoothed level.
    pub fn feed(&mut self, samples: &[f32], secs: f32) -> f32 {
        if samples.is_empty() || secs <= 0.0 {
            return self.level;
        }
        let power = samples.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / samples.len() as f64;
        let db = if power > 0.0 { (10.0 * power.log10()) as f32 } else { SILENCE_DB };
        let t = self.tuning;
        let Some(floor) = self.floor_db else {
            if db < t.dead_db {
                return 0.0;
            }
            let db = db + self.gain_db;
            self.calib_sum += db as f64;
            self.calib_n += 1;
            self.calib_secs += secs;
            if self.calib_secs >= t.calibrate_secs {
                let mean = (self.calib_sum / self.calib_n.max(1) as f64) as f32;
                self.floor_db = Some(mean.clamp(t.floor_min_db, t.floor_max_db));
            }
            return 0.0;
        };
        let db = db.max(SILENCE_DB) + self.gain_db;
        let floor = if db < floor {
            floor + (db - floor) * (1.0 - (-secs / t.floor_fall_secs.max(1e-4)).exp())
        } else {
            floor + (db - floor).min(t.floor_rise_db_per_sec * secs)
        }
        .clamp(t.floor_min_db, t.floor_max_db);
        self.floor_db = Some(floor);
        self.last_db = db;
        let target = ((db - floor - t.gate_db) / t.span_db).clamp(0.0, 1.0);
        let tau = if target > self.level { t.attack_secs } else { t.release_secs };
        let k = 1.0 - (-secs / tau.max(1e-4)).exp();
        self.level += (target - self.level) * k;
        self.level
    }
}

// ---- microphone --------------------------------------------------------------------

fn mic() -> Option<&'static Arc<AtomicU32>> {
    static M: OnceLock<Option<Arc<AtomicU32>>> = OnceLock::new();
    M.get_or_init(|| {
        let shared = Arc::new(AtomicU32::new(0f32.to_bits()));
        let out = shared.clone();
        let gain = std::env::var("DROID_VOICE_GAIN").ok().and_then(|g| g.trim().parse::<f32>().ok()).unwrap_or(0.0);
        let spawned = std::thread::Builder::new().name("droid-voice".into()).spawn(move || {
            if let Err(e) = capture(out, gain) {
                eprintln!("droid: no microphone ({e}); voice() stays 0");
            }
        });
        if let Err(e) = spawned {
            eprintln!("droid: no microphone thread ({e}); voice() stays 0");
            return None;
        }
        Some(shared)
    })
    .as_ref()
}

/// Open the default input device and keep feeding the meter until the program ends.
fn capture(out: Arc<AtomicU32>, gain_db: f32) -> Result<(), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or("no default input device")?;
    let supported = device.default_input_config().map_err(|e| e.to_string())?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let channels = config.channels.max(1) as usize;
    let rate = config.sample_rate.0.max(1) as f32;
    let name = device.name().unwrap_or_else(|_| "default input".into());

    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, &config, channels, rate, out, gain_db),
        cpal::SampleFormat::I16 => build::<i16>(&device, &config, channels, rate, out, gain_db),
        cpal::SampleFormat::U16 => build::<u16>(&device, &config, channels, rate, out, gain_db),
        cpal::SampleFormat::I32 => build::<i32>(&device, &config, channels, rate, out, gain_db),
        cpal::SampleFormat::I8 => build::<i8>(&device, &config, channels, rate, out, gain_db),
        cpal::SampleFormat::U8 => build::<u8>(&device, &config, channels, rate, out, gain_db),
        other => return Err(format!("unsupported sample format {other:?}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    log::info!("voice: listening on \"{name}\" ({} Hz, {channels} ch)", config.sample_rate.0);
    // The stream lives as long as this thread; the thread lives as long as the program.
    loop {
        std::thread::park();
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    rate: f32,
    out: Arc<AtomicU32>,
    gain_db: f32,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    use cpal::traits::DeviceTrait;
    let mut meter = Meter::new(TUNING, gain_db);
    let mut mono: Vec<f32> = Vec::new();
    let mut told = false;
    let debug = std::env::var_os("DROID_VOICE_DEBUG").is_some();
    let mut since = 0.0f32;
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                mono.clear();
                mono.extend(data.chunks(channels).map(|frame| frame.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / frame.len() as f32));
                let secs = mono.len() as f32 / rate;
                let level = meter.feed(&mono, secs);
                since += secs;
                if debug && since >= 1.0 {
                    since = 0.0;
                    let floor = meter.floor_db().map_or("calibrating".to_string(), |f| format!("{f:.1} dBFS"));
                    eprintln!("voice: level {level:.2}  block {:.1} dBFS  floor {floor}", meter.last_db());
                }
                if !told {
                    if let Some(floor) = meter.floor_db() {
                        told = true;
                        log::info!("voice: noise floor {floor:.1} dBFS");
                    }
                }
                out.store(level.to_bits(), Ordering::Relaxed);
            },
            |e| eprintln!("droid: microphone error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_steps_and_constants() {
        let s = Schedule::parse("0:0,3:0.9,6:0,8:1").unwrap();
        assert_eq!(s.at(0.0), 0.0);
        assert_eq!(s.at(2.99), 0.0);
        assert_eq!(s.at(3.0), 0.9);
        assert_eq!(s.at(7.0), 0.0);
        assert_eq!(s.at(100.0), 1.0);
        assert_eq!(Schedule::parse("0.7").unwrap().at(5.0), 0.7);
        assert_eq!(Schedule::parse("2").unwrap().at(5.0), 1.0);
        assert!(Schedule::parse("x:1").is_err());
    }

    fn tone(amp: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| amp * (i as f32 * 0.07).sin()).collect()
    }

    #[test]
    fn meter_calibrates_then_maps_speech_and_shouts() {
        let rate = 48000.0;
        let block = 480; // 10 ms
        let secs = block as f32 / rate;
        let mut m = Meter::new(TUNING, 0.0);
        // a device warming up: digital silence first, which must not count as the room
        for _ in 0..50 {
            assert_eq!(m.feed(&tone(0.0, block), secs), 0.0);
        }
        // a quiet room: about -60 dBFS
        for _ in 0..120 {
            assert_eq!(m.feed(&tone(0.0014, block), secs), 0.0);
        }
        let floor = m.floor_db().expect("calibrated");
        assert!((-63.0..-57.0).contains(&floor), "{floor}");
        let settle = |m: &mut Meter, amp: f32, blocks: usize| {
            let mut l = 0.0;
            for _ in 0..blocks {
                l = m.feed(&tone(amp, block), secs);
            }
            l
        };
        let speech = settle(&mut m, 0.045, 50); // about -30 dBFS
        let shout = settle(&mut m, 0.4, 50); // about -11 dBFS
        let quiet = settle(&mut m, 0.0014, 300);
        assert!((0.2..0.5).contains(&speech), "speech {speech}");
        assert!(shout >= 0.75, "shout {shout}");
        assert!(quiet < 0.02, "quiet {quiet}");
    }

    #[test]
    fn the_floor_follows_a_louder_room_slowly() {
        let mut m = Meter::new(TUNING, 0.0);
        for _ in 0..120 {
            m.feed(&tone(0.0014, 480), 0.01); // -60 dBFS
        }
        // a long shout barely moves the floor
        for _ in 0..300 {
            m.feed(&tone(0.4, 480), 0.01);
        }
        assert!(m.floor_db().unwrap() < -57.0, "{:?}", m.floor_db());
        // a pause brings it back down quickly
        for _ in 0..200 {
            m.feed(&tone(0.0014, 480), 0.01);
        }
        assert!(m.floor_db().unwrap() < -59.0, "{:?}", m.floor_db());
    }

    #[test]
    fn attack_is_faster_than_release() {
        let mut m = Meter::new(TUNING, 0.0);
        for _ in 0..120 {
            m.feed(&tone(0.0014, 480), 0.01);
        }
        let up = m.feed(&tone(0.5, 4800), 0.1);
        let mut m2 = Meter::new(TUNING, 0.0);
        for _ in 0..120 {
            m2.feed(&tone(0.0014, 480), 0.01);
        }
        for _ in 0..100 {
            m2.feed(&tone(0.5, 480), 0.01);
        }
        let top = m2.feed(&tone(0.5, 480), 0.01);
        let down = top - m2.feed(&tone(0.0, 4800), 0.1);
        assert!(up > down, "attack {up} release {down}");
    }
}
