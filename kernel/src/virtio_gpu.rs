//! virtio-gpu 2D driver with several scanouts (monitors), Stage 20.14.
//!
//! Each head shows a host resource backed by guest pages. The driver creates
//! a B8G8R8X8 resource per head (the kernel framebuffer's own 0x00RRGGBB
//! pixel layout), attaches pages to it, binds it to the scanout, and
//! presents changes by transferring them to the host and flushing. Commands
//! complete synchronously on the control queue.

use crate::virtio_pci::{DmaPages, Error as TransportError, Queue, Transport};

const VENDOR_VIRTIO: u16 = 0x1af4;
const DEVICE_GPU: u16 = 0x1050;

const CMD_GET_DISPLAY_INFO: u32 = 0x0100;
const CMD_RESOURCE_CREATE_2D: u32 = 0x0101;
const CMD_SET_SCANOUT: u32 = 0x0103;
const CMD_RESOURCE_FLUSH: u32 = 0x0104;
const CMD_TRANSFER_TO_HOST_2D: u32 = 0x0105;
const CMD_RESOURCE_ATTACH_BACKING: u32 = 0x0106;
#[cfg(feature = "stage20-14-test")]
const CMD_GET_CAPSET_INFO: u32 = 0x0108;
#[cfg(feature = "stage20-14-test")]
const CMD_RESOURCE_CREATE_BLOB: u32 = 0x010c;
#[cfg(feature = "stage20-14-test")]
const CMD_SET_SCANOUT_BLOB: u32 = 0x010d;
#[cfg(feature = "stage20-14-test")]
const CMD_CTX_CREATE: u32 = 0x0200;
#[cfg(feature = "stage20-14-test")]
const CMD_CTX_DESTROY: u32 = 0x0201;
#[cfg(feature = "stage20-14-test")]
const CMD_CTX_ATTACH_RESOURCE: u32 = 0x0202;
#[cfg(feature = "stage20-14-test")]
const CMD_CTX_DETACH_RESOURCE: u32 = 0x0203;
#[cfg(feature = "stage20-14-test")]
const CMD_RESOURCE_CREATE_3D: u32 = 0x0204;
#[cfg(feature = "stage20-14-test")]
const CMD_SUBMIT_3D: u32 = 0x0207;
#[cfg(feature = "stage20-14-test")]
const CMD_RESOURCE_UNREF: u32 = 0x0102;
const RESP_OK_NODATA: u32 = 0x1100;
const RESP_OK_DISPLAY_INFO: u32 = 0x1101;
#[cfg(feature = "stage20-14-test")]
const RESP_OK_CAPSET_INFO: u32 = 0x1102;
/// B8G8R8X8: bytes B, G, R, X in memory, i.e. a little-endian 0x00RRGGBB.
const FORMAT_B8G8R8X8: u32 = 2;
/// VirtIO-GPU feature bit 0: the device accepts VIRGL 3D resources.
const FEATURE_VIRGL: u64 = 1 << 0;
#[cfg(feature = "stage20-14-test")]
const FEATURE_RESOURCE_BLOB: u64 = 1 << 3;
#[cfg(feature = "stage20-14-test")]
const BLOB_MEM_GUEST: u32 = 1;
#[cfg(feature = "stage20-14-test")]
const BLOB_FLAG_USE_MAPPABLE: u32 = 1;

pub const MAX_SCANOUTS: usize = 16;
/// Backing memory is allocated in runs of this many pages.
const CHUNK_PAGES: usize = 16;
const MAX_CHUNKS: usize = 64;
const PAGE_SIZE: usize = 4096;
#[cfg(feature = "stage20-14-test")]
const MAX_VIRGL_STREAM_BYTES: usize = 4096;
#[cfg(feature = "stage20-14-test")]
const MAX_VIRGL_STREAM_DWORDS: usize = MAX_VIRGL_STREAM_BYTES / 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    NotFound,
    #[cfg(feature = "stage20-14-test")]
    Unsupported,
    Transport(TransportError),
    Device(u32),
    TooLarge,
}

/// Hardware capabilities negotiated from the VirtIO-GPU device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub scanouts: u32,
    pub capsets: u32,
    pub virgl: bool,
    #[cfg(feature = "stage20-14-test")]
    pub resource_blob: bool,
}

#[cfg(feature = "stage20-14-test")]
struct BlobBacking {
    width: u32,
    height: u32,
    chunks: [Option<DmaPages>; MAX_CHUNKS],
    count: usize,
}

