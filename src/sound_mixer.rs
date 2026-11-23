//! All voices use original edited takes. Sample decoding happens once on the device worker.
use super::{Cue, Motor, Trigger, BANK, COUNT, RATE, ROTOR};
use std::f32::consts::{FRAC_PI_4, TAU};

type Take = Box<[f32]>;
struct Samples {
    cues: [Vec<Take>; COUNT],
    diesel: Take,
    wind: Take,
    rotor: Take,
}
impl Samples {
    fn load() -> Self {
        let raw = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
            include_bytes!("../assets/field-audio.fsbk"),
            24 * 1024 * 1024,
        )
        .expect("bundled field audio");
        let mut data = raw.as_slice();
        fn read<'a>(data: &mut &'a [u8], count: usize) -> &'a [u8] {
            let (head, tail) = data.split_at(count);
            *data = tail;
            head
        }
        assert_eq!(read(&mut data, 5), b"FSBK\x01");
        assert_eq!(
            u32::from_le_bytes(read(&mut data, 4).try_into().unwrap()) as usize,
            RATE
        );
        let groups = u16::from_le_bytes(read(&mut data, 2).try_into().unwrap());
        let mut bank = Self {
            cues: Default::default(),
            diesel: Box::default(),
            wind: Box::default(),
            rotor: Box::default(),
        };
        for _ in 0..groups {
            let length = read(&mut data, 1)[0] as usize;
            let name = std::str::from_utf8(read(&mut data, length)).unwrap();
            let count = read(&mut data, 1)[0];
            let mut takes = Vec::new();
            for _ in 0..count {
                let len = u32::from_le_bytes(read(&mut data, 4).try_into().unwrap()) as usize;
                takes.push(
                    read(&mut data, 2 * len)
                        .chunks_exact(2)
                        .map(|s| i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.)
                        .collect::<Take>(),
                );
            }
            match name {
                "Diesel" => bank.diesel = takes.pop().unwrap(),
                "Rotor" => bank.rotor = takes.pop().unwrap(),
                "Wind" => bank.wind = takes.pop().unwrap(),
                _ => {
                    let cue = BANK
                        .iter()
                        .find(|c| format!("{c:?}") == name)
                        .expect("known sound cue");
                    bank.cues[*cue as usize] = takes;
                }
            }
        }
        assert!(
            data.is_empty()
                && !bank.wind.is_empty()
                && !bank.diesel.is_empty()
                && !bank.rotor.is_empty()
        );
        assert!(bank.cues.iter().all(|takes| !takes.is_empty()));
        bank
    }
}

