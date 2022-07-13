use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::marker::PhantomData;
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Instant;

use crate::bridge::{ffi, GetInner};
use crate::config::{CameraConfig, InnerCameraConfig, PixelFormat};
use crate::controls::CameraControls;
use crate::image::{self, CameraImage, MultiImage};
use crate::{LibcameraError, Result};

pub use ffi::StreamRole;

/// Manages cameras
pub struct CameraManager {
  inner: RwLock<ffi::BindCameraManager>,
}

unsafe impl Send for CameraManager {}
unsafe impl Sync for CameraManager {}

impl Debug for CameraManager {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("CameraManager").finish_non_exhaustive()
  }
}

impl CameraManager {
  /// Constructs a new camera manager
  pub fn new() -> Result<CameraManager> {
    let mut cm = unsafe { ffi::make_camera_manager() };
    // The primary safety concern for the CM is that it must be started once before calling all functions.
    unsafe { cm.get_mut().start() }?;
    Ok(CameraManager {
      inner: RwLock::new(cm),
    })
  }
  /// Get a list of all attached cameras
  pub fn get_camera_names(&self) -> Vec<String> {
    unsafe { self.inner.read().unwrap().get().get_camera_ids() }
  }
  /// Get a camera with a given name
  pub fn get_camera_by_name<'a>(&'a self, name: &str) -> Result<Camera<'a>> {
    let mut cam = unsafe { self.inner.write().unwrap().get_mut().get_camera_by_id(name) }?;
    unsafe { cam.get_mut().acquire() }?;
    let allocator = unsafe { ffi::make_frame_buffer_allocator(cam.get_mut()) };
    let controls = CameraControls::from_libcamera(unsafe { cam.get().get_controls() });
    Ok(Camera::wrap_inner(
      InnerCamera {
        name: name.to_string(),
        config: None,
        inner: cam,
        allocator,
        streams: Vec::new(),
        configured: false,
        started: false,
        next_request_id: 0,
        request_infos: HashMap::new(),
      },
      controls,
      PhantomData,
    ))
  }
}

impl Drop for CameraManager {
  fn drop(&mut self) {
    unsafe { self.inner.write().unwrap().get_mut().stop() };
  }
}

struct CameraBuffer {
  buffer: ffi::BindFrameBuffer,
  request: Option<ffi::BindRequest>,
  planes: Vec<ffi::BindMemoryBuffer>,
}

impl Debug for CameraBuffer {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("CameraBuffer")
      .field("plane_count", &self.planes.len())
      .finish_non_exhaustive()
  }
}

struct CameraStream {
  pixel_format: Option<PixelFormat>,
  width: u32,
  height: u32,
  stream: ffi::BindStream,
  next_buffer: usize,
  buffers: Vec<CameraBuffer>,
}

impl Debug for CameraStream {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("CameraStream")
      .field("pixel_format", &self.pixel_format)
      .field("width", &self.width)
      .field("height", &self.height)
      .field("next_buffer", &self.next_buffer)
      .field("buffers", &self.buffers)
      .finish_non_exhaustive()
  }
}

struct RequestInfo {
  stream_id: usize,
  buffer_id: usize,
  timestamp: Instant,
}

// This struct MUST have it's destructor called before being deallocated, otherwise a libcamera background thread will cause a segfault.
// For this reason, this is created inside an Arc<Mutex> and a shim struct is returned to the user.
pub(crate) struct InnerCamera {
  name: String,
  pub(crate) config: Option<InnerCameraConfig>,
  inner: ffi::BindCamera,
  allocator: ffi::BindFrameBufferAllocator,
  streams: Vec<CameraStream>,
  configured: bool,
  started: bool,
  next_request_id: u64,
  request_infos: HashMap<u64, RequestInfo>,
}

unsafe impl Send for InnerCamera {}
unsafe impl Sync for InnerCamera {}