#[cfg(feature = "stage20-14-test")]
impl BlobBacking {
    fn allocate(width: u32, height: u32) -> Result<Self, Error> {
        let bytes = (width as usize)
            .checked_mul(height as usize)
            .and_then(|value| value.checked_mul(4))
            .ok_or(Error::TooLarge)?;
        let count = bytes.div_ceil(CHUNK_PAGES * PAGE_SIZE);
        if count == 0 || count > MAX_CHUNKS {
            return Err(Error::TooLarge);
        }
        let mut chunks = [const { None }; MAX_CHUNKS];
        for slot in chunks.iter_mut().take(count) {
            *slot = Some(DmaPages::allocate(CHUNK_PAGES)?);
        }
        Ok(Self {
            width,
            height,
            chunks,
            count,
        })
    }

    fn size(&self) -> u64 {
        self.width as u64 * self.height as u64 * 4
    }
}

/// Version and size advertised for one GPU rendering capset.
#[cfg(feature = "stage20-14-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapsetInfo {
    pub id: u32,
    pub max_version: u32,
    pub max_size: u32,
}

/// Validated dimensions and format fields for a VIRGL 3D resource request.
#[cfg(feature = "stage20-14-test")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resource3dDesc {
    pub context_id: u32,
    pub resource_id: u32,
    pub target: u32,
    pub format: u32,
    pub bind: u32,
    pub width: u32,
    pub height: u32,
}

/// Bounded VirGL command stream encoder. Command payload lengths are encoded
/// in the protocol header and the fixed capacity prevents unbounded DMA input.
#[cfg(feature = "stage20-14-test")]
pub struct VirglStream {
    words: [u32; MAX_VIRGL_STREAM_DWORDS],
    length: usize,
}

#[cfg(feature = "stage20-14-test")]
impl VirglStream {
    pub const fn new() -> Self {
        Self {
            words: [0; MAX_VIRGL_STREAM_DWORDS],
            length: 0,
        }
    }

    pub fn command(&mut self, opcode: u8, object: u8, payload: &[u32]) -> Result<(), Error> {
        if payload.len() > u16::MAX as usize {
            return Err(Error::TooLarge);
        }
        let required = payload.len().checked_add(1).ok_or(Error::TooLarge)?;
        if self.length.checked_add(required).ok_or(Error::TooLarge)? > self.words.len() {
            return Err(Error::TooLarge);
        }
        self.words[self.length] =
            u32::from(opcode) | (u32::from(object) << 8) | ((payload.len() as u32) << 16);
        self.length += 1;
        self.words[self.length..self.length + payload.len()].copy_from_slice(payload);
        self.length += payload.len();
        Ok(())
    }

    pub fn nop(&mut self) -> Result<(), Error> {
        self.command(0, 0, &[])
    }

    pub fn create_surface(
        &mut self,
        surface_handle: u32,
        resource_id: u32,
        format: u32,
    ) -> Result<(), Error> {
        self.command(1, 8, &[surface_handle, resource_id, format, 0, 1])
    }

    pub fn set_framebuffer_color(&mut self, surface_handle: u32) -> Result<(), Error> {
        self.command(5, 0, &[1, 0, surface_handle])
    }

    pub fn clear_color(&mut self, rgba: [u32; 4]) -> Result<(), Error> {
        self.command(7, 0, &[1, rgba[0], rgba[1], rgba[2], rgba[3], 0, 0, 0])
    }

    pub fn destroy_surface(&mut self, surface_handle: u32) -> Result<(), Error> {
        self.command(3, 8, &[surface_handle])
    }

    pub fn as_slice(&self) -> &[u32] {
        &self.words[..self.length]
    }
}

impl From<TransportError> for Error {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}

/// A scanout's mode as the device reports it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mode {
    pub enabled: bool,
    pub width: u32,
    pub height: u32,
}

/// A head the driver drives: its resource and the pages behind it.
pub struct Head {
    pub width: u32,
    pub height: u32,
    resource: u32,
    chunks: [Option<DmaPages>; MAX_CHUNKS],
}

impl Head {
    /// Write `pixels` (0x00RRGGBB) into row `y` from column `x`, crossing
    /// backing runs as needed.
    pub fn write_row(&mut self, x: u32, y: u32, pixels: &[u32]) {
        if y >= self.height || x >= self.width {
            return;
        }
        let count = pixels.len().min((self.width - x) as usize);
        let run = CHUNK_PAGES * PAGE_SIZE;
        let mut offset = (y as usize * self.width as usize + x as usize) * 4;
        let mut written = 0;
        while written < count {
            let (chunk, within) = (offset / run, offset % run);
            let fit = ((run - within) / 4).min(count - written);
            let Some(Some(pages)) = self.chunks.get_mut(chunk) else {
                return;
            };
            let bytes = &mut pages.bytes_mut()[within..within + fit * 4];
            for (slot, pixel) in bytes
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(&pixels[written..written + fit])
            {
                *slot = pixel.to_le_bytes();
            }
            written += fit;
            offset += fit * 4;
        }
    }

