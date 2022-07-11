use std::fmt;
use std::sync::{Arc, Mutex};

use crate::bridge::{ffi, GetInner};
use crate::camera::InnerCamera;
use crate::Result;

pub use ffi::DefaultPixelFormat as PixelFormat;

pub(crate) struct InnerCameraConfig {
  inner: ffi::BindCameraConfiguration,
  pub(crate) streams: Vec<InnerStreamConfig>,
}

impl InnerCameraConfig {
  pub(crate) fn wrap_inner(mut inner: ffi::BindCameraConfiguration) -> Result<InnerCameraConfig> {
    let streams = (0..unsafe { inner.get().size() })
      .map(|n| {
        Ok(InnerStreamConfig::wrap_inner(unsafe {
          inner.get_mut().at(n.try_into()?)
        }?))
      })
      .collect::<Result<Vec<_>>>()?;
    Ok(InnerCameraConfig { inner, streams })
  }
  pub(crate) fn get_inner(&mut self) -> &mut ffi::BindCameraConfiguration {
    &mut self.inner
  }
}

impl fmt::Debug for InnerCameraConfig {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("CameraConfig")
      .field("streams", &self.streams)
      .finish_non_exhaustive()
  }
}

pub(crate) struct InnerStreamConfig {
  inner: ffi::BindStreamConfiguration,
}

impl InnerStreamConfig {
  pub(crate) fn wrap_inner(inner: ffi::BindStreamConfiguration) -> Self {
    InnerStreamConfig { inner }
  }
  pub(crate) fn get_inner(&self) -> &ffi::BindStreamConfiguration {
    &self.inner
  }
  pub(crate) fn set_pixel_format(&mut self, fmt: PixelFormat) {
    unsafe {
      self
        .inner
        .get_mut()
        .set_pixel_format(ffi::get_default_pixel_format(fmt))
    };
  }
  pub(crate) fn get_pixel_format(&self) -> Option<PixelFormat> {
    let pixel_format = unsafe { self.inner.get().get_pixel_format() };
    unsafe { pixel_format.get().as_default_pixel_format() }.ok()
  }
  pub(crate) fn set_size(&mut self, width: u32, height: u32) {
    unsafe { self.inner.get_mut().set_size(ffi::new_size(width, height)) };
  }
  pub(crate) fn get_size(&self) -> (u32, u32) {
    let size = unsafe { self.inner.get().get_size() };
    (unsafe { size.get().get_width() }, unsafe {
      size.get().get_height()
    })
  }
  pub(crate) fn description(&self) -> String {
    unsafe { self.inner.get().raw_to_string() }
  }
}

impl fmt::Debug for InnerStreamConfig {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("StreamConfig")
      .field("size", &self.get_size())
      .field("pixel_format", &self.get_pixel_format())
      .finish_non_exhaustive()
  }
}

// Public shim structs because we are unable to borrow from a InnerCamera, because it is behind a Mutex.
/// Represents the configuration for a camera.
pub struct CameraConfig {
  camera: Arc<Mutex<InnerCamera>>,
  stream_configs: Vec<StreamConfig>,
}

impl CameraConfig {
  pub(crate) fn new(camera: Arc<Mutex<InnerCamera>>) -> CameraConfig {
    let mut cam = camera.lock().unwrap();
    let config = cam.config.as_mut().unwrap();
    CameraConfig {
      stream_configs: config
        .streams
        .iter()
        .enumerate()
        .map(|(at, ..)| StreamConfig {
          camera: camera.clone(),
          at,
        })
        .collect(),
      camera: camera.clone(),
    }
  }
  /// Get a reference to the vec of stream configurations contained within this camera configuration.
  pub fn streams(&self) -> &Vec<StreamConfig> {
    &self.stream_configs
  }
  /// Get a mutable reference to the vec of stream configurations contained within this camera configuration.
  pub fn streams_mut(&mut self) -> &mut Vec<StreamConfig> {
    &mut self.stream_configs
  }
}

impl fmt::Debug for CameraConfig {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    (&self.camera.lock().unwrap().config.as_mut().unwrap() as &dyn fmt::Debug).fmt(f)
  }
}

/// Represents the configuration for a stream in a camera.
pub struct StreamConfig {
  camera: Arc<Mutex<InnerCamera>>,
  at: usize,
}

impl StreamConfig {
  /// Set the pixel format for this stream to a [PixelFormat]
  pub fn set_pixel_format(&mut self, fmt: PixelFormat) {
    let mut cam = self.camera.lock().unwrap();
    let config = &mut cam.config.as_mut().unwrap().streams[self.at];
    config.set_pixel_format(fmt)
  }
  /// Retrieve the current pixel format
  ///
  /// # Returns
  /// Returns None if the pixel format is not a known pixel format.
  pub fn get_pixel_format(&self) -> Option<PixelFormat> {
    let cam = self.camera.lock().unwrap();
    let config = &cam.config.as_ref().unwrap().streams[self.at];
    config.get_pixel_format()
  }
  /// Set the target image size for this stream.
  pub fn set_size(&mut self, width: u32, height: u32) {
    let mut cam = self.camera.lock().unwrap();
    let config = &mut cam.config.as_mut().unwrap().streams[self.at];
    config.set_size(width, height)
  }
  /// Get the target image size for this stream.
  pub fn get_size(&self) -> (u32, u32) {
    let cam = self.camera.lock().unwrap();
    let config = &cam.config.as_ref().unwrap().streams[self.at];
    config.get_size()
  }
  /// Get a human-readable description of this stream configuraiton from libcamera.
  pub fn description(&self) -> String {
    let cam = self.camera.lock().unwrap();
    let config = &cam.config.as_ref().unwrap().streams[self.at];
    config.description()
  }
}

impl fmt::Debug for StreamConfig {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    (&self.camera.lock().unwrap().config.as_ref().unwrap().streams[self.at] as &dyn fmt::Debug)
      .fmt(f)
  }
}
