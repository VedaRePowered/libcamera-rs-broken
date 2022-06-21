#![allow(dead_code)]

use std::pin::Pin;

#[cfg(test)]
mod test;

#[cxx::bridge]
pub mod ffi {
  #[namespace = "libcamera"]
  #[repr(i32)]
  #[derive(Debug)]
  enum StreamRole {
    Raw,
    StillCapture,
    VideoRecording,
    Viewfinder,
  }

  #[namespace = "libcamera"]
  #[repr(i32)]
  #[derive(Debug)]
  enum CameraConfigurationStatus {
    Valid,
    Adjusted,
    Invalid,
  }

  #[repr(i32)]
  #[derive(Debug)]
  enum DefaultPixelFormat {
    Rgb888,
    Bgr888,
    Yuv420,
    Mjpeg,
  }

  #[repr(i32)]
  #[derive(Debug)]
  enum BindErrorCode {
    /// Operation not permitted
    EPerm = 1,
    /// No such file or directory
    ENoEnt = 2,
    /// No such process
    ESrch = 3,
    /// Interrupted system call
    EIntr = 4,
    /// I/O error
    EIo = 5,
    /// No such device or address
    ENxIo = 6,
    /// Argument list too long
    E2Big = 7,
    /// EXec format error
    ENoExec = 8,
    /// Bad file number
    EBadF = 9,
    /// No child processes
    EChild = 10,
    /// Try again
    EAgain = 11,
    /// Out of memory
    ENoMem = 12,
    /// Permission denied
    EAcces = 13,
    /// Bad address
    EFault = 14,
    /// Block device required
    ENotBlk = 15,
    /// Device or resource busy
    EBusy = 16,
    /// File exists
    EExist = 17,
    /// Cross-device link
    EXDev = 18,
    /// No such device
    ENoDev = 19,
    /// Not a directory
    ENotDir = 20,
    /// Is a directory
    EIsDir = 21,
    /// Invalid argument
    EInval = 22,
    /// File table overflow
    ENFile = 23,
    /// Too many open files
    EMFile = 24,
    /// Not a typewriter
    ENotTy = 25,
    /// Text file busy
    ETxtBsy = 26,
    /// File too large
    EFBig = 27,
    /// No space left on device
    ENoSpc = 28,
    /// Illegal seek
    ESPipe = 29,
    /// Read-only file system
    ERoFs = 30,
    /// Too many links
    EMLink = 31,
    /// Broken pipe
    EPipe = 32,
    /// Math argument out of domain of func
    EDom = 33,
    /// Math result not representable
    ERange = 34,
  }

  struct BindCameraManager {
    inner: UniquePtr<CameraManager>,
  }
  struct BindCamera {
    inner: UniquePtr<Camera>,
  }
  struct BindCameraConfiguration {
    inner: UniquePtr<CameraConfiguration>,
  }
  struct BindStreamConfiguration {
    inner: UniquePtr<StreamConfiguration>,
  }
  struct BindStream {
    inner: UniquePtr<Stream>,
  }
  struct BindFrameBufferAllocator {
    inner: UniquePtr<FrameBufferAllocator>,
  }
  struct BindFrameBuffer {
    inner: UniquePtr<FrameBuffer>,
  }
  struct BindFrameBufferPlane {
    inner: UniquePtr<FrameBufferPlane>,
  }
  struct BindMemoryBuffer {
    inner: UniquePtr<MemoryBuffer>,
  }
  struct BindRequest {
    inner: UniquePtr<Request>,
  }