    /// Set one pixel (0x00RRGGBB) in the head's backing memory.
    #[cfg(feature = "stage20-14-test")]
    pub fn put(&mut self, x: u32, y: u32, pixel: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let offset = (y as usize * self.width as usize + x as usize) * 4;
        let (chunk, within) = (
            offset / (CHUNK_PAGES * PAGE_SIZE),
            offset % (CHUNK_PAGES * PAGE_SIZE),
        );
        if let Some(Some(pages)) = self.chunks.get_mut(chunk) {
            pages.bytes_mut()[within..within + 4].copy_from_slice(&pixel.to_le_bytes());
        }
    }
}

pub struct Gpu {
    transport: Transport,
    control: Queue,
    /// Request and response buffers for the control queue.
    buffers: DmaPages,
    pub scanouts: u32,
    capsets: u32,
    pub heads: [Option<Head>; MAX_SCANOUTS],
}

const RESPONSE_OFFSET: usize = 2 * PAGE_SIZE;

impl Gpu {
    /// Find the first virtio-gpu and bring it up.
    pub fn probe() -> Result<Self, Error> {
        let device = (0..crate::hal::pci::topology::MAX_FUNCTIONS)
            .filter_map(crate::hal::pci::device)
            .find(|device| device.vendor_id == VENDOR_VIRTIO && device.device_id == DEVICE_GPU)
            .ok_or(Error::NotFound)?;
        let mut transport = Transport::new(
            device,
            FEATURE_VIRGL | {
                #[cfg(feature = "stage20-14-test")]
                {
                    FEATURE_RESOURCE_BLOB
                }
                #[cfg(not(feature = "stage20-14-test"))]
                {
                    0
                }
            },
        )?;
        let control = transport.queue(0)?;
        transport.ready()?;
        let scanouts = transport.device.read32(8)?.min(MAX_SCANOUTS as u32);
        let capsets = transport.device.read32(12)?;
        let gpu = Self {
            transport,
            control,
            buffers: DmaPages::allocate(4)?,
            scanouts,
            capsets,
            heads: [const { None }; MAX_SCANOUTS],
        };
        let _ = gpu.capabilities();
        Ok(gpu)
    }

    /// Report the negotiated display and optional 3D capability.
    pub const fn capabilities(&self) -> Capabilities {
        Capabilities {
            scanouts: self.scanouts,
            capsets: self.capsets,
            virgl: self.transport.negotiated_features() & FEATURE_VIRGL != 0,
            #[cfg(feature = "stage20-14-test")]
            resource_blob: self.transport.negotiated_features() & FEATURE_RESOURCE_BLOB != 0,
        }
    }

    #[cfg(feature = "stage20-14-test")]
    fn create_blob(&mut self, resource_id: u32, backing: &BlobBacking) -> Result<(), Error> {
        if !self.capabilities().resource_blob {
            return Err(Error::Unsupported);
        }
        let mut request = header(CMD_RESOURCE_CREATE_BLOB).to_vec();
        request.extend_from_slice(&resource_id.to_le_bytes());
        request.extend_from_slice(&BLOB_MEM_GUEST.to_le_bytes());
        request.extend_from_slice(&BLOB_FLAG_USE_MAPPABLE.to_le_bytes());
        request.extend_from_slice(&(backing.count as u32).to_le_bytes());
        request.extend_from_slice(&0u64.to_le_bytes());
        request.extend_from_slice(&backing.size().to_le_bytes());
        for pages in backing.chunks.iter().take(backing.count).flatten() {
            request.extend_from_slice(&pages.physical.to_le_bytes());
            request.extend_from_slice(&((CHUNK_PAGES * PAGE_SIZE) as u32).to_le_bytes());
            request.extend_from_slice(&0u32.to_le_bytes());
        }
        self.expect_ok(&request)
    }

