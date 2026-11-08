//! The phone's camera as this computer's webcam: `emaki-mirror --camera
//! front|back` runs without a window, takes the camera's stream from
//! scrcpy's server (Android 12 and later) and writes each frame, raw
//! YUV 4:2:0, to the v4l2loopback device named "Phone Camera", where any
//! video call app picks it up. It runs until stopped; the shell starts it
//! as its own user unit, so restarting the shell doesn't end a call.

use std::path::{Path, PathBuf};

use ffmpeg::format::Pixel;
use ffmpeg_next as ffmpeg;

use crate::decode;
use crate::server::{self, Facing, Options};

/// The loopback device's name, as nixos-config loads it.
pub const LABEL: &str = "Phone Camera";

/// What the camera is asked for: enough for a call, light on the network.
const SIZE: &str = "1280x720";

/// The video device whose name is `LABEL`.
pub fn device() -> Option<PathBuf> {
    let entries = std::fs::read_dir("/sys/class/video4linux").ok()?;
    entries.flatten().find_map(|entry| {
        let name = std::fs::read_to_string(entry.path().join("name")).ok()?;
        (name.trim() == LABEL).then(|| Path::new("/dev").join(entry.file_name()))
    })
}

/// Raw frames into the loopback device, through libavdevice's v4l2
/// output, as scrcpy's own v4l2 sink does.
struct Sink {
    output: ffmpeg::format::context::Output,
    encoder: ffmpeg::encoder::Video,
    size: (u32, u32),
    frames: i64,
}

impl Sink {
    fn open(path: &Path, width: u32, height: u32) -> Result<Self, String> {
        ffmpeg::device::register_all();

        let mut output = ffmpeg::format::output_as(path, "v4l2")
            .map_err(|error| format!("{}: {error}", path.display()))?;

        let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::RAWVIDEO)
            .ok_or("ffmpeg has no raw video encoder")?;
        let mut encoder = ffmpeg::codec::Context::new_with_codec(codec)
            .encoder()
            .video()
            .map_err(|error| error.to_string())?;
        encoder.set_width(width);
        encoder.set_height(height);
        encoder.set_format(Pixel::YUV420P);
        encoder.set_time_base((1, 1_000_000));
        let encoder = encoder.open().map_err(|error| error.to_string())?;

        let mut stream = output
            .add_stream(codec)
            .map_err(|error| error.to_string())?;
        stream.set_parameters(&encoder);
        stream.set_time_base((1, 1_000_000));

        output.write_header().map_err(|error| error.to_string())?;

        Ok(Self {
            output,
            encoder,
            size: (width, height),
            frames: 0,
        })
    }

    fn write(&mut self, frame: &ffmpeg::frame::Video) -> Result<(), String> {
        let mut frame = frame.clone();
        frame.set_pts(Some(self.frames));
        self.frames += 1;

        self.encoder
            .send_frame(&frame)
            .map_err(|error| error.to_string())?;

        let mut packet = ffmpeg::Packet::empty();
        while self.encoder.receive_packet(&mut packet).is_ok() {
            packet.set_stream(0);
            packet
                .write(&mut self.output)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

/// Runs until the phone ends the stream: the camera on `facing`, into the loopback.
pub fn run(serial: &str, facing: Facing) -> Result<(), String> {
    let path = device().ok_or(
        "There's no \"Phone Camera\" video device: v4l2loopback isn't loaded with that label.",
    )?;

    let mut options = Options::screen(serial);
    options.audio = false;
    options.control = false;
    options.camera = Some((facing, SIZE.to_string()));

    let mut connection = server::start(&options)?;
    let video = connection.video.take().ok_or("The phone sent no video.")?;

    let mut sink: Option<Sink> = None;
    let mut failure: Option<String> = None;

    // the processor decodes, so frames come out as the planar YUV the loopback takes
    let result = decode::run_frames(
        video,
        false,
        |_, _, _| {},
        |frame| {
            if failure.is_some() {
                return;
            }
            if !matches!(frame.format(), Pixel::YUV420P | Pixel::YUVJ420P) {
                return;
            }
            let size = (frame.width(), frame.height());
            if sink.as_ref().is_none_or(|sink| sink.size != size) {
                match Sink::open(&path, size.0, size.1) {
                    Ok(opened) => sink = Some(opened),
                    Err(why) => {
                        failure = Some(why);
                        return;
                    }
                }
            }
            if let Some(sink) = &mut sink
                && let Err(why) = sink.write(frame)
            {
                failure = Some(why);
            }
        },
    );

    if let Some(why) = failure {
        return Err(why);
    }
    result?;
    match connection.last_error() {
        Some(why) => Err(why),
        None => Ok(()),
    }
}
