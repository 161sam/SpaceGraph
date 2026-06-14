//! UI sound effects — behind the `audio` cargo feature.
//!
//! One-shot cues for the gameplay loop: a sweep on the scan pulse (`G`), a
//! klaxon when a new alert appears, a chime when an incident is resolved, and a
//! soft blip on node selection. All are gated by `cfg.audio_enabled` and scaled
//! by `cfg.audio_volume`. Assets live in `assets/audio/*.wav` (see
//! `gen_sounds.py`). Audio playback is graceful when no output device exists.

use bevy::audio::{AudioBundle, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;

use crate::app::events::Picked;
use crate::graph::GraphState;
use crate::render::gameplay::{Mission, ScanPulse};

/// Preloaded one-shot sound handles.
#[derive(Resource)]
pub struct AudioAssets {
    blip: Handle<AudioSource>,
    scan: Handle<AudioSource>,
    alert: Handle<AudioSource>,
    mission: Handle<AudioSource>,
}

/// Embed the WAVs in the binary (via `include_bytes!`) and register them as
/// audio assets. Embedding keeps playback working regardless of the working
/// directory or where the executable is installed — no `assets/` deploy step.
pub fn setup_audio(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    fn embed(sources: &mut Assets<AudioSource>, bytes: &'static [u8]) -> Handle<AudioSource> {
        sources.add(AudioSource {
            bytes: bytes.into(),
        })
    }
    commands.insert_resource(AudioAssets {
        blip: embed(&mut sources, include_bytes!("../../assets/audio/blip.wav")),
        scan: embed(&mut sources, include_bytes!("../../assets/audio/scan.wav")),
        alert: embed(&mut sources, include_bytes!("../../assets/audio/alert.wav")),
        mission: embed(
            &mut sources,
            include_bytes!("../../assets/audio/mission.wav"),
        ),
    });
}

/// Edge-detection memory so each event fires its cue exactly once.
#[derive(Default)]
pub struct AudioMemory {
    init: bool,
    scan_active: bool,
    alert_count: usize,
    mission_score: u32,
}

fn play(commands: &mut Commands, src: &Handle<AudioSource>, volume: f32) {
    commands.spawn(AudioBundle {
        source: src.clone(),
        settings: PlaybackSettings::DESPAWN.with_volume(Volume::new(volume)),
    });
}

pub fn audio_triggers(
    mut commands: Commands,
    st: Res<GraphState>,
    scan: Res<ScanPulse>,
    mission: Res<Mission>,
    assets: Res<AudioAssets>,
    mut picks: EventReader<Picked>,
    mut mem: Local<AudioMemory>,
) {
    let alert_count = st.core.alert_order.len();
    let pick_count = picks.read().count(); // always drain the reader

    // First run: snapshot current state so a pre-loaded backlog doesn't blast.
    if !mem.init {
        mem.init = true;
        mem.scan_active = scan.active;
        mem.alert_count = alert_count;
        mem.mission_score = mission.score;
        return;
    }

    if st.cfg.audio_enabled {
        let vol = st.cfg.audio_volume;
        if scan.active && !mem.scan_active {
            play(&mut commands, &assets.scan, vol);
        }
        if alert_count > mem.alert_count {
            play(&mut commands, &assets.alert, vol);
        }
        if mission.score > mem.mission_score {
            play(&mut commands, &assets.mission, vol);
        }
        if pick_count > 0 {
            play(&mut commands, &assets.blip, vol);
        }
    }

    mem.scan_active = scan.active;
    mem.alert_count = alert_count;
    mem.mission_score = mission.score;
}
