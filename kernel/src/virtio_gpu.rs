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
const RESP_OK_NODATA: u32 = 0x1100;
const RESP_OK_DISPLAY_INFO: u32 = 0x1101;
/// B8G8R8X8: bytes B, G, R, X in memory, i.e. a little-endian 0x00RRGGBB.
const FORMAT_B8G8R8X8: u32 = 2;

pub const MAX_SCANOUTS: usize = 16;
/// Backing memory is allocated in runs of this many pages.
const CHUNK_PAGES: usize = 16;
const MAX_CHUNKS: usize = 64;
const PAGE_SIZE: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    NotFound,
    Transport(TransportError),
    Device(u32),
    TooLarge,
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
            let Some(Some(pages)) = self.chunks.get_mut(chunk) else { return };
            let bytes = &mut pages.bytes_mut()[within..within + fit * 4];
            for (slot, pixel) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(&pixels[written..written + fit]) {
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
        let (chunk, within) = (offset / (CHUNK_PAGES * PAGE_SIZE), offset % (CHUNK_PAGES * PAGE_SIZE));
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
        let mut transport = Transport::new(device, 0)?;
        let control = transport.queue(0)?;
        transport.ready()?;
        let scanouts = transport.device.read32(8)?.min(MAX_SCANOUTS as u32);
        Ok(Self {
            transport,
            control,
            buffers: DmaPages::allocate(4)?,
            scanouts,
            heads: [const { None }; MAX_SCANOUTS],
        })
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
        Ok(u32::from_le_bytes([response[0], response[1], response[2], response[3]]))
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
        let word = |offset: usize| u32::from_le_bytes([body[offset], body[offset + 1], body[offset + 2], body[offset + 3]]);
        let mut modes = [Mode::default(); MAX_SCANOUTS];
        for (index, mode) in modes.iter_mut().enumerate() {
            let base = index * 24;
            *mode = Mode { width: word(base + 8), height: word(base + 12), enabled: word(base + 16) != 0 };
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
        let mut head = Head { width, height, resource, chunks: [None; MAX_CHUNKS] };
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
    pub fn present_rect(&mut self, index: usize, x: u32, y: u32, width: u32, height: u32) -> Result<(), Error> {
        let (stride, resource) = match self.heads.get(index) {
            Some(Some(head)) if x + width <= head.width && y + height <= head.height => (head.width, head.resource),
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

/// Stage 20.14a acceptance: two monitors through virtio-gpu. Head 0 shows an
/// exact colour gradient and head 1 a checkerboard; the harness captures both
/// heads from the host (QMP screendump) and checks every pixel.
#[cfg(feature = "stage20-14-test")]
pub fn self_test() -> bool {
    fn step(name: &str, ok: bool) -> bool {
        crate::serial::write_line(format_args!("[S20.14] {} {}", name, if ok { "PASS" } else { "FAIL" }));
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
    match gpu.display_info() {
        Ok(modes) => crate::serial::write_line(format_args!(
            "[S20.14] head 0 {:?}, head 1 {:?}",
            modes[0], modes[1]
        )),
        Err(error) => return step(&alloc::format!("display info: {error:?}"), false),
    }
    for index in 0..2 {
        if let Err(error) = gpu.set_mode(index, WIDTH, HEIGHT) {
            return step(&alloc::format!("head {index} given a {WIDTH}x{HEIGHT} resource: {error:?}"), false);
        }
    }
    let gradient = |x: u32, y: u32| ((x * 255 / (WIDTH - 1)) << 16) | ((y * 255 / (HEIGHT - 1)) << 8) | 0x40;
    let checker = |x: u32, y: u32| if (x / 16 + y / 16).is_multiple_of(2) { 0x0020_40c0 } else { 0x00f0_d020 };
    for (index, pattern) in [(0usize, &gradient as &dyn Fn(u32, u32) -> u32), (1, &checker)] {
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
    crate::serial::write_line(format_args!("[S20.14] ready for capture {}x{}", WIDTH, HEIGHT));
    // The harness captures both heads now; keep them up meanwhile.
    let start = crate::timer::ticks();
    while crate::timer::ticks().wrapping_sub(start) < 1500 {
        crate::task::yield_now();
    }
    true
}