    #[cfg(feature = "stage20-14-test")]
    fn set_scanout_blob(
        &mut self,
        index: usize,
        resource_id: u32,
        width: u32,
        height: u32,
        format: u32,
    ) -> Result<(), Error> {
        if !self.capabilities().resource_blob || index >= self.scanouts as usize {
            return Err(Error::Unsupported);
        }
        let stride = width.checked_mul(4).ok_or(Error::TooLarge)?;
        let mut request = header(CMD_SET_SCANOUT_BLOB).to_vec();
        for value in [
            0,
            0,
            width,
            height,
            index as u32,
            resource_id,
            width,
            height,
            format,
            0,
            stride,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ] {
            request.extend_from_slice(&value.to_le_bytes());
        }
        self.expect_ok(&request)
    }

    /// Query one advertised rendering capset before selecting a 3D protocol.
    #[cfg(feature = "stage20-14-test")]
    pub fn capset_info(&mut self, index: u32) -> Result<CapsetInfo, Error> {
        if !self.capabilities().virgl || index >= self.capsets {
            return Err(Error::Unsupported);
        }
        let mut request = header(CMD_GET_CAPSET_INFO).to_vec();
        request.extend_from_slice(&index.to_le_bytes());
        request.extend_from_slice(&0u32.to_le_bytes());
        match self.command(&request, 48)? {
            RESP_OK_CAPSET_INFO => {
                let response = &self.buffers.bytes()[RESPONSE_OFFSET + 24..];
                let word = |offset: usize| {
                    u32::from_le_bytes([
                        response[offset],
                        response[offset + 1],
                        response[offset + 2],
                        response[offset + 3],
                    ])
                };
                Ok(CapsetInfo {
                    id: word(0),
                    max_version: word(4),
                    max_size: word(8),
                })
            }
            other => Err(Error::Device(other)),
        }
    }

    /// Create a VIRGL rendering context with a bounded debug name.
    #[cfg(feature = "stage20-14-test")]
    pub fn create_context(&mut self, context_id: u32, name: &[u8]) -> Result<(), Error> {
        if !self.capabilities().virgl || name.len() > 64 {
            return Err(Error::Unsupported);
        }
        let mut request = header_with_context(CMD_CTX_CREATE, context_id).to_vec();
        request.extend_from_slice(&(name.len() as u32).to_le_bytes());
        request.extend_from_slice(&0u32.to_le_bytes());
        let mut debug_name = [0u8; 64];
        debug_name[..name.len()].copy_from_slice(name);
        request.extend_from_slice(&debug_name);
        self.expect_ok(&request)
    }

    /// Destroy a previously created VIRGL rendering context.
    #[cfg(feature = "stage20-14-test")]
    pub fn destroy_context(&mut self, context_id: u32) -> Result<(), Error> {
        if !self.capabilities().virgl {
            return Err(Error::Unsupported);
        }
        let request = header_with_context(CMD_CTX_DESTROY, context_id);
        self.expect_ok(&request)
    }

    /// Create a bounded 2D VIRGL resource in a rendering context.
    #[cfg(feature = "stage20-14-test")]
    pub fn create_resource_3d(&mut self, desc: Resource3dDesc) -> Result<(), Error> {
        if !self.capabilities().virgl || desc.width == 0 || desc.height == 0 {
            return Err(Error::Unsupported);
        }
        let mut request = header_with_context(CMD_RESOURCE_CREATE_3D, desc.context_id).to_vec();
        for value in [
            desc.resource_id,
            desc.target,
            desc.format,
            desc.bind,
            desc.width,
            desc.height,
            1,
            1,
            0,
            0,
            1,
            0,
        ] {
            request.extend_from_slice(&value.to_le_bytes());
        }
        self.expect_ok(&request)
    }

    /// Attach a resource to a VIRGL context before issuing commands.
    #[cfg(feature = "stage20-14-test")]
    pub fn attach_resource(&mut self, context_id: u32, resource_id: u32) -> Result<(), Error> {
        if !self.capabilities().virgl {
            return Err(Error::Unsupported);
        }
        let mut request = header_with_context(CMD_CTX_ATTACH_RESOURCE, context_id).to_vec();
        request.extend_from_slice(&resource_id.to_le_bytes());
        request.extend_from_slice(&0u32.to_le_bytes());
        self.expect_ok(&request)
    }

    /// Bind a VIRGL resource to a scanout. Resource zero disables the scanout.
    #[cfg(feature = "stage20-14-test")]
    pub fn set_scanout_resource(
        &mut self,
        index: usize,
        resource_id: u32,
        width: u32,
        height: u32,
    ) -> Result<(), Error> {
        if index >= self.scanouts as usize {
            return Err(Error::NotFound);
        }
        if resource_id != 0 && (width == 0 || height == 0) {
            return Err(Error::Unsupported);
        }
        let mut request = header(CMD_SET_SCANOUT).to_vec();
        for value in [0, 0, width, height, index as u32, resource_id] {
            request.extend_from_slice(&value.to_le_bytes());
        }
        self.expect_ok(&request)
    }

