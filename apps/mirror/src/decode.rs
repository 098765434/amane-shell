//! The video stream into frames amane can draw: H.264 decoded by ffmpeg,
//! on the GPU through VAAPI when it can be, else on the processor; each
//! frame handed over as NV12, which the shell's quad converts to colour.

use std::io::Read;
use std::ptr;

use amane::{VideoColors, VideoFeed, VideoFrame};
use ffmpeg::ffi;
use ffmpeg::format::Pixel;
use ffmpeg::util::color::{Range, Space};
use ffmpeg_next as ffmpeg;

use crate::protocol::{self, Codec, HEADER_SIZE, Header};

pub struct Decoder {
    decoder: ffmpeg::decoder::Video,
    // a frame as decoded, on the GPU when VAAPI decodes
    decoded: ffmpeg::frame::Video,
    // the same frame copied back to memory
    copied: ffmpeg::frame::Video,
    pub hardware: bool,
}

// picks VAAPI's frames when the decoder offers them, else what it likes best
unsafe extern "C" fn vaapi_format(
    _: *mut ffi::AVCodecContext,
    formats: *const ffi::AVPixelFormat,
) -> ffi::AVPixelFormat {
    let mut format = formats;
    unsafe {
        while *format != ffi::AVPixelFormat::AV_PIX_FMT_NONE {
            if *format == ffi::AVPixelFormat::AV_PIX_FMT_VAAPI {
                return *format;
            }
            format = format.add(1);
        }
        *formats
    }
}

impl Decoder {
    /// An H.264 (or H.265) decoder, on the GPU when `hardware` and VAAPI works.
    pub fn new(codec: Codec, hardware: bool) -> Result<Self, String> {
        ffmpeg::init().map_err(|error| error.to_string())?;

        let id = match codec {
            Codec::H264 => ffmpeg::codec::Id::H264,
            Codec::H265 => ffmpeg::codec::Id::HEVC,
            Codec::Av1 => ffmpeg::codec::Id::AV1,
            other => return Err(format!("The phone sent video this can't play ({other:?}).")),
        };

        let found = ffmpeg::decoder::find(id).ok_or("ffmpeg has no decoder for the video")?;
        let mut context = ffmpeg::codec::Context::new_with_codec(found);

        let mut on_gpu = false;

        unsafe {
            let raw = context.as_mut_ptr();

            // every frame as soon as it can be shown, none held back to reorder
            (*raw).flags |= ffi::AV_CODEC_FLAG_LOW_DELAY as i32;

            if hardware {
                let mut device = ptr::null_mut();
                let made = ffi::av_hwdevice_ctx_create(
                    &mut device,
                    ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_VAAPI,
                    ptr::null(),
                    ptr::null_mut(),
                    0,
                );
                if made == 0 {
                    (*raw).hw_device_ctx = ffi::av_buffer_ref(device);
                    (*raw).get_format = Some(vaapi_format);
                    ffi::av_buffer_unref(&mut device);
                    on_gpu = true;
                }
            }
        }

        let decoder = context
            .decoder()
            .video()
            .map_err(|error| format!("The video decoder didn't open: {error}"))?;

        Ok(Self {
            decoder,
            decoded: ffmpeg::frame::Video::empty(),
            copied: ffmpeg::frame::Video::empty(),
            hardware: on_gpu,
        })
    }

    /// Decodes one packet, handing each frame it finishes to `show`.
    pub fn decode(
        &mut self,
        data: &[u8],
        pts: Option<u64>,
        key: bool,
        mut show: impl FnMut(VideoFrame),
    ) -> Result<(), String> {
        let mut packet = ffmpeg::Packet::copy(data);
        packet.set_pts(pts.map(|pts| pts as i64));
        if key {
            packet.set_flags(ffmpeg::packet::Flags::KEY);
        }

        // a damaged packet loses a frame or two, not the stream
        if self.decoder.send_packet(&packet).is_err() {
            return Ok(());
        }

        while self.decoder.receive_frame(&mut self.decoded).is_ok() {
            let frame = if self.decoded.format() == Pixel::VAAPI {
                let copied = unsafe {
                    ffi::av_hwframe_transfer_data(
                        self.copied.as_mut_ptr(),
                        self.decoded.as_ptr(),
                        0,
                    )
                };
                if copied < 0 {
                    return Err("The GPU's frame couldn't be read back.".into());
                }
                unsafe {
                    (*self.copied.as_mut_ptr()).color_range = (*self.decoded.as_ptr()).color_range;
                    (*self.copied.as_mut_ptr()).colorspace = (*self.decoded.as_ptr()).colorspace;
                }
                &self.copied
            } else {
                &self.decoded
            };

            if let Some(frame) = nv12(frame) {
                show(frame);
            }
        }

        Ok(())
    }
}