impl InnerCamera {
  pub fn generate_config(&mut self, caps: &[StreamRole]) -> Result<()> {
    self.configured = false;
    let config = unsafe { self.inner.get_mut().generate_configuration(caps) }?;
    self.config = Some(InnerCameraConfig::wrap_inner(config)?);
    Ok(())
  }
  pub fn apply_config(&mut self) -> Result<ConfigStatus> {
    if let Some(config) = &mut self.config {
      let config_status = unsafe { config.get_inner().get_mut().validate() };
      let (set, result) = match config_status {
        ffi::CameraConfigurationStatus::Valid => (true, Ok(ConfigStatus::Unchanged)),
        ffi::CameraConfigurationStatus::Adjusted => (true, Ok(ConfigStatus::Changed)),
        _ => (false, Err(LibcameraError::InvalidConfig)),
      };
      if set {
        self.configured = true;
        unsafe { self.inner.get_mut().configure(config.get_inner().get_mut()) }?;
      }
      result
    } else {
      Err(LibcameraError::InvalidConfig)
    }
  }
  /// Start the camera so that it's ready to capture images.
  ///
  /// This should only be called once, future calls will do nothing and the camera's streams cannot be configured while it is started.
  /// # Returns
  /// On success returns whether the stream was newly started (i.e. false means the stream was already running).
  /// This will fail if the camera has not been properly configured, or if libcamera decides to not work.
  /// # Panics
  /// This will panic if the buffer sizes produced by libcamera extend past the end of the actual camera memory buffer.
  pub fn start_stream(&mut self) -> Result<bool> {
    if !self.configured {
      return Err(LibcameraError::InvalidConfig);
    }
    if self.started {
      // Do nothing if the camera was already started.
      return Ok(false);
    }
    self.started = true;
    // Ok so:
    // The camera contains streams, each stream has multiple buffers and each buffer has multiple planes.
    // Each request to the camera operates on one buffer on one stream and fills the buffer with data from that stream.
    // To start the camera we must allocate the buffers for all the streams and save them somewhere for future reading.
    // We also must create a request for each buffer that we can re-use later every time we need an image.
    // Technically requests can have multiple buffers, but I don't think know why this would be the case and I don't think it's necessary.

    // For each stream...
    for stream_config in &self
      .config
      .as_ref()
      .ok_or(LibcameraError::InvalidConfig)?
      .streams
    {
      let mut stream = unsafe { stream_config.get_inner().get().stream() };
      // Allocate buffers
      let _buffer_count = unsafe { self.allocator.get_mut().allocate(stream.get_mut()) };
      let (width, height) = stream_config.get_size();
      let mut camera_stream = CameraStream {
        pixel_format: stream_config.get_pixel_format(),
        width,
        height,
        stream,
        next_buffer: 0,
        buffers: Vec::new(),
      };
      // Map memory for buffers
      for mut buffer in unsafe { self.allocator.get().buffers(camera_stream.stream.get_mut()) } {
        let buffer_id = camera_stream.buffers.len();
        unsafe { buffer.get_mut().set_cookie(buffer_id as u64) };
        let mut planes = Vec::new();
        let mut mapped_buffers: HashMap<i32, (Option<ffi::BindMemoryBuffer>, usize, usize)> =
          HashMap::new();
        for plane in unsafe { buffer.get().planes() } {
          let fd = unsafe { plane.get().get_fd() };
          let mapped_buffer = mapped_buffers
            .entry(fd)
            .or_insert((None, 0, unsafe { ffi::fd_len(fd) }?));
          let length = mapped_buffer.2;
          let plane_offset = unsafe { plane.get().get_offset() };
          let plane_length = unsafe { plane.get().get_length() };
          if plane_offset + plane_length > length {
            panic!(
							"Plane is out of buffer: buffer length = {length}, plane offset = {}, plane length = {}",
							unsafe { plane.get().get_offset() },
							unsafe { plane.get().get_length() },
						);
          }
          mapped_buffer.1 = mapped_buffer.1.max(plane_offset + plane_length);
        }
        for plane in unsafe { buffer.get().planes() } {
          let fd = unsafe { plane.get().get_fd() };
          let mapped_buffer = mapped_buffers.get_mut(&fd).unwrap();
          if mapped_buffer.0.is_none() {
            mapped_buffer.0 = Some(unsafe { ffi::mmap_plane(fd, mapped_buffer.1) }?);
          }
          planes.push(unsafe {
            mapped_buffer
              .0
              .as_mut()
              .unwrap()
              .get_mut()
              .sub_buffer(plane.get().get_offset(), plane.get().get_length())
          }?);
        }

        camera_stream.buffers.push(CameraBuffer {
          request: None,
          buffer,
          planes,
        });
      }
      self.streams.push(camera_stream);
    }
    unsafe { self.inner.get_mut().start() }?;
    Ok(true)
  }
  pub fn capture_next_picture(
    &mut self,
    stream_id: usize,
    controls: &CameraControls,
  ) -> Result<u64> {
    let mut stream = &mut self.streams[stream_id];
    let buffer = &mut stream.buffers[stream.next_buffer];
    if buffer.request.is_none() {
      let request_id = self.next_request_id;
      let mut req = unsafe { self.inner.get_mut().create_request(request_id) }?;
      unsafe {
        req
          .get_mut()
          .add_buffer(stream.stream.get(), buffer.buffer.get_mut())
      }?;
      for (control_id, control_value) in controls.get_libcamera() {
        unsafe { req.get_mut().set_control(control_id, control_value.get()) };
      }
      let timestamp = Instant::now();
      unsafe { self.inner.get_mut().queue_request(req.get_mut()) }?;
      self.request_infos.insert(
        request_id,
        RequestInfo {
          stream_id,
          buffer_id: stream.next_buffer,
          timestamp,
        },
      );
      self.next_request_id += 1;
      buffer.request = Some(req);
      stream.next_buffer += 1;
      stream.next_buffer %= stream.buffers.len();
      Ok(request_id)
    } else {
      Err(LibcameraError::NoBufferReady)
    }
  }
  pub fn poll_events(&mut self, match_id: Option<u64>) -> Result<Vec<CameraEvent>> {
    let events = if let Some(match_id) = match_id {
      unsafe { self.inner.get_mut().poll_events_with_cookie(match_id) }
    } else {
      unsafe { self.inner.get_mut().poll_events() }
    };
    Ok(
      events
        .into_iter()
        .flat_map(|event| match event.message_type {
          ffi::CameraMessageType::RequestComplete => {
            println!("Ev: {event:?}");
            let request_id = event.request_cookie;
            let request_info = self.request_infos.remove(&request_id)?;
            println!(
              "Request completed on stream {}, buffer {}.",
              request_info.stream_id, request_info.buffer_id
            );
            let stream = &mut self.streams[request_info.stream_id];
            let buffer = &mut stream.buffers[request_info.buffer_id];
            buffer.request = None;
            println!("Pixel format: {:?}", stream.pixel_format);
            Some(CameraEvent::RequestComplete {
              serial_id: request_id,
              queue_timestamp: request_info.timestamp,
              image: RawCameraImage {
                width: stream.width as usize,
                height: stream.height as usize,
                pixel_format: stream.pixel_format,
                planes: buffer
                  .planes
                  .iter()
                  .map(|plane| unsafe { plane.get().read_to_vec() })
                  .collect(),
              },
            })
          }
          _ => None,
        })
        .collect(),
    )
  }
}

