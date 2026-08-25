//! Global audio player: one track at a time, playable from anywhere in the
//! app and controlled from anywhere (attachment cards, the media bar).
//!
//! Decoding and output run on a dedicated audio thread (`thread`); the UI
//! talks to it through [`MediaPlayer`], a gpui [`Global`] holding an
//! `Entity<PlayerState>` that views `observe` for re-renders. A 200ms timer
//! task copies progress reported by the audio thread into the entity.

pub mod element;
mod thread;

use std::{
  cell::{Cell, RefCell},
  sync::{Arc, Mutex, mpsc},
  time::Duration,
};

use gpui::{App, AppContext as _, Entity, Global};

use crate::thread::{Command, Shared, audio_thread};

/// How often the UI copies progress out of the audio thread.
const TICK: Duration = Duration::from_millis(200);

// ---- model ---------------------------------------------------------------

/// Where a track's encoded bytes come from.
#[derive(Clone, Debug)]
pub enum MediaSource {
  /// Fetched with reqwest on the tokio runtime.
  Url(String),
  /// Already in memory (e.g. loaded from the app's asset source).
  Bytes(Arc<Vec<u8>>),
}

/// One playable item.
#[derive(Clone, Debug)]
pub struct Track {
  /// Stable identity — by convention the attachment url, so message cards can
  /// tell whether "their" track is the one loaded.
  pub id: String,
  pub title: String,
  pub subtitle: Option<String>,
  pub source: MediaSource,
  /// Used when the decoder cannot report a duration (e.g. some streams).
  pub duration_hint: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaybackStatus {
  Stopped,
  /// Fetching or decoding.
  Loading,
  Playing,
  Paused,
  Error(String),
}

/// Observable player state. UI reads it via [`MediaPlayer::state`] and
/// mutates it only through [`MediaPlayer`] methods.
pub struct PlayerState {
  pub track: Option<Track>,
  pub status: PlaybackStatus,
  pub position: Duration,
  pub duration: Option<Duration>,
  pub volume: f32,
  pub muted: bool,
}

impl Default for PlayerState {
  fn default() -> Self {
    PlayerState {
      track: None,
      status: PlaybackStatus::Stopped,
      position: Duration::ZERO,
      duration: None,
      volume: 1.0,
      muted: false,
    }
  }
}

impl PlayerState {
  /// True when `id` is the loaded track.
  pub fn is_current(&self, id: &str) -> bool {
    self.track.as_ref().is_some_and(|t| t.id == id)
  }

  /// 0.0..=1.0 through the track, when the duration is known.
  pub fn fraction(&self) -> Option<f32> {
    let duration = self.duration?;
    if duration.is_zero() {
      return None;
    }
    Some((self.position.as_secs_f32() / duration.as_secs_f32()).clamp(0., 1.))
  }
}

// ---- global --------------------------------------------------------------

/// The gpui global. All methods are associated functions taking `&mut App`
/// so click handlers can call them directly.
pub struct MediaPlayer {
  state: Entity<PlayerState>,
  tx: mpsc::Sender<Command>,
  shared: Arc<Mutex<Shared>>,
  /// Bumped on every `play`; stale fetch/decode results are ignored.
  generation: Cell<u64>,
  /// Bytes of the loaded track, kept so an ended track can replay.
  bytes: RefCell<Option<Arc<Vec<u8>>>>,
}

impl Global for MediaPlayer {}

/// Start the audio thread and register the global. Call once at startup.
pub fn init(cx: &mut App) {
  let state = cx.new(|_| PlayerState::default());
  let (tx, rx) = mpsc::channel();
  let shared = Arc::new(Mutex::new(Shared::default()));

  {
    let shared = shared.clone();
    std::thread::Builder::new().name("scope-audio".into()).spawn(move || audio_thread(rx, shared)).expect("failed to spawn the audio thread");
  }

  cx.set_global(MediaPlayer {
    state,
    tx,
    shared,
    generation: Cell::new(0),
    bytes: RefCell::new(None),
  });

  cx.spawn(async move |cx| {
    loop {
      cx.background_executor().timer(TICK).await;
      if cx.update(tick).is_err() {
        break;
      }
    }
  })
  .detach();

  // Demo hook: `SCOPE_DEMO_AUTOPLAY=1` starts the bundled track on launch so
  // the media bar can be exercised (and screenshotted) without clicking.
  if std::env::var("SCOPE_DEMO_AUTOPLAY").is_ok_and(|v| v == "1")
    && let Ok(Some(bytes)) = cx.asset_source().load("demo/set.wav")
  {
    MediaPlayer::play(
      Track {
        id: "demo/set.wav".into(),
        title: "set.wav".into(),
        subtitle: Some("Demo track".into()),
        source: MediaSource::Bytes(Arc::new(bytes.into_owned())),
        duration_hint: None,
      },
      cx,
    );
  }
}

impl MediaPlayer {
  /// The observable state; panels `cx.observe` this for re-renders.
  pub fn state(cx: &App) -> Entity<PlayerState> {
    cx.global::<MediaPlayer>().state.clone()
  }