  unsafe extern "C++" {
    include!("libcamera-rs/libcamera-bridge/core.hpp");

    #[namespace = "libcamera"]
    type StreamRole;
    type CameraConfigurationStatus;

    type CameraManager;
    pub fn make_camera_manager() -> BindCameraManager;

    pub unsafe fn start(self: Pin<&mut CameraManager>);
    pub unsafe fn stop(self: Pin<&mut CameraManager>);
    pub unsafe fn get_camera_ids(self: Pin<&mut CameraManager>) -> Vec<String>;
    pub unsafe fn get_camera_by_id(self: Pin<&mut CameraManager>, id: &str) -> BindCamera;

    type Camera;
    pub unsafe fn acquire(self: Pin<&mut Camera>);
    pub unsafe fn release(self: Pin<&mut Camera>);
    pub unsafe fn generate_configuration(
      self: Pin<&mut Camera>,
      roles: &[StreamRole],
    ) -> BindCameraConfiguration;
    pub unsafe fn configure(self: Pin<&mut Camera>, conf: Pin<&mut CameraConfiguration>);
    pub unsafe fn create_request(self: Pin<&mut Camera>) -> BindRequest;
    pub unsafe fn queue_request(self: Pin<&mut Camera>, req: Pin<&mut Request>);
    pub unsafe fn start(self: Pin<&mut Camera>);
    pub unsafe fn stop(self: Pin<&mut Camera>);

    type CameraConfiguration;
    pub unsafe fn at(self: Pin<&mut CameraConfiguration>, idx: u32) -> BindStreamConfiguration;
    pub unsafe fn validate(self: Pin<&mut CameraConfiguration>) -> CameraConfigurationStatus;

    type StreamConfiguration;
    pub unsafe fn stream(self: Pin<&mut StreamConfiguration>) -> BindStream;

    type Stream;

    type FrameBufferAllocator;
    pub fn make_frame_buffer_allocator(camera: Pin<&mut Camera>) -> BindFrameBufferAllocator;

    pub unsafe fn allocate(self: Pin<&mut FrameBufferAllocator>, stream: Pin<&mut Stream>);
    pub unsafe fn free(self: Pin<&mut FrameBufferAllocator>, stream: Pin<&mut Stream>);
    pub unsafe fn buffers(
      self: Pin<&mut FrameBufferAllocator>,
      stream: Pin<&mut Stream>,
    ) -> Vec<BindFrameBuffer>;

    type FrameBuffer;
    pub unsafe fn planes(self: Pin<&mut FrameBuffer>) -> Vec<BindFrameBufferPlane>;

    type FrameBufferPlane;
    pub unsafe fn get_fd(self: Pin<&mut FrameBufferPlane>) -> i32;
    pub unsafe fn get_offset(self: Pin<&mut FrameBufferPlane>) -> usize;
    pub unsafe fn get_length(self: Pin<&mut FrameBufferPlane>) -> usize;

    /// File descriptor functions
    pub unsafe fn fd_len(fd: i32) -> usize;
    pub unsafe fn mmap_plane(fd: i32, length: usize) -> BindMemoryBuffer;

    type MemoryBuffer;
    pub unsafe fn sub_buffer(
      self: Pin<&mut MemoryBuffer>,
      offset: usize,
      length: usize,
    ) -> BindMemoryBuffer;
    pub unsafe fn read_to_vec(self: Pin<&mut MemoryBuffer>) -> Vec<u8>;

    type Request;
    pub unsafe fn add_buffer(
      self: Pin<&mut Request>,
      stream: Pin<&mut Stream>,
      buffer: Pin<&mut FrameBuffer>,
    );
  }
}

/// # Safety
/// The inner pointer to the libcamera object must be valid.
unsafe trait PinMut {
  type Inner;
  unsafe fn get(&mut self) -> Pin<&mut Self::Inner>;
}

unsafe impl PinMut for ffi::BindCameraManager {
  type Inner = ffi::CameraManager;
  unsafe fn get(&mut self) -> Pin<&mut Self::Inner> {
    self.inner.pin_mut()
  }
}

unsafe impl PinMut for ffi::BindCamera {
  type Inner = ffi::Camera;
  unsafe fn get(&mut self) -> Pin<&mut Self::Inner> {
    self.inner.pin_mut()
  }
}

unsafe impl PinMut for ffi::BindCameraConfiguration {
  type Inner = ffi::CameraConfiguration;
  unsafe fn get(&mut self) -> Pin<&mut Self::Inner> {
    self.inner.pin_mut()
  }
}

unsafe impl PinMut for ffi::BindStreamConfiguration {
  type Inner = ffi::StreamConfiguration;
  unsafe fn get(&mut self) -> Pin<&mut Self::Inner> {
    self.inner.pin_mut()
  }
}

unsafe impl PinMut for ffi::BindStream {
  type Inner = ffi::Stream;
  unsafe fn get(&mut self) -> Pin<&mut Self::Inner> {
    self.inner.pin_mut()
  }
}

unsafe impl PinMut for ffi::BindFrameBufferAllocator {
  type Inner = ffi::FrameBufferAllocator;
  unsafe fn get(&mut self) -> Pin<&mut Self::Inner> {
    self.inner.pin_mut()
  }
}

unsafe impl PinMut for ffi::BindFrameBuffer {
  type Inner = ffi::FrameBuffer;
  unsafe fn get(&mut self) -> Pin<&mut Self::Inner> {
    self.inner.pin_mut()
  }
}

unsafe impl PinMut for ffi::BindFrameBufferPlane {
  type Inner = ffi::FrameBufferPlane;
  unsafe fn get(&mut self) -> Pin<&mut Self::Inner> {
    self.inner.pin_mut()
  }
}

unsafe impl PinMut for ffi::BindMemoryBuffer {
  type Inner = ffi::MemoryBuffer;
  unsafe fn get(&mut self) -> Pin<&mut Self::Inner> {
    self.inner.pin_mut()
  }
}

unsafe impl PinMut for ffi::BindRequest {
  type Inner = ffi::Request;
  unsafe fn get(&mut self) -> Pin<&mut Self::Inner> {
    self.inner.pin_mut()
  }
}
