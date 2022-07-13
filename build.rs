use std::env;

const SOURCES: [&str; 15] = [
  "libcamera-bridge/camera_manager.cpp",
  "libcamera-bridge/camera.cpp",
  "libcamera-bridge/camera_configuration.cpp",
  "libcamera-bridge/stream_configuration.cpp",
  "libcamera-bridge/pixel_format.cpp",
  "libcamera-bridge/size.cpp",
  "libcamera-bridge/stream.cpp",
  "libcamera-bridge/frame_buffer_allocator.cpp",
  "libcamera-bridge/frame_buffer.cpp",
  "libcamera-bridge/frame_buffer_plane.cpp",
  "libcamera-bridge/fd.cpp",
  "libcamera-bridge/memory_buffer.cpp",
  "libcamera-bridge/request.cpp",
  "libcamera-bridge/control_id.cpp",
  "libcamera-bridge/control_value.cpp",
];

fn main() {
  println!("cargo:rerun-if-changed=src/bridge.rs");
  println!("cargo:rerun-if-changed=libcamera-bridge/core.hpp");
  for source in &SOURCES {
    println!("cargo:rerun-if-changed={source}");
  }

  // link libcamera
  println!("cargo:rustc-link-search=/usr/local/lib");
  println!("cargo:rustc-link-lib=dylib=camera");
  println!("cargo:rustc-link-lib=dylib=camera-base");

  env::set_var("CROSS_COMPILE", "armv7-unknown-linux-gnueabihf");
  env::set_var("TARGET", "armv7-unknown-linux-gnueabihf");

  // Build CXX bridge
  cxx_build::bridge("src/bridge.rs")
    .flag("--target=armv7-unknown-linux-gnueabihf")
    .target("armv7-unknown-linux-gnueabihf")
    .flag("-std=c++17")
    .include("/usr/local/include/libcamera")
    .warnings(true)
    .extra_warnings(true)
    .files(SOURCES)
    .compile("libcamera-bridge");
}