  /// Load and play `track`, replacing whatever is playing. Fetching a url
  /// happens on the tokio runtime; results come back through the ticker.
  pub fn play(track: Track, cx: &mut App) {
    let this = cx.global::<MediaPlayer>();
    let generation = this.generation.get() + 1;
    this.generation.set(generation);

    let tx = this.tx.clone();
    let shared = this.shared.clone();
    let state = this.state.clone();
    let duration_hint = track.duration_hint.map(Duration::from_secs_f32);

    match track.source.clone() {
      MediaSource::Bytes(bytes) => {
        *this.bytes.borrow_mut() = Some(bytes.clone());
        let _ = tx.send(Command::Play {
          bytes,
          generation,
          duration_hint,
        });
      }
      MediaSource::Url(url) => {
        *this.bytes.borrow_mut() = None;
        tokio::spawn(async move {
          let fetched = fetch(&url).await;
          match fetched {
            Ok(bytes) => {
              let _ = tx.send(Command::Play {
                bytes: Arc::new(bytes),
                generation,
                duration_hint,
              });
            }
            Err(error) => {
              log::warn!("media fetch failed for {url}: {error:#}");
              let mut shared = shared.lock().unwrap();
              shared.generation = generation;
              shared.error = Some("Couldn't download this file".into());
            }
          }
        });
      }
    }

    state.update(cx, |s, cx| {
      s.track = Some(track);
      s.status = PlaybackStatus::Loading;
      s.position = Duration::ZERO;
      s.duration = duration_hint;
      cx.notify();
    });
  }

  pub fn pause(cx: &mut App) {
    let this = cx.global::<MediaPlayer>();
    let _ = this.tx.send(Command::Pause);
    let state = this.state.clone();
    state.update(cx, |s, cx| {
      if s.status == PlaybackStatus::Playing {
        s.status = PlaybackStatus::Paused;
        cx.notify();
      }
    });
  }

  pub fn resume(cx: &mut App) {
    // An ended track has drained the audio queue; replay it from the start.
    let this = cx.global::<MediaPlayer>();
    let state = this.state.clone();
    let ended = state.read(cx).status == PlaybackStatus::Paused && this.shared.lock().unwrap().ended;

    if ended {
      let replay = state.read(cx).track.clone().zip(this.bytes.borrow().clone());
      if let Some((track, bytes)) = replay {
        return Self::play(
          Track {
            source: MediaSource::Bytes(bytes),
            ..track
          },
          cx,
        );
      }
    }

    let _ = this.tx.send(Command::Resume);
    state.update(cx, |s, cx| {
      if s.status == PlaybackStatus::Paused {
        s.status = PlaybackStatus::Playing;
        cx.notify();
      }
    });
  }

  /// Pause when playing, play when paused (or errored: retry is meaningless,
  /// so this is a no-op then).
  pub fn toggle(cx: &mut App) {
    let status = Self::state(cx).read(cx).status.clone();
    match status {
      PlaybackStatus::Playing | PlaybackStatus::Loading => Self::pause(cx),
      PlaybackStatus::Paused => Self::resume(cx),
      _ => {}
    }
  }

  /// Jump to `fraction` (0.0..=1.0) of the track, when seekable.
  pub fn seek_fraction(fraction: f32, cx: &mut App) {
    let state = Self::state(cx);
    let snapshot = state.read(cx);
    let Some(duration) = snapshot.duration else { return };
    if !matches!(snapshot.status, PlaybackStatus::Playing | PlaybackStatus::Paused) {
      return;
    }

    let position = duration.mul_f32(fraction.clamp(0., 1.));
    let _ = cx.global::<MediaPlayer>().tx.send(Command::Seek(position));
    state.update(cx, |s, cx| {
      s.position = position;
      cx.notify();
    });
  }

