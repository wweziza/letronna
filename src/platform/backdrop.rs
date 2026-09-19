//! Backdrop blur for modal overlays.
//!
//! GPUI has no backdrop-filter: its renderer only draws quads, paths, text and
//! sprites, and `Style` exposes opacity but no filters. So we make the frosted
//! panel ourselves. When a modal opens we grab the window's pixels with
//! `PrintWindow`, shrink and blur them on the CPU, and hand the result back as
//! an image the overlay draws behind itself. The snapshot is taken once, which
//! is what a modal wants anyway: the content behind it is frozen.

use crate::prelude::*;
use std::sync::Arc;
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject,
};
use windows_sys::Win32::Storage::Xps::PrintWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;

/// How much the snapshot is shrunk before blurring. The GPU smooths it back up,
/// which does most of the blurring for free and keeps the CPU work tiny.
const SHRINK: u32 = 10;
/// Box-blur passes over the shrunk image. Three approximates a gaussian.
const PASSES: usize = 3;
/// Blur radius in shrunk pixels.
const RADIUS: i32 = 2;

/// Capture the window and return a blurred copy, or `None` if it cannot be read.
pub(crate) fn capture(window: &Window) -> Option<Arc<RenderImage>> {
    let hwnd = super::window::hwnd(window)?;
    let (width, height, bgra) = unsafe { read_pixels(hwnd)? };
    let (small_w, small_h, mut small) = shrink(width, height, &bgra);
    for _ in 0..PASSES {
        small = box_blur(small_w, small_h, &small);
    }
    lift(&mut small);
    let buffer = image::RgbaImage::from_raw(small_w, small_h, small)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

/// Read the window's pixels as BGRA, which is the order `RenderImage` wants.
unsafe fn read_pixels(hwnd: *mut core::ffi::c_void) -> Option<(u32, u32, Vec<u8>)> {
    unsafe {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if GetClientRect(hwnd, &mut rect) == 0 {
            return None;
        }
        let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
        if w <= 0 || h <= 0 {
            return None;
        }

        let screen = GetDC(std::ptr::null_mut());
        let dc = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, w, h);
        let previous = SelectObject(dc, bitmap);

        // PW_RENDERFULLCONTENT (2) is what makes this work for a GPU-drawn window.
        let drawn = PrintWindow(hwnd, dc, 2) != 0;

        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            // Negative height asks for a top-down buffer.
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut pixels = vec![0u8; (w as usize) * (h as usize) * 4];
        let read = GetDIBits(
            dc,
            bitmap,
            0,
            h as u32,
            pixels.as_mut_ptr().cast(),
            &mut info,
            DIB_RGB_COLORS,
        );

        SelectObject(dc, previous);
        DeleteObject(bitmap);
        DeleteDC(dc);
        ReleaseDC(std::ptr::null_mut(), screen);

        if !drawn || read == 0 {
            return None;
        }
        // BitBlt leaves alpha at zero; the backdrop must be opaque.
        for chunk in pixels.chunks_exact_mut(4) {
            chunk[3] = 255;
        }
        Some((w as u32, h as u32, pixels))
    }
}

/// A blurred near-black UI is just black. Lift it so the backdrop reads as
/// frosted glass rather than a void, leaving alpha alone.
fn lift(pixels: &mut [u8]) {
    for chunk in pixels.chunks_exact_mut(4) {
        for c in chunk.iter_mut().take(3) {
            *c = ((*c as f32 * 1.55) + 14.).min(255.) as u8;
        }
    }
}

/// Average blocks of `SHRINK` pixels into one. Cheap and already a soft blur.
fn shrink(width: u32, height: u32, bgra: &[u8]) -> (u32, u32, Vec<u8>) {
    let out_w = (width / SHRINK).max(1);
    let out_h = (height / SHRINK).max(1);
    let mut out = vec![0u8; (out_w * out_h * 4) as usize];
    for y in 0..out_h {
        for x in 0..out_w {
            let mut sum = [0u32; 4];
            let mut count = 0u32;
            for dy in 0..SHRINK {
                for dx in 0..SHRINK {
                    let sx = x * SHRINK + dx;
                    let sy = y * SHRINK + dy;
                    if sx >= width || sy >= height {
                        continue;
                    }
                    let i = ((sy * width + sx) * 4) as usize;
                    for c in 0..4 {
                        sum[c] += bgra[i + c] as u32;
                    }
                    count += 1;
                }
            }
            let o = ((y * out_w + x) * 4) as usize;
            for c in 0..4 {
                out[o + c] = (sum[c] / count.max(1)) as u8;
            }
        }
    }
    (out_w, out_h, out)
}

/// One box-blur pass. Repeating it approximates a gaussian.
fn box_blur(width: u32, height: u32, src: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let mut sum = [0u32; 4];
            let mut count = 0u32;
            for dy in -RADIUS..=RADIUS {
                for dx in -RADIUS..=RADIUS {
                    let (sx, sy) = (x + dx, y + dy);
                    if sx < 0 || sy < 0 || sx >= width as i32 || sy >= height as i32 {
                        continue;
                    }
                    let i = ((sy as u32 * width + sx as u32) * 4) as usize;
                    for c in 0..4 {
                        sum[c] += src[i + c] as u32;
                    }
                    count += 1;
                }
            }
            let o = ((y as u32 * width + x as u32) * 4) as usize;
            for c in 0..4 {
                out[o + c] = (sum[c] / count.max(1)) as u8;
            }
        }
    }
    out
}