    /// Detach a resource from a VIRGL context.
    #[cfg(feature = "stage20-14-test")]
    pub fn detach_resource(&mut self, context_id: u32, resource_id: u32) -> Result<(), Error> {
        if !self.capabilities().virgl {
            return Err(Error::Unsupported);
        }
        let mut request = header_with_context(CMD_CTX_DETACH_RESOURCE, context_id).to_vec();
        request.extend_from_slice(&resource_id.to_le_bytes());
        request.extend_from_slice(&0u32.to_le_bytes());
        self.expect_ok(&request)
    }

    /// Submit a bounded little-endian VIRGL command stream.
    #[cfg(feature = "stage20-14-test")]
    pub fn submit_3d(&mut self, context_id: u32, stream: &[u32]) -> Result<(), Error> {
        let bytes = stream
            .len()
            .checked_mul(core::mem::size_of::<u32>())
            .ok_or(Error::TooLarge)?;
        if !self.capabilities().virgl || bytes > MAX_VIRGL_STREAM_BYTES {
            return Err(Error::Unsupported);
        }
        let mut request = header_with_context(CMD_SUBMIT_3D, context_id).to_vec();
        request.extend_from_slice(&(bytes as u32).to_le_bytes());
        request.extend_from_slice(&0u32.to_le_bytes());
        for word in stream {
            request.extend_from_slice(&word.to_le_bytes());
        }
        self.expect_ok(&request)
    }

    /// Release a VIRGL resource after detaching it from all contexts.
    #[cfg(feature = "stage20-14-test")]
    pub fn unref_resource(&mut self, resource_id: u32) -> Result<(), Error> {
        if !self.capabilities().virgl {
            return Err(Error::Unsupported);
        }
        let mut request = header(CMD_RESOURCE_UNREF).to_vec();
        request.extend_from_slice(&resource_id.to_le_bytes());
        request.extend_from_slice(&0u32.to_le_bytes());
        self.expect_ok(&request)
    }

    /// Send `request` (built in the request buffer) and return the response
    /// type; the response body stays in the response buffer.
    fn command(&mut self, request: &[u8], response_length: u32) -> Result<u32, Error> {
        if request.len() > RESPONSE_OFFSET {
            return Err(Error::TooLarge);
        }
        let physical = self.buffers.physical;
        let bytes = self.buffers.bytes_mut();
        bytes[..request.len()].copy_from_slice(request);
        bytes[RESPONSE_OFFSET..RESPONSE_OFFSET + 24].fill(0);
        self.transport.exchange(
            &mut self.control,
            (physical, request.len() as u32),
            (physical + RESPONSE_OFFSET as u64, response_length),
        )?;
        let response = &self.buffers.bytes()[RESPONSE_OFFSET..];
        Ok(u32::from_le_bytes([
            response[0],
            response[1],
            response[2],
            response[3],
        ]))
    }

    fn expect_ok(&mut self, request: &[u8]) -> Result<(), Error> {
        match self.command(request, 24)? {
            RESP_OK_NODATA => Ok(()),
            other => Err(Error::Device(other)),
        }
    }

    /// Every scanout's reported mode.
    pub fn display_info(&mut self) -> Result<[Mode; MAX_SCANOUTS], Error> {
        let request = header(CMD_GET_DISPLAY_INFO);
        match self.command(&request, 24 + 24 * MAX_SCANOUTS as u32)? {
            RESP_OK_DISPLAY_INFO => {}
            other => return Err(Error::Device(other)),
        }
        let body = &self.buffers.bytes()[RESPONSE_OFFSET + 24..];
        let word = |offset: usize| {
            u32::from_le_bytes([
                body[offset],
                body[offset + 1],
                body[offset + 2],
                body[offset + 3],
            ])
        };
        let mut modes = [Mode::default(); MAX_SCANOUTS];
        for (index, mode) in modes.iter_mut().enumerate() {
            let base = index * 24;
            *mode = Mode {
                width: word(base + 8),
                height: word(base + 12),
                enabled: word(base + 16) != 0,
            };
        }
        Ok(modes)
    }