impl Drop for InnerCamera {
  // NOTE: This MUST be called before the camera is deallocated,
  // not doing so might result in a segfault.
  fn drop(&mut self) {
    eprintln!("Dropping inner camera.");
    self.streams = Vec::new();
    unsafe { self.inner.get_mut().stop() }.unwrap();
    unsafe { self.inner.get_mut().release() }.unwrap();
  }
}

// This struct must be destructed before the camera manager it came from.
/// Represents a camera
pub struct Camera<'a> {
  _camera_manager: PhantomData<&'a CameraManager>,
  inner: Arc<Mutex<InnerCamera>>,
  _drop_detector: SyncSender<()>,
  // A shim that internally stores a reference (Arc) to our innercamera.
  camera_config: Option<CameraConfig>,
  // This doesn't maintain a reference to libcamera and can therefore be stored here instead of inside InnerCamera.
  controls: CameraControls,
}

impl Camera<'_> {
  fn wrap_inner(
    inner: InnerCamera,
    controls: CameraControls,
    manager: PhantomData<&'_ CameraManager>,
  ) -> Camera<'_> {
    let (tx, rx) = mpsc::sync_channel(0);
    let inner = Arc::new(Mutex::new(inner));
    // This thread's sole purpose is to drop InnerCamera even when Camera's destructor doesn't run.
    {
      let thread_camera = inner.clone();
      thread::spawn(move || {
        let _ = rx.recv();
        // Require thread_camera to live until at least here.
        eprintln!("Dropping camera from thread.");
        let _cam = thread_camera;
      });
    }
    Camera {
      _camera_manager: manager,
      inner,
      _drop_detector: tx,
      camera_config: None,
      controls,
    }
  }

  /// Generate a configuration for this camera using the given set of stream roles to generate an corresponding set of streams.
  pub fn generate_config(&mut self, caps: &[StreamRole]) -> Result<&mut CameraConfig> {
    self.inner.lock().unwrap().generate_config(caps)?;
    self.camera_config = Some(CameraConfig::new(self.inner.clone()));
    Ok(self.camera_config.as_mut().unwrap())
  }
  /// Validate and apply the configuration previously generated by this camera.
  pub fn apply_config(&mut self) -> Result<ConfigStatus> {
    self.inner.lock().unwrap().apply_config()
  }
  /// Borrow this camera's config.
  pub fn get_config(&self) -> Option<&CameraConfig> {
    self.camera_config.as_ref()
  }
  /// Borrow the camera's controls
  pub fn get_controls(&self) -> &CameraControls {
    &self.controls
  }
  /// Borrow the camera's controls mutably
  pub fn get_controls_mut(&mut self) -> &mut CameraControls {
    &mut self.controls
  }

  /// Start the camera so that it's ready to capture images.
  ///
  /// This should only be called once, future calls will do nothing and the camera's streams cannot be configured while it is started.
  /// # Returns
  /// On success returns whether the stream was newly started (i.e. false means the stream was already running).
  /// This will fail if the camera has not been properly configured, or if libcamera decides to not work.
  /// # Panics
  /// This will panic if the buffer sizes produced by libcamera extend past the end of the actual camera memory buffer.
  pub fn start_stream(&mut self) -> Result<bool> {
    self.inner.lock().unwrap().start_stream()
  }
  /// Start the process to capture an image from the camera.
  ///
  /// # Returns
  /// On success returns the `serial_id` of the request, which can be used to match with the correct request complete event.
  ///
  /// # Errors
  /// Errors if there are no buffers currently available (all buffers are in-use, if this happens take pictures slower!)
  pub fn capture_next_picture(&mut self, stream_id: usize) -> Result<u64> {
    self
      .inner
      .lock()
      .unwrap()
      .capture_next_picture(stream_id, &self.controls)
  }
  /// Poll events from the camera.
  ///
  /// The results should be in order of when the camera sent them, but not neccesarily in order of when they were initially queued. Make sure to use `serial_id`, or the event `timestamp` to keep track of that if you need to.
  pub fn poll_events(&mut self, match_id: Option<u64>) -> Result<Vec<CameraEvent>> {
    self.inner.lock().unwrap().poll_events(match_id)
  }
}