/// The frame's planes packed tightly as NV12, from NV12 or planar 4:2:0.
pub fn nv12(frame: &ffmpeg::frame::Video) -> Option<VideoFrame> {
    let width = frame.width();
    let height = frame.height();

    if width == 0 || height == 0 {
        return None;
    }

    let chroma_width = width.div_ceil(2) as usize;
    let chroma_height = height.div_ceil(2) as usize;

    let full_range =
        frame.color_range() == Range::JPEG || matches!(frame.format(), Pixel::YUVJ420P);

    let colors = match frame.color_space() {
        Space::BT470BG | Space::SMPTE170M => VideoColors::Bt601,
        _ => VideoColors::Bt709,
    };

    let mut y = Vec::with_capacity((width * height) as usize);
    let stride = frame.stride(0);
    let plane = frame.data(0);
    for row in 0..height as usize {
        y.extend_from_slice(&plane[row * stride..row * stride + width as usize]);
    }

    let mut uv = Vec::with_capacity(chroma_width * chroma_height * 2);

    match frame.format() {
        Pixel::NV12 => {
            let stride = frame.stride(1);
            let plane = frame.data(1);
            for row in 0..chroma_height {
                uv.extend_from_slice(&plane[row * stride..row * stride + chroma_width * 2]);
            }
        }
        Pixel::YUV420P | Pixel::YUVJ420P => {
            let (u_stride, v_stride) = (frame.stride(1), frame.stride(2));
            let (u, v) = (frame.data(1), frame.data(2));
            for row in 0..chroma_height {
                let u = &u[row * u_stride..row * u_stride + chroma_width];
                let v = &v[row * v_stride..row * v_stride + chroma_width];
                for (u, v) in u.iter().zip(v) {
                    uv.push(*u);
                    uv.push(*v);
                }
            }
        }
        _ => return None,
    }

    Some(VideoFrame {
        width,
        height,
        y,
        uv,
        colors,
        full_range,
    })
}

/// Reads the video socket until it ends: the codec, the sessions and the
/// packets, each frame published to `feed`; `session` hears each new size.
pub fn run(
    mut stream: impl Read,
    feed: &VideoFeed,
    hardware: bool,
    mut session: impl FnMut(u32, u32, bool),
) -> Result<(), String> {
    let codec = Codec::from_id(read_u32(&mut stream)?);

    match codec {
        Codec::Disabled => return Ok(()),
        Codec::Failed => return Err("The phone couldn't start its screen's video.".into()),
        _ => {}
    }

    let mut decoder = Decoder::new(codec, hardware)?;
    session(0, 0, decoder.hardware);

    // the codec's setup arrives alone, and goes in front of the next packet
    let mut config: Option<Vec<u8>> = None;

    loop {
        let mut header = [0u8; HEADER_SIZE];
        if stream.read_exact(&mut header).is_err() {
            return Ok(());
        }

        match protocol::header(&header) {
            Header::Session { width, height } => session(width, height, decoder.hardware),

            Header::Packet {
                size,
                config: is_config,
                key,
                pts,
            } => {
                let mut data = vec![0u8; size as usize];
                if stream.read_exact(&mut data).is_err() {
                    return Ok(());
                }

                if is_config {
                    config = Some(data);
                    continue;
                }

                if let Some(mut setup) = config.take() {
                    setup.extend_from_slice(&data);
                    data = setup;
                }

                decoder.decode(&data, Some(pts), key, |frame| feed.publish(frame))?;
            }
        }
    }
}

pub fn read_u32(stream: &mut impl Read) -> Result<u32, String> {
    let mut word = [0u8; 4];
    stream
        .read_exact(&mut word)
        .map_err(|_| "The phone closed the connection.".to_string())?;
    Ok(u32::from_be_bytes(word))
}

#[cfg(test)]
mod tests {
    use super::*;

    /*
     * an H.264 file made with
     * `ffmpeg -f lavfi -i testsrc=size=320x240:rate=10 -t 1 -c:v libx264 -bsf:v h264_mp4toannexb -f h264 test.h264`,
     * read back through ffmpeg's demuxer into packets, decoded on the processor and on the GPU
     */
    #[test]
    #[ignore = "needs AMANE_MIRROR_TEST_H264 pointing at an H.264 file"]
    fn decodes_h264_to_nv12() {
        let Ok(path) = std::env::var("AMANE_MIRROR_TEST_H264") else {
            return;
        };

        for hardware in [false, true] {
            ffmpeg::init().expect("ffmpeg");
            let mut input = ffmpeg::format::input(&path).expect("the file opens");
            let mut decoder = Decoder::new(Codec::H264, hardware).expect("a decoder");

            let mut frames = Vec::new();
            for (_, packet) in input.packets() {
                decoder
                    .decode(
                        packet.data().expect("data"),
                        None,
                        packet.is_key(),
                        |frame| frames.push(frame),
                    )
                    .expect("decodes");
            }

            assert!(!frames.is_empty(), "hardware {hardware}: no frames");
            let frame = &frames[0];
            assert_eq!((frame.width, frame.height), (320, 240));
            assert_eq!(frame.y.len(), 320 * 240);
            assert_eq!(frame.uv.len(), 160 * 120 * 2);
            eprintln!(
                "hardware asked {hardware}, used {}: {} frames",
                decoder.hardware,
                frames.len()
            );
        }
    }
}