impl Cue {
    fn level(self) -> f32 {
        match self {
            Cue::Cannon | Cue::ShellBlast => 1.0,
            Cue::Blast | Cue::Apc => 0.90,
            Cue::Shot | Cue::Patrol => 0.77,
            Cue::Mounted => 0.70,
            Cue::Smg | Cue::Distant => 0.63,
            Cue::TankLoader | Cue::Crash => 0.57,
            Cue::Dry | Cue::BootGround | Cue::BootHard | Cue::BootRubble => 0.34,
            Cue::Harvest | Cue::Throw => 0.39,
            _ => 0.51,
        }
    }
}
struct Voice {
    effect: Trigger,
    index: usize,
    take: usize,
    left: f32,
    right: f32,
    filtered: f32,
    air: f32,
}
struct MotorVoice {
    target: Motor,
    speed: f32,
    rpm: f32,
    phase: f32,
    contact: f32,
    tread_sample: usize,
    tread_index: usize,
    level: f32,
    pan: f32,
    body: f32,
}
impl MotorVoice {
    fn new(m: Motor) -> Self {
        Self {
            target: m,
            speed: m.speed,
            rpm: 1.,
            phase: (m.id % 8) as f32 * 1473.,
            contact: 0.,
            tread_sample: 0,
            tread_index: usize::MAX,
            level: 0.,
            pan: m.pan,
            body: 0.,
        }
    }
    fn next_sample(&mut self, clip: &Take) -> f32 {
        let i = self.phase as usize % clip.len();
        let a = clip[i];
        let b = clip[(i + 1) % clip.len()];
        let result = a + (b - a) * self.phase.fract();
        self.phase = (self.phase + self.rpm) % clip.len() as f32;
        result
    }
    fn sample(&mut self, bank: &Samples) -> (f32, f32) {
        self.level += (self.target.level - self.level) * 0.001;
        self.pan += (self.target.pan - self.pan) * 0.0012;
        self.speed += (self.target.speed - self.speed) * 0.002;
        let angle = (self.pan + 1.) * FRAC_PI_4;
        if self.target.kind == ROTOR {
            self.rpm += (0.91 + self.speed * 0.28 - self.rpm) * 0.00044;
            let blades = self.next_sample(&bank.rotor);
            self.body += (blades - self.body) * (0.12 + 0.52 * self.level);
            let signal = self.body * self.level * (0.87 + self.speed * 0.13);
            return (signal * angle.cos(), signal * angle.sin());
        }
        let gear = (self.speed.min(0.999) * 3.).fract();
        let load = (self.target.speed - self.speed).clamp(-0.18, 0.28);
        let gearing = match self.target.kind {
            0 => 1.65,
            1 => 1.21,
            2 => 0.91,
            _ => 0.72,
        };
        self.rpm +=
            (gearing * (0.84 + 0.37 * self.speed + 0.33 * gear + 0.4 * load) - self.rpm) * 0.00022;
        let engine = self.next_sample(&bank.diesel);
        self.body += (engine - self.body) * if self.target.kind == 3 { 0.12 } else { 0.28 };
        let mut sound = (0.64 * self.body + 0.16 * engine) * (0.76 + 0.24 * self.speed);
        self.contact += self.speed * (if self.target.kind == 3 { 21. } else { 4. }) / RATE as f32;
        if self.contact >= 1. {
            self.contact -= 1.;
            self.tread_sample = (self.tread_sample + 1) % 3;
            self.tread_index = 0;
        }
        let cue = if self.target.kind == 3 {
            Cue::Metal
        } else {
            Cue::BootGround
        };
        let sample = &bank.cues[cue as usize][self.tread_sample];
        if let Some(&x) = sample.get(self.tread_index) {
            // A link or tyre contacts the ground. End this little take before the next contact.
            let age = self.tread_index as f32 / RATE as f32;
            sound += x
                * (1. - age * 14.).max(0.)
                * self.speed
                * if self.target.kind == 3 { 0.45 } else { 0.21 };
            self.tread_index += 1;
        }
        (
            sound * self.level * angle.cos(),
            sound * self.level * angle.sin(),
        )
    }
}