impl Debug for Camera<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let cam = self.inner.lock().unwrap();
    f.debug_struct("Camera")
      .field("name", &cam.name)
      .field("config", &cam.config)
      .field("streams", &cam.streams)
      .field("started", &cam.started)
      .field("controls", &self.controls)
      .field("next_request_id", &cam.next_request_id)
      .finish_non_exhaustive()
  }
}

/// Represents raw image data fetched from the camera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawCameraImage {
  /// The pixel format for the image, if it is known.
  pub pixel_format: Option<PixelFormat>,
  /// The width of the image.
  pub width: usize,
  /// The height of the image.
  pub height: usize,
  /// The raw data planes for the image.
  pub planes: Vec<Vec<u8>>,
}

impl RawCameraImage {
  /// Attempts to decode this camera image.
  ///
  /// Currently only supports Bgr, Rgb, Yuyv, and Yuv420 formats, and Mjpeg with the `image` feature.
  pub fn try_decode(self) -> Option<MultiImage> {
    match (self.pixel_format, self.planes.as_slice()) {
      (Some(PixelFormat::Bgr888), [data]) => {
        image::BgrImage::from_planes(self.width, self.height, [data.to_owned()])
          .map(MultiImage::Bgr)
      }
      (Some(PixelFormat::Rgb888), [data]) => {
        image::RgbImage::from_planes(self.width, self.height, [data.to_owned()])
          .map(MultiImage::Rgb)
      }
      (Some(PixelFormat::Yuyv), [data]) => {
        image::YuyvImage::from_planes(self.width, self.height, [data.to_owned()])
          .map(MultiImage::Yuyv)
      }
      (Some(PixelFormat::Yuv420), [y, u, v]) => image::Yuv420Image::from_planes(
        self.width,
        self.height,
        [y.to_owned(), u.to_owned(), v.to_owned()],
      )
      .map(MultiImage::Yuv420),
      #[cfg(feature = "image")]
      (Some(PixelFormat::Mjpeg), [data]) => {
        image::RgbImage::decode_jpeg(data).ok().map(MultiImage::Rgb)
      }
      _ => None,
    }
  }
}

/// Represents an event from the camera
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CameraEvent {
  /// Triggered when a capture request has completed, containing a vec of the resulting image planes.
  RequestComplete {
    /// The same `serial_id` that was returned from the function that queued this request.
    serial_id: u64,
    /// When this event was __queued__ to the camera.
    queue_timestamp: Instant,
    /// The raw image data for this request, might not actually contain a real image (at the moment there isn't any way of determining success as far as I can tell).
    image: RawCameraImage,
  },
}

/// Represents the result of applying a configuration to a camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigStatus {
  /// The configuration was applied to the camera unchanged
  Unchanged,
  /// The configuration was applied to the camera, but some values have been adjusted by the driver to a supported configuration for this camera
  Changed,
}
