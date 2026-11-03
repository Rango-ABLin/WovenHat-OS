// Camera module - capability-driven organization example
//! Demonstrates how modules declare required capabilities via Cargo features

#[cfg(feature = "cap-camera")]
mod camera_impl {
    #[derive(Debug, Clone, Copy)]
    pub enum CameraError { NotFound, Busy, IOError }

    pub struct Camera { device_id: u32 }

    impl Camera {
        /// Create camera device
        pub fn new(device_id: u32) -> Result<Self, CameraError> {
            if device_id > 4 { 
                Err(CameraError::NotFound) 
            } else { 
                Ok(Camera { device_id }) 
            }
        }

        /// Capture image from camera
        pub fn capture(&self) -> Result<Vec<u8>, CameraError> {
            Ok(vec![0x00, 0xFF, 0x00])
        }
    }
}

// Only export Camera if cap-camera feature is enabled
#[cfg(feature = "cap-camera")]
pub use camera_impl::{Camera, CameraError};
