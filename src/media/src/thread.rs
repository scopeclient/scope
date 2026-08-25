//! The audio thread: owns the rodio output stream (not `Send`) and a
//! `Player` per track. Commands arrive over an mpsc channel; progress goes
//! back through a shared mutex the UI ticker polls.

use std::{
  io::{Read, Seek, SeekFrom},
  sync::{Arc, Mutex, mpsc},
  time::Duration,
};

use rodio::{Decoder, DeviceSinkBuilder, Player, Source, decoder::DecoderError};

/// UI → audio thread.
pub enum Command {
  Play {
    bytes: Arc<Vec<u8>>,
    generation: u64,
    duration_hint: Option<Duration>,
  },
  Pause,
  Resume,
  Seek(Duration),
  SetVolume(f32),
  Stop,
}

/// Audio thread → UI, polled by the ticker. `generation` says which `Play`
/// the report belongs to.
#[derive(Clone, Default)]
pub struct Shared {
  pub generation: u64,
  /// Decoding succeeded and the track was queued.
  pub decoded: bool,
  pub position: Duration,
  pub duration: Option<Duration>,
  pub ended: bool,
  /// User-facing message; fetch errors land here too.
  pub error: Option<String>,
}

/// `Read + Seek` over shared bytes without copying them.
struct ArcCursor {
  data: Arc<Vec<u8>>,
  pos: u64,
}

impl Read for ArcCursor {
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    let data = &self.data[(self.pos.min(self.data.len() as u64)) as usize..];
    let n = data.len().min(buf.len());
    buf[..n].copy_from_slice(&data[..n]);
    self.pos += n as u64;
    Ok(n)
  }
}

impl Seek for ArcCursor {
  fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
    let len = self.data.len() as i64;
    let target = match from {
      SeekFrom::Start(n) => n as i64,
      SeekFrom::End(n) => len + n,
      SeekFrom::Current(n) => self.pos as i64 + n,
    };
    if target < 0 {
      return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "seek before start"));
    }
    self.pos = target as u64;
    Ok(self.pos)
  }
}

/// Runs until the command channel closes (i.e. the app quits).
pub fn audio_thread(rx: mpsc::Receiver<Command>, shared: Arc<Mutex<Shared>>) {
  // One output stream for the app's lifetime; a fresh `Player` per track
  // (dropping a `Player` stops its sound, which is exactly what we want).
  let sink = match DeviceSinkBuilder::open_default_sink() {
    Ok(sink) => Some(sink),
    Err(error) => {
      log::error!("no audio output device: {error}");
      None
    }
  };

  let mut player: Option<Player> = None;
  let mut volume: f32 = 1.0;
  let mut playing = false;

  loop {
    match rx.recv_timeout(Duration::from_millis(100)) {
      Ok(Command::Play {
        bytes,
        generation,
        duration_hint,
      }) => {
        player = None; // Stop the previous track first.
        playing = false;

        let Some(sink) = &sink else {
          report(&shared, generation, |s| s.error = Some("No audio output device".into()));
          continue;
        };

        let byte_len = bytes.len() as u64;
        let cursor = ArcCursor { data: bytes, pos: 0 };

        match Decoder::builder().with_data(cursor).with_byte_len(byte_len).with_seekable(true).with_gapless(true).build() {
          Ok(source) => {
            let duration = source.total_duration().or(duration_hint);
            let next = Player::connect_new(sink.mixer());
            next.set_volume(volume);
            next.append(source);
            player = Some(next);
            playing = true;
            report(&shared, generation, |s| {
              s.decoded = true;
              s.duration = duration;
            });
          }
          Err(error) => {
            log::warn!("audio decode failed: {error}");
            report(&shared, generation, |s| s.error = Some(friendly_decode_error(&error)));
          }
        }
      }
      Ok(Command::Pause) => {
        if let Some(player) = &player {
          player.pause();
        }
      }
      Ok(Command::Resume) => {
        if let Some(player) = &player {
          player.play();
        }
      }
      Ok(Command::Seek(position)) => {
        if let Some(player) = &player {
          match player.try_seek(position) {
            Ok(()) => shared.lock().unwrap().position = position,
            Err(error) => log::warn!("audio seek failed: {error}"),
          }
        }
      }
      Ok(Command::SetVolume(v)) => {
        volume = v;
        if let Some(player) = &player {
          player.set_volume(v);
        }
      }
      Ok(Command::Stop) => {
        player = None;
        playing = false;
      }
      Err(mpsc::RecvTimeoutError::Timeout) => {}
      Err(mpsc::RecvTimeoutError::Disconnected) => break,
    }

    if let Some(current) = &player {
      let mut s = shared.lock().unwrap();
      s.position = current.get_pos();
      if playing && current.empty() {
        s.ended = true;
        playing = false;
      }
    }
  }
}

/// Reset the report to a fresh state for `generation`, then apply `fill`.
fn report(shared: &Arc<Mutex<Shared>>, generation: u64, fill: impl FnOnce(&mut Shared)) {
  let mut s = shared.lock().unwrap();
  *s = Shared {
    generation,
    ..Shared::default()
  };
  fill(&mut s);
}

/// Discord voice messages are Ogg Opus, which symphonia cannot decode; say so
/// nicely instead of leaking decoder jargon.
fn friendly_decode_error(error: &DecoderError) -> String {
  match error {
    DecoderError::UnrecognizedFormat | DecoderError::DecodeError(_) => "This format can't be played yet".into(),
    other => format!("Playback failed: {other}"),
  }
}