pub(super) struct Mixer {
    bank: Samples,
    variants: [usize; COUNT],
    voices: Vec<Voice>,
    motors: Vec<MotorVoice>,
    wind_target: f32,
    wind: f32,
    wind_at: usize,
    headroom: f32,
}
impl Mixer {
    pub(super) fn new() -> Self {
        Self {
            bank: Samples::load(),
            variants: [0; COUNT],
            voices: Vec::with_capacity(24),
            motors: Vec::with_capacity(4),
            wind_target: 0.,
            wind: 0.,
            wind_at: 0,
            headroom: 1.,
        }
    }
    pub(super) fn clear(&mut self) {
        self.voices.clear();
        self.motors.clear();
        self.wind_target = 0.;
        self.wind = 0.;
        self.headroom = 1.;
    }
    pub(super) fn outdoors(&mut self, volume: f32) {
        self.wind_target = volume;
    }
    pub(super) fn engines(&mut self, motors: &[Motor]) {
        for voice in &mut self.motors {
            voice.target.level = 0.;
        }
        for &m in motors {
            if let Some(v) = self
                .motors
                .iter_mut()
                .find(|v| v.target.id == m.id && (v.target.kind == ROTOR) == (m.kind == ROTOR))
            {
                v.target = m;
            } else if self.motors.len() < 14 {
                self.motors.push(MotorVoice::new(m));
            }
        }
    }
    pub(super) fn add(&mut self, effect: Trigger) {
        if self.voices.len() >= 24 {
            let rank = |e: &Trigger| e.priority as f32 + e.gain * e.cue.level();
            let (i, v) = self
                .voices
                .iter()
                .enumerate()
                .min_by(|a, b| rank(&a.1.effect).total_cmp(&rank(&b.1.effect)))
                .unwrap();
            if rank(&v.effect) > rank(&effect) {
                return;
            }
            self.voices.swap_remove(i);
        }
        let key = effect.cue as usize;
        let take = self.variants[key];
        self.variants[key] = (take + 1) % self.bank.cues[key].len();
        let angle = (effect.pan.clamp(-1., 1.) + 1.) * FRAC_PI_4;
        let level = effect.gain * effect.cue.level();
        let cutoff = 14500. / (1. + 0.045 * effect.distance);
        self.voices.push(Voice {
            effect,
            index: (effect.offset * RATE as f32) as usize,
            take,
            left: angle.cos() * level,
            right: angle.sin() * level,
            filtered: 0.,
            air: 1. - (-TAU * cutoff / RATE as f32).exp(),
        });
    }
    pub(super) fn render(&mut self, output: &mut [i16]) {
        let n = self.bank.wind.len();
        for frame in output.chunks_exact_mut(2) {
            self.wind += (self.wind_target - self.wind) * 0.00015;
            let mut left = self.wind * self.bank.wind[self.wind_at];
            let mut right = self.wind * self.bank.wind[(self.wind_at + 2237) % n];
            self.wind_at = (self.wind_at + 1) % n;
            for m in &mut self.motors {
                let (l, r) = m.sample(&self.bank);
                left += l;
                right += r;
            }
            for v in &mut self.voices {
                if let Some(&x) = self.bank.cues[v.effect.cue as usize][v.take].get(v.index) {
                    v.filtered += v.air * (x - v.filtered);
                    left += v.filtered * v.left;
                    right += v.filtered * v.right;
                    v.index += 1;
                }
            }
            // Stereo-linked peak limiter. Retain the image and release over about 80 ms.
            let target = 0.88 / left.abs().max(right.abs()).max(0.88);
            self.headroom = target.min(self.headroom + (1. - self.headroom) * 0.00039);
            frame[0] = (left * self.headroom * 32767.).round() as i16;
            frame[1] = (right * self.headroom * 32767.).round() as i16;
        }
        self.voices
            .retain(|v| v.index < self.bank.cues[v.effect.cue as usize][v.take].len());
        self.motors
            .retain(|m| m.target.level > 0. || m.level > 0.0002);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn trigger(cue: Cue, pan: f32) -> Trigger {
        Trigger {
            cue,
            pan,
            gain: 1.,
            priority: 3,
            distance: 0.,
            offset: 0.,
        }
    }
    #[test]
    fn field_takes_are_distinct_and_all_events_reach_mix() {
        let mut mixer = Mixer::new();
        for cue in BANK {
            let takes = &mixer.bank.cues[cue as usize];
            assert_eq!(takes.len(), 3);
            assert_ne!(takes[0], takes[1], "{cue:?}");
            for x in takes {
                assert!(x.len() >= RATE / 22 && x.len() <= 4 * RATE);
                assert!(x.last().unwrap().abs() < 0.004);
            }
            mixer.add(trigger(cue, 1.));
            let mut block = vec![0; RATE * 8];
            mixer.render(&mut block);
            assert!(block.chunks_exact(2).all(|v| v[0].abs() <= 1));
            assert!(block.chunks_exact(2).any(|v| v[1].abs() > 1000), "{cue:?}");
            assert!(mixer.voices.is_empty());
        }
    }
    #[test]
    fn engines_and_wind_change_with_the_scene_and_clear_cleanly() {
        let mut m = Mixer::new();
        let car = Motor {
            id: 4,
            kind: 0,
            speed: 0.12,
            pan: 1.,
            level: 0.8,
        };
        m.engines(&[car]);
        let mut x = vec![0; RATE];
        m.render(&mut x);
        assert!(x.chunks_exact(2).all(|f| f[0].abs() <= 1));
        assert!(x.chunks_exact(2).any(|f| f[1].abs() > 800));
        let idle = x.clone();
        m.engines(&[Motor {
            kind: 3,
            speed: 0.9,
            ..car
        }]);
        m.render(&mut x);
        assert_ne!(idle, x);
        m.engines(&[]);
        m.render(&mut x);
        assert!(m.motors.is_empty());
        m.outdoors(0.1);
        m.render(&mut x);
        assert!(x.iter().any(|v| v.abs() > 100));
        m.clear();
        m.render(&mut x);
        assert!(x.iter().all(|v| *v == 0));
    }
    #[test]
    fn independent_vehicle_and_rotor_with_equal_ids_mix_and_fade() {
        let mut m = Mixer::new();
        let tank = Motor {
            id: 1,
            kind: 3,
            speed: 0.25,
            pan: -1.,
            level: 0.4,
        };
        let drone = Motor {
            id: 1,
            kind: ROTOR,
            speed: 0.,
            pan: 1.,
            level: 0.5,
        };
        m.engines(&[drone, tank]);
        assert_eq!(m.motors.len(), 2);
        let mut x = vec![0; RATE];
        m.render(&mut x);
        assert!(x
            .chunks_exact(2)
            .any(|f| f[0].abs() > 200 && f[1].abs() > 200));
        m.engines(&[drone]);
        m.render(&mut x);
        assert_eq!(m.motors.len(), 1);
        assert!(x[3 * RATE / 4..].chunks_exact(2).all(|f| f[0].abs() < 2));
        m.engines(&[]);
        m.render(&mut x);
        assert!(m.motors.is_empty());
    }
    #[test]
    fn busy_mix_preserves_headroom_and_loader_runs_to_chamber_close() {
        let mut m = Mixer::new();
        for _ in 0..12 {
            for cue in [Cue::Mounted, Cue::Blast, Cue::Cannon] {
                m.add(trigger(cue, 0.));
            }
        }
        assert!(m.voices.len() <= 24);
        let mut block = vec![0; RATE * 4];
        m.render(&mut block);
        assert!(block.iter().all(|s| i32::from(*s).abs() <= 28836));
        let mut loader = trigger(Cue::TankLoader, 0.);
        loader.offset = 0.18;
        m.add(loader);
        let mut body = vec![0; (RATE as f32 * 3.45) as usize * 2];
        m.render(&mut body);
        assert!(body[..RATE / 6].iter().all(|s| *s == 0));
        assert!(body[2 * RATE * 3..].iter().any(|s| s.abs() > 1200));
    }
}

#[cfg(test)]
#[test]
#[ignore = "writes stereo WAV to BATTLE_AUDIO_PREVIEW"]
fn render_audio_preview() {
    use std::io::Write;
    let path = std::env::var("BATTLE_AUDIO_PREVIEW").expect("output WAV path");
    let mut m = Mixer::new();
    m.outdoors(0.045);
    let schedule = [
        (0.12, Cue::BootGround, 0.),
        (0.51, Cue::BootGround, 0.),
        (0.92, Cue::BootHard, 0.),
        (1.31, Cue::Shot, -0.23),
        (1.85, Cue::Smg, 0.1),
        (1.95, Cue::Smg, 0.1),
        (2.05, Cue::Smg, 0.1),
        (2.15, Cue::Smg, 0.1),
        (2.33, Cue::Reload, 0.),
        (3.65, Cue::Cannon, 0.),
        (3.65, Cue::TankLoader, 0.),
        (4.56, Cue::Mounted, -0.43),
        (4.67, Cue::Mounted, -0.43),
        (4.78, Cue::Mounted, -0.43),
        (4.89, Cue::Metal, 0.65),
        (5.23, Cue::ShellBlast, 0.72),
    ];
    let mut block = [0_i16; 640];
    let mut pcm = Vec::new();
    for tick in 0..810 {
        if tick == 300 {
            m.engines(&[Motor {
                id: 2,
                kind: 3,
                speed: 0.41,
                pan: -0.34,
                level: 0.38,
            }]);
        }
        if tick == 575 {
            m.engines(&[Motor {
                id: 7,
                kind: ROTOR,
                speed: 0.1,
                pan: 0.52,
                level: 0.26,
            }]);
        }
        if tick == 740 {
            m.engines(&[]);
        }
        for &(at, cue, pan) in &schedule {
            if (at * 100.0_f32).round() as usize == tick {
                m.add(Trigger {
                    cue,
                    pan,
                    gain: 0.86,
                    priority: 3,
                    distance: 0.,
                    offset: 0.,
                });
            }
        }
        m.render(&mut block);
        for sample in block {
            pcm.extend(sample.to_le_bytes());
        }
    }
    let mut f = std::fs::File::create(path).unwrap();
    f.write_all(b"RIFF").unwrap();
    f.write_all(&(36 + pcm.len() as u32).to_le_bytes()).unwrap();
    f.write_all(b"WAVEfmt \x10\0\0\0\x01\0\x02\0").unwrap();
    f.write_all(&(RATE as u32).to_le_bytes()).unwrap();
    f.write_all(&(RATE as u32 * 4).to_le_bytes()).unwrap();
    f.write_all(b"\x04\0\x10\0data").unwrap();
    f.write_all(&(pcm.len() as u32).to_le_bytes()).unwrap();
    f.write_all(&pcm).unwrap();
}