  /// Unload the track entirely; the media bar hides.
  pub fn stop(cx: &mut App) {
    let this = cx.global::<MediaPlayer>();
    this.generation.set(this.generation.get() + 1);
    *this.bytes.borrow_mut() = None;
    let _ = this.tx.send(Command::Stop);
    let state = this.state.clone();
    state.update(cx, |s, cx| {
      s.track = None;
      s.status = PlaybackStatus::Stopped;
      s.position = Duration::ZERO;
      s.duration = None;
      cx.notify();
    });
  }

  pub fn set_volume(volume: f32, cx: &mut App) {
    let volume = volume.clamp(0., 1.);
    let state = Self::state(cx);
    let muted = state.read(cx).muted;
    if !muted {
      let _ = cx.global::<MediaPlayer>().tx.send(Command::SetVolume(volume));
    }
    state.update(cx, |s, cx| {
      s.volume = volume;
      cx.notify();
    });
  }

  pub fn toggle_mute(cx: &mut App) {
    let state = Self::state(cx);
    let (muted, volume) = {
      let s = state.read(cx);
      (!s.muted, s.volume)
    };
    let _ = cx.global::<MediaPlayer>().tx.send(Command::SetVolume(if muted { 0. } else { volume }));
    state.update(cx, |s, cx| {
      s.muted = muted;
      cx.notify();
    });
  }
}

// ---- ticker --------------------------------------------------------------

/// Copy progress reported by the audio thread (and url fetches) into the
/// state entity. Runs every [`TICK`]; only notifies when something changed.
fn tick(cx: &mut App) {
  let Some(this) = cx.try_global::<MediaPlayer>() else { return };
  let generation = this.generation.get();
  let state = this.state.clone();
  let shared = this.shared.lock().unwrap().clone();

  if shared.generation != generation {
    return; // Reports about a track we've moved on from.
  }

  state.update(cx, |s, cx| {
    let mut changed = false;

    if let Some(error) = shared.error {
      if matches!(s.status, PlaybackStatus::Loading | PlaybackStatus::Playing) {
        s.status = PlaybackStatus::Error(error);
        changed = true;
      }
    } else if s.status == PlaybackStatus::Loading && shared.decoded {
      s.status = PlaybackStatus::Playing;
      s.duration = shared.duration.or(s.duration);
      changed = true;
    } else if s.status == PlaybackStatus::Playing {
      if shared.ended {
        s.status = PlaybackStatus::Paused;
        s.position = s.duration.unwrap_or(shared.position);
        changed = true;
      } else if s.position != shared.position {
        s.position = shared.position;
        changed = true;
      }
    }

    if changed {
      cx.notify();
    }
  });
}

/// Download `url` fully into memory.
async fn fetch(url: &str) -> anyhow::Result<Vec<u8>> {
  let response = reqwest::get(url).await?.error_for_status()?;
  Ok(response.bytes().await?.to_vec())
}

#[cfg(test)]
mod tests {
  use std::io::Cursor;

  use rodio::{Decoder, Source};

  /// The generated demo track must decode and report its real duration, so we
  /// know the decoder features are wired up (wav today; mp3/flac/ogg come
  /// from the same symphonia backend).
  #[test]
  fn decodes_demo_wav() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/demo/set.wav");
    let bytes = std::fs::read(path).expect("assets/demo/set.wav should exist");

    let decoder = Decoder::builder().with_data(Cursor::new(bytes)).with_seekable(true).build().expect("wav should decode");

    let duration = decoder.total_duration().expect("wav should know its duration");
    assert!((duration.as_secs_f32() - 12.0).abs() < 0.5, "expected ~12s, got {duration:?}");

    // And it should actually produce samples.
    assert!(decoder.take(1000).count() == 1000);
  }

  #[test]
  fn voice_wav_decodes_too() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/demo/voice-message.wav");
    let bytes = std::fs::read(path).expect("assets/demo/voice-message.wav should exist");

    let decoder = Decoder::builder().with_data(Cursor::new(bytes)).with_seekable(true).build().expect("wav should decode");
    let duration = decoder.total_duration().expect("wav should know its duration");
    assert!((duration.as_secs_f32() - 7.0).abs() < 0.5, "expected ~7s, got {duration:?}");
  }
}
