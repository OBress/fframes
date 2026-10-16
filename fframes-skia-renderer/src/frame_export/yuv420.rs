//! RGBA to yuv420p with Skia's own conversion.
//!
//! `Surface::async_rescale_and_read_pixels_yuv420` draws the Y, U and V planes of the frame
//! and copies them into memory Skia owns; from there they are copied into the frame of the
//! encoder. Skia converts 8 bit 4:2:0 of even sizes only, the shader of [`super::planes`]
//! converts everything else.
use std::{cell::Cell, rc::Rc, thread, time::Duration};

use fframes::ffmpeg_sys_fframes::AVPixelFormat::{self, AV_PIX_FMT_YUV420P};
use fframes::{FFramesRendererError, FFramesRendererResult, FramePool, VideoFrame};
use skia_safe::image::{RescaleGamma, RescaleMode};
use skia_safe::{IRect, Surface, YUVColorSpace, gpu};

use crate::{QueueLock, lock_queue};

/// Converts rendered frames to yuv420p with Skia, see the module docs.
pub(crate) struct Yuv420Exporter {
    pool: FramePool,
    width: i32,
    height: i32,
}

impl Yuv420Exporter {
    /// `None` unless `format` is yuv420p of an even size.
    pub(crate) fn new(
        format: AVPixelFormat,
        width: i32,
        height: i32,
    ) -> Option<FFramesRendererResult<Self>> {
        (format == AV_PIX_FMT_YUV420P && width % 2 == 0 && height % 2 == 0).then(|| {
            let pool = FramePool::new(format, width, height)
                .map_err(|err| FFramesRendererError::from_chunk(0, err))?;
            Ok(Self {
                pool,
                width,
                height,
            })
        })
    }

    /// Converts what was drawn into `surface`, submits it and waits for the planes. The
    /// queue is held for the submission only, like [`crate::SurfaceReader`] does.
    pub(crate) fn read(
        &mut self,
        gpu: &mut gpu::DirectContext,
        surface: &mut Surface,
        queue_lock: Option<&QueueLock>,
    ) -> FFramesRendererResult<VideoFrame> {
        let frame = self
            .pool
            .get()
            .map_err(|err| FFramesRendererError::from_chunk(0, err))?;
        let (width, height) = (self.width, self.height);

        // Skia records the conversion and the copy into its transfer buffers; the callback
        // runs once the GPU is done with them, when the context checks its fences.
        let result = Rc::new(Cell::new(None));
        surface.async_rescale_and_read_pixels_yuv420(
            YUVColorSpace::Rec601_Limited,
            None,
            IRect::from_wh(width, height),
            (width, height),
            RescaleGamma::Src,
            RescaleMode::Nearest,
            {
                let result = result.clone();
                move |planes| result.set(Some(planes))
            },
        );
        {
            let _queue = lock_queue(queue_lock);
            gpu.submit(gpu::SyncCpu::No);
        }
        let planes = loop {
            if let Some(planes) = result.take() {
                break planes;
            }
            gpu.check_async_work_completion();
            thread::sleep(Duration::from_micros(50));
        };

        let planes = planes.filter(|planes| planes.count() == 3).ok_or_else(|| {
            FFramesRendererError::Skia("Skia could not convert the frame to yuv420p".to_owned())
        })?;
        let plane_sizes = [
            (width, height),
            (width / 2, height / 2),
            (width / 2, height / 2),
        ];
        for (index, (bytes, rows)) in plane_sizes.into_iter().enumerate() {
            let (data, linesize) = frame.plane(index);
            let source = planes.data(index).cast::<u8>();
            let row_bytes = planes.row_bytes(index);
            for row in 0..rows as usize {
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        source.add(row * row_bytes),
                        data.add(row * linesize as usize),
                        bytes as usize,
                    );
                }
            }
        }

        Ok(frame)
    }
}