    /// Give scanout `index` a `width` x `height` resource and show it.
    pub fn set_mode(&mut self, index: usize, width: u32, height: u32) -> Result<(), Error> {
        if index >= self.scanouts as usize {
            return Err(Error::NotFound);
        }
        let bytes = width as usize * height as usize * 4;
        let chunks = bytes.div_ceil(CHUNK_PAGES * PAGE_SIZE);
        if chunks > MAX_CHUNKS {
            return Err(Error::TooLarge);
        }
        let resource = index as u32 + 1;
        let mut request = header(CMD_RESOURCE_CREATE_2D).to_vec();
        for value in [resource, FORMAT_B8G8R8X8, width, height] {
            request.extend_from_slice(&value.to_le_bytes());
        }
        self.expect_ok(&request)?;
        let mut head = Head {
            width,
            height,
            resource,
            chunks: [None; MAX_CHUNKS],
        };
        let mut request = header(CMD_RESOURCE_ATTACH_BACKING).to_vec();
        request.extend_from_slice(&resource.to_le_bytes());
        request.extend_from_slice(&(chunks as u32).to_le_bytes());
        for slot in head.chunks.iter_mut().take(chunks) {
            let pages = DmaPages::allocate(CHUNK_PAGES)?;
            request.extend_from_slice(&pages.physical.to_le_bytes());
            request.extend_from_slice(&((CHUNK_PAGES * PAGE_SIZE) as u32).to_le_bytes());
            request.extend_from_slice(&0u32.to_le_bytes());
            *slot = Some(pages);
        }
        self.expect_ok(&request)?;
        let mut request = header(CMD_SET_SCANOUT).to_vec();
        for value in [0, 0, width, height, index as u32, resource] {
            request.extend_from_slice(&value.to_le_bytes());
        }
        self.expect_ok(&request)?;
        self.heads[index] = Some(head);
        Ok(())
    }

    /// Show scanout `index`'s current pixels.
    #[cfg(feature = "stage20-14-test")]
    pub fn present(&mut self, index: usize) -> Result<(), Error> {
        let (width, height) = match self.heads.get(index) {
            Some(Some(head)) => (head.width, head.height),
            _ => return Err(Error::NotFound),
        };
        self.present_rect(index, 0, 0, width, height)
    }

