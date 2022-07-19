#include "core.hpp"

#include "libcamera-rs/src/bridge.rs.h"

#include <iostream>
#include <thread>

BindCameraManager make_camera_manager() {
  BindCameraManager manager{
      .inner = std::make_unique<CameraManager>(
          std::make_unique<libcamera::CameraManager>()),
  };
  std::cerr << " - make_camera_manager() -> "
            << static_cast<const void *>(&*manager.inner) << " on "
            << std::this_thread::get_id() << std::endl;
  return manager;
}

CameraManager::~CameraManager() {
  std::cerr << " - CameraManager::~CameraManager("
            << static_cast<const void *>(this) << ")"
            << " on " << std::this_thread::get_id() << std::endl;
}

void CameraManager::start() {
  VALIDATE_POINTERS()

  std::cerr << " - CameraManager::start(" << static_cast<const void *>(this)
            << ")"
            << " on " << std::this_thread::get_id() << std::endl;
  int ret = this->inner->start();
  if (ret < 0) {
    throw error_from_code(-ret);
  }
}

void CameraManager::stop() {
  VALIDATE_POINTERS()

  std::cerr << " - CameraManager::stop(" << static_cast<const void *>(this)
            << ")"
            << " on " << std::this_thread::get_id() << std::endl;
  this->inner->stop();
}

rust::Vec<rust::String> CameraManager::get_camera_ids() const {
  VALIDATE_POINTERS()

  std::cerr << " - CameraManager::get_camera_ids("
            << static_cast<const void *>(this) << ")"
            << " on " << std::this_thread::get_id() << std::endl;
  rust::Vec<rust::String> camera_ids;
  for (std::shared_ptr<libcamera::Camera> cam : this->inner->cameras()) {
    // *(int *)(*(char **)((char *)(void *)&cam+4)+4) = 50;
    camera_ids.push_back(cam->id());
  }
  return camera_ids;
}

BindCamera CameraManager::get_camera_by_id(rust::Str id) {
  VALIDATE_POINTERS()

  std::string cam_id = std::string(id);
  std::shared_ptr<libcamera::Camera> cam = this->inner->get(cam_id);
  size_t uc =
      cam.use_count(); // Sometimes libcamera will give an invalid thing here???
  // int raw_uc_1 = *(int *)(*(char **)((char *)(void *)&cam+4)+4);
  // int raw_uc_2 = *(int *)(*(char **)((char *)(void *)&cam+4)+28);
  // std::cout << "uc=" << uc << "raw=" << raw_uc_1 << "," << raw_uc_2 << std::endl;
  if (!cam || uc == 0 || uc > 1000) {
    throw error_from_code(ENODEV);
  }
  BindCamera bind_cam{
      .inner = std::make_unique<Camera>(cam),
  };
  std::cerr << " - CameraManager::get_camera_by_id("
            << static_cast<const void *>(this) << ", " << cam_id << ") -> "
            << static_cast<const void *>(&*bind_cam.inner) << "(uc=" << uc
            << ") on " << std::this_thread::get_id() << std::endl;
  return bind_cam;
}
