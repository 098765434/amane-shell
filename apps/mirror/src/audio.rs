//! The phone's sound, played here: Opus decoded by ffmpeg into a
//! PulseAudio stream named "Phone", so the shell's mixer shows it beside
//! the other apps. The phone's own speaker goes quiet meanwhile (Android
//! routes playback to the capture).

use std::io::Read;
use std::time::Duration;

use ffmpeg::ffi;
use ffmpeg::format::Sample;
use ffmpeg::format::sample::Type;
use ffmpeg_next as ffmpeg;
use libpulse_binding::def::BufferAttr;
use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;

use crate::decode::read_u32;
use crate::protocol::{self, Codec, HEADER_SIZE, Header};

const RATE: u32 = 48_000;
const CHANNELS: u8 = 2;

/// What the stream keeps queued: enough to ride out the network's jitter.
const TARGET: Duration = Duration::from_millis(50);

/// Past this behind, what's queued is dropped to catch up.
const TOO_LATE: Duration = Duration::from_millis(250);

/// Whether the phone sends sound at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    Playing,
    /// Android 10 and older can't capture it.
    Unavailable,
}

fn bytes_for(duration: Duration) -> u32 {
    let frame = 4 * u32::from(CHANNELS);
    (RATE as f64 * duration.as_secs_f64()) as u32 * frame
}

fn opus_decoder() -> Result<ffmpeg::decoder::Audio, String> {
    ffmpeg::init().map_err(|error| error.to_string())?;

    let codec = ffmpeg::decoder::find(ffmpeg::codec::Id::OPUS).ok_or("ffmpeg has no Opus")?;
    let mut context = ffmpeg::codec::Context::new_with_codec(codec);

    // the server's stream is always this; its Opus header isn't needed for it
    // SAFETY: the context is ours, allocated and not yet opened, so its fields may be set
    unsafe {
        let raw = context.as_mut_ptr();
        (*raw).sample_rate = RATE as i32;
        ffi::av_channel_layout_default(&mut (*raw).ch_layout, i32::from(CHANNELS));
    }

    context
        .decoder()
        .audio()
        .map_err(|error| format!("The sound decoder didn't open: {error}"))
}

/// Reads the audio socket until it ends, playing what comes; `started`
/// hears whether the phone sends any.
pub fn run(mut stream: impl Read, mut started: impl FnMut(Sound)) -> Result<(), String> {
    let codec = Codec::from_id(read_u32(&mut stream)?);

    match codec {
        Codec::Opus => {}
        Codec::Disabled | Codec::Failed => {
            started(Sound::Unavailable);
            return Ok(());
        }
        other => return Err(format!("The phone sent sound this can't play ({other:?}).")),
    }

    let mut decoder = opus_decoder()?;

    let spec = Spec {
        format: Format::F32le,
        channels: CHANNELS,
        rate: RATE,
    };
    let attr = BufferAttr {
        maxlength: u32::MAX,
        tlength: bytes_for(TARGET),
        prebuf: u32::MAX,
        minreq: u32::MAX,
        fragsize: u32::MAX,
    };

    let output = Simple::new(
        None,
        "Phone Screen",
        Direction::Playback,
        None,
        "Phone",
        &spec,
        None,
        Some(&attr),
    )
    .map_err(|error| format!("Sound can't play here: {error}"))?;

    started(Sound::Playing);

    let mut frame = ffmpeg::frame::Audio::empty();
    let mut samples: Vec<u8> = Vec::new();

    loop {
        let mut header = [0u8; HEADER_SIZE];
        if stream.read_exact(&mut header).is_err() {
            break;
        }

        let Header::Packet {
            size, config, pts, ..
        } = protocol::header(&header)
        else {
            continue;
        };

        // the size comes from the other end
        if size > protocol::MAX_PACKET {
            break;
        }
        let mut data = vec![0u8; size as usize];
        if stream.read_exact(&mut data).is_err() {
            break;
        }

        // Opus's header, which the decoder doesn't need
        if config {
            continue;
        }

        let mut packet = ffmpeg::Packet::copy(&data);
        packet.set_pts(Some(pts as i64));

        if decoder.send_packet(&packet).is_err() {
            continue;
        }

        while decoder.receive_frame(&mut frame).is_ok() {
            samples.clear();
            interleave(&frame, &mut samples);

            if output.write(&samples).is_err() {
                return Err("The sound stream stopped.".into());
            }
        }

        // a stall on the network piles sound up; better a skip than a lag
        if output
            .get_latency()
            .is_ok_and(|latency| Duration::from_micros(latency.0) > TOO_LATE)
        {
            // sound is best effort: a device gone mid-stream just goes quiet
            let _ = output.flush();
        }
    }

    // as above
    let _ = output.drain();

    Ok(())
}

/// A decoded frame as interleaved 32-bit floats, whatever layout ffmpeg gave.
fn interleave(frame: &ffmpeg::frame::Audio, out: &mut Vec<u8>) {
    let count = frame.samples();
    let channels = usize::from(frame.channels().max(1));

    match frame.format() {
        Sample::F32(Type::Planar) => {
            for index in 0..count {
                for channel in 0..usize::from(CHANNELS) {
                    let plane = frame.plane::<f32>(channel.min(channels - 1));
                    out.extend(plane[index].to_le_bytes());
                }
            }
        }
        Sample::F32(Type::Packed) => {
            let data = frame.data(0);
            out.extend_from_slice(&data[..count * channels * 4]);
        }
        Sample::I16(Type::Packed) => {
            let data = frame.data(0);
            for pair in data[..count * channels * 2].as_chunks::<2>().0 {
                let sample = f32::from(i16::from_le_bytes(*pair)) / 32768.0;
                out.extend(sample.to_le_bytes());
            }
        }
        _ => {}
    }
}
