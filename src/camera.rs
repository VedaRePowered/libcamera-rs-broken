use std::fmt;
use std::marker::PhantomData;

use crate::bridge::{ffi, GetInner};
use crate::config::CameraConfig;

use crate::{LibcameraError, Result};

pub use ffi::StreamRole;

/// Manages cameras
pub struct CameraManager {
  inner: ffi::BindCameraManager,
}

impl fmt::Debug for CameraManager {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("CameraManager").finish_non_exhaustive()
  }
}

impl CameraManager {
  /// Constructs a new camera manager
  pub fn new() -> Result<CameraManager> {
    let mut cm = ffi::make_camera_manager();
    // The primary safety concern for the CM is that it must be started once before calling all functions.
    unsafe { cm.get_mut().start() }?;
    Ok(CameraManager { inner: cm })
  }
  /// Get a list of all attached cameras
  pub fn get_camera_names(&self) -> Vec<String> {
    unsafe { self.inner.get().get_camera_ids() }
  }
  /// Get a camera with a given name
  pub fn get_camera_by_name(&mut self, name: &str) -> Result<Camera<'_>> {
    let mut cam = unsafe { self.inner.get_mut().get_camera_by_id(name) }?;
    unsafe { cam.get_mut().acquire() }?;
    let allocator = unsafe { ffi::make_frame_buffer_allocator(cam.get_mut()) };
    Ok(Camera {
      _camera_manager: PhantomData,
      name: name.to_string(),
      config: None,
      inner: cam,
      allocator,
    })
  }
}

impl Drop for CameraManager {
  fn drop(&mut self) {
    unsafe { self.inner.get_mut().stop() };
  }
}

/// Represents a camera
pub struct Camera<'a> {
  _camera_manager: PhantomData<&'a CameraManager>,
  name: String,
  config: Option<CameraConfig>,
  inner: ffi::BindCamera,
  allocator: ffi::BindFrameBufferAllocator,
}

impl fmt::Debug for Camera<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Camera")
      .field("name", &self.name)
      .finish_non_exhaustive()
  }
}

impl Camera<'_> {
  /// Generate a configuration for this camera using the given set of stream roles to generate an corresponding set of streams.
  pub fn generate_config(&mut self, caps: &[StreamRole]) -> Result<&mut CameraConfig> {
    let config = unsafe { self.inner.get_mut().generate_configuration(caps) }?;
    self.config = Some(CameraConfig::wrap_inner(config)?);
    self.config.as_mut().ok_or(LibcameraError::InvalidConfig)
  }
  /// Validate and apply the configuration previously generated by this camera.
  pub fn apply_config(&mut self) -> Result<ConfigStatus> {
    if let Some(config) = &mut self.config {
      let config_status = unsafe { config.get_inner().get_mut().validate() };
      let (set, result) = match config_status {
        ffi::CameraConfigurationStatus::Valid => (true, Ok(ConfigStatus::Unchanged)),
        ffi::CameraConfigurationStatus::Adjusted => (true, Ok(ConfigStatus::Changed)),
        _ => (false, Err(LibcameraError::InvalidConfig)),
      };
      if set {
        unsafe { self.inner.get_mut().configure(config.get_inner().get_mut()) }?;
      }
      result
    } else {
      Err(LibcameraError::InvalidConfig)
    }
  }
  /// Borrow this camera's config.
  pub fn get_config(&self) -> Option<&CameraConfig> {
    self.config.as_ref()
  }
}

impl Drop for Camera<'_> {
  fn drop(&mut self) {
    unsafe { self.inner.get_mut().release() }.unwrap();
  }
}

/// Represents the result of applying a configuration to a camera.
#[derive(Debug)]
pub enum ConfigStatus {
  /// The configuration was applied to the camera unchanged
  Unchanged,
  /// The configuration was applied to the camera, but some values have been adjusted by the driver to a supported configuration for this camera
  Changed,
}