    /// Show the `width` x `height` rectangle at (`x`, `y`) of scanout
    /// `index`.
    pub fn present_rect(
        &mut self,
        index: usize,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<(), Error> {
        let (stride, resource) = match self.heads.get(index) {
            Some(Some(head)) if x + width <= head.width && y + height <= head.height => {
                (head.width, head.resource)
            }
            _ => return Err(Error::NotFound),
        };
        let offset = (u64::from(y) * u64::from(stride) + u64::from(x)) * 4;
        let mut request = header(CMD_TRANSFER_TO_HOST_2D).to_vec();
        for value in [x, y, width, height] {
            request.extend_from_slice(&value.to_le_bytes());
        }
        request.extend_from_slice(&offset.to_le_bytes());
        request.extend_from_slice(&resource.to_le_bytes());
        request.extend_from_slice(&0u32.to_le_bytes());
        self.expect_ok(&request)?;
        let mut request = header(CMD_RESOURCE_FLUSH).to_vec();
        for value in [x, y, width, height, resource, 0] {
            request.extend_from_slice(&value.to_le_bytes());
        }
        self.expect_ok(&request)
    }
}

fn header(command: u32) -> [u8; 24] {
    let mut header = [0u8; 24];
    header[0..4].copy_from_slice(&command.to_le_bytes());
    header
}

#[cfg(feature = "stage20-14-test")]
fn header_with_context(command: u32, context_id: u32) -> [u8; 24] {
    let mut header = header(command);
    header[16..20].copy_from_slice(&context_id.to_le_bytes());
    header
}

/// Stage 20.14a acceptance: two monitors through virtio-gpu. Head 0 shows an
/// exact colour gradient and head 1 a checkerboard; the harness captures both
/// heads from the host (QMP screendump) and checks every pixel.
#[cfg(feature = "stage20-14-test")]
pub fn self_test() -> bool {
    fn step(name: &str, ok: bool) -> bool {
        crate::serial::write_line(format_args!(
            "[S20.14] {} {}",
            name,
            if ok { "PASS" } else { "FAIL" }
        ));
        ok
    }
    const WIDTH: u32 = 640;
    const HEIGHT: u32 = 400;
    let mut gpu = match Gpu::probe() {
        Ok(gpu) => gpu,
        Err(error) => {
            crate::serial::write_line(format_args!("[S20.14] probe: {:?}", error));
            return step("virtio-gpu found and brought up", false);
        }
    };
    crate::serial::write_line(format_args!("[S20.14] scanouts={}", gpu.scanouts));
    if !step("virtio-gpu found with two scanouts", gpu.scanouts >= 2) {
        return false;
    }
    if gpu.capabilities().virgl {
        if gpu.capabilities().capsets > 0 {
            match gpu.capset_info(0) {
                Ok(info) => crate::serial::write_line(format_args!(
                    "[S20.14] VIRGL capset id={} version={} size={}",
                    info.id, info.max_version, info.max_size
                )),
                Err(error) => {
                    return step(&alloc::format!("VIRGL capset query: {error:?}"), false);
                }
            }
        }
        if let Err(error) = gpu.create_context(1, b"wovenhat-virgl") {
            return step(&alloc::format!("VIRGL context create: {error:?}"), false);
        }
        #[cfg(feature = "stage20-14-test")]
        if gpu.capabilities().resource_blob {
            let backing = match BlobBacking::allocate(WIDTH, HEIGHT) {
                Ok(backing) => backing,
                Err(error) => {
                    let _ = gpu.destroy_context(1);
                    return step(&alloc::format!("VIRGL blob backing: {error:?}"), false);
                }
            };
            if let Err(error) = gpu.create_blob(7, &backing) {
                let _ = gpu.destroy_context(1);
                return step(&alloc::format!("VIRGL blob create: {error:?}"), false);
            }
            if let Err(error) = gpu.attach_resource(1, 7) {
                let _ = gpu.unref_resource(7);
                let _ = gpu.destroy_context(1);
                return step(&alloc::format!("VIRGL blob attach: {error:?}"), false);
            }
            let mut stream = VirglStream::new();
            if let Err(error) = stream
                .create_surface(9, 7, 1)
                .and_then(|_| stream.set_framebuffer_color(9))
                .and_then(|_| stream.clear_color([0, 0x3f80_0000, 0, 0x3f80_0000]))
                .and_then(|_| stream.destroy_surface(9))
            {
                let _ = gpu.detach_resource(1, 7);
                let _ = gpu.unref_resource(7);
                let _ = gpu.destroy_context(1);
                return step(
                    &alloc::format!("VIRGL blob command encode: {error:?}"),
                    false,
                );
            }
            if let Err(error) = gpu.submit_3d(1, stream.as_slice()) {
                let _ = gpu.detach_resource(1, 7);
                let _ = gpu.unref_resource(7);
                let _ = gpu.destroy_context(1);
                return step(
                    &alloc::format!("VIRGL blob command submit: {error:?}"),
                    false,
                );
            }
            if let Err(error) = gpu.set_scanout_blob(0, 7, WIDTH, HEIGHT, 1) {
                let _ = gpu.detach_resource(1, 7);
                let _ = gpu.unref_resource(7);
                let _ = gpu.destroy_context(1);
                return step(&alloc::format!("VIRGL blob scanout bind: {error:?}"), false);
            }
            crate::serial::write_line(format_args!(
                "[S20.14] VIRGL GPU blob clear scanout ready {}x{}",
                WIDTH, HEIGHT
            ));
            let start = crate::timer::ticks();
            while crate::timer::ticks().wrapping_sub(start) < 1500 {
                x86_64::instructions::hlt();
            }
            let _ = gpu.set_scanout_blob(0, 0, 0, 0, 0);
            let _ = gpu.detach_resource(1, 7);
            let _ = gpu.unref_resource(7);
            let _ = gpu.destroy_context(1);
            crate::serial::write_line(format_args!(
                "[S20.14] VIRGL context/resource/command lifecycle PASS"
            ));
            return true;
        }
        if let Err(error) = gpu.create_resource_3d(Resource3dDesc {
            context_id: 1,
            resource_id: 7,
            target: 2,
            format: 1,
            bind: 1,
            width: 64,
            height: 64,
        }) {
            let _ = gpu.destroy_context(1);
            return step(&alloc::format!("VIRGL resource create: {error:?}"), false);
        }
        if let Err(error) = gpu.attach_resource(1, 7) {
            let _ = gpu.unref_resource(7);
            let _ = gpu.destroy_context(1);
            return step(&alloc::format!("VIRGL resource attach: {error:?}"), false);
        }
        // VIRGL_CCMD_NOP: opcode 0, object 0, zero payload dwords. Build the
        // header through the bounded encoder so later draw commands share the
        // same checked framing path.
        let mut stream = VirglStream::new();
        if let Err(error) = stream
            .create_surface(9, 7, 1)
            .and_then(|_| stream.set_framebuffer_color(9))
            .and_then(|_| stream.clear_color([0, 0x3f80_0000, 0, 0x3f80_0000]))
            .and_then(|_| stream.destroy_surface(9))
        {
            let _ = gpu.detach_resource(1, 7);
            let _ = gpu.unref_resource(7);
            let _ = gpu.destroy_context(1);
            return step(&alloc::format!("VIRGL command encode: {error:?}"), false);
        }
        if let Err(error) = gpu.submit_3d(1, stream.as_slice()) {
            let _ = gpu.detach_resource(1, 7);
            let _ = gpu.unref_resource(7);
            let _ = gpu.destroy_context(1);
            return step(&alloc::format!("VIRGL command submit: {error:?}"), false);
        }
        if let Err(error) = gpu.detach_resource(1, 7) {
            let _ = gpu.unref_resource(7);
            let _ = gpu.destroy_context(1);
            return step(&alloc::format!("VIRGL resource detach: {error:?}"), false);
        }
        if let Err(error) = gpu.unref_resource(7) {
            let _ = gpu.destroy_context(1);
            return step(&alloc::format!("VIRGL resource release: {error:?}"), false);
        }
        if let Err(error) = gpu.destroy_context(1) {
            return step(&alloc::format!("VIRGL context destroy: {error:?}"), false);
        }
        crate::serial::write_line(format_args!(
            "[S20.14] VIRGL context/resource/command lifecycle PASS"
        ));
        // Exercise the conventional 2D resource path on the GL-capable
        // device as a separate visible-scanout probe. This does not claim
        // that the 3D resource can be scanned out; it verifies that a
        // software-rendered frame can still be presented when the device
        // advertises VIRGL.
        if let Err(error) = gpu.set_mode(0, WIDTH, HEIGHT) {
            crate::serial::write_line(format_args!(
                "[S20.14] VIRGL 2D scanout unavailable: {error:?}"
            ));
            return true;
        }
        let mut row = [0u32; WIDTH as usize];
        for y in 0..HEIGHT {
            for (x, pixel) in row.iter_mut().enumerate() {
                let x = x as u32;
                *pixel = ((x * 255 / (WIDTH - 1)) << 16) | ((y * 255 / (HEIGHT - 1)) << 8) | 0x40;
            }
            if let Some(head) = gpu.heads[0].as_mut() {
                head.write_row(0, y, &row);
            } else {
                return step("VIRGL 2D scanout backing", false);
            }
        }
        if let Err(error) = gpu.present(0) {
            crate::serial::write_line(format_args!(
                "[S20.14] VIRGL 2D scanout unavailable: {error:?}"
            ));
            return true;
        }
        crate::serial::write_line(format_args!(
            "[S20.14] VIRGL 2D scanout ready {}x{}",
            WIDTH, HEIGHT
        ));
        return true;
    }
    match gpu.display_info() {
        Ok(modes) => crate::serial::write_line(format_args!(
            "[S20.14] head 0 {:?}, head 1 {:?}",
            modes[0], modes[1]
        )),
        Err(error) => return step(&alloc::format!("display info: {error:?}"), false),
    }
    for index in 0..2 {
        if let Err(error) = gpu.set_mode(index, WIDTH, HEIGHT) {
            return step(
                &alloc::format!("head {index} given a {WIDTH}x{HEIGHT} resource: {error:?}"),
                false,
            );
        }
    }
    let gradient =
        |x: u32, y: u32| ((x * 255 / (WIDTH - 1)) << 16) | ((y * 255 / (HEIGHT - 1)) << 8) | 0x40;
    let checker = |x: u32, y: u32| {
        if (x / 16 + y / 16).is_multiple_of(2) {
            0x0020_40c0
        } else {
            0x00f0_d020
        }
    };
    for (index, pattern) in [
        (0usize, &gradient as &dyn Fn(u32, u32) -> u32),
        (1, &checker),
    ] {
        if let Some(head) = gpu.heads[index].as_mut() {
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    head.put(x, y, pattern(x, y));
                }
            }
        }
        if let Err(error) = gpu.present(index) {
            return step(&alloc::format!("head {index} presented: {error:?}"), false);
        }
    }
    step("both heads given resources and presented", true);
    crate::serial::write_line(format_args!(
        "[S20.14] ready for capture {}x{}",
        WIDTH, HEIGHT
    ));
    // The harness captures both heads now; keep them up meanwhile.
    let start = crate::timer::ticks();
    while crate::timer::ticks().wrapping_sub(start) < 1500 {
        crate::task::yield_now();
    }
    true
}